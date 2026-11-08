//! Decide what has to change.
//!
//! `plan` is the whole engine: it renders every import, compares the result
//! with the project tree and with the manifest, and returns typed actions
//! plus the problems `check` reports. It never writes. `check` prints the
//! problems; `sync` performs the actions — and because every refusal is
//! decided here, `sync` cannot start something it will regret.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::Config;
use crate::files::{self, guard_ancestors};
use crate::manifest::{self, Manifest, hash_bytes};
use crate::merge;
use crate::registry::{AssetKind, Registry};
use crate::render::{self, Rendered};
use crate::report::{Origin, Status};
use crate::spec::Spec;

// --------------------------------------------------------------- outputs

/// How a `Write` came about, which is what decides the word `sync` prints
/// and whether `sync` is willing to do it at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// Nothing usable was there: `WRITE`.
    Fresh,
    /// A file was there but held different bytes we wrote last time:
    /// `UPDATE`.
    Update,
    /// A file was there and differs from the render, and we cannot show it
    /// is ours: `CONFLICT`. `sync` skips this unless `--force`.
    Conflict,
}

/// One change `sync` should make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Make `path` a real directory, clearing an obstacle there first.
    /// Emitted only when an obstacle is in the way.
    PrepareDir { path: PathBuf },
    /// Write rendered bytes at `path`, replacing what is there.
    Write {
        path: PathBuf,
        bytes: Vec<u8>,
        kind: WriteKind,
        origin: Origin,
    },
    /// Delete an owned path and prune now-empty parents.
    Remove { path: PathBuf },
    /// Merge content into a JSON file the project owns, creating it if
    /// needed.
    MergeJson { path: PathBuf, content: Value },
}

/// One thing `check` reports.
///
/// Shaped like [`FileState`] on purpose: the same three facts, with the
/// attribution optional because a stray or an orphan belongs to no import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub status: Status,
    pub path: PathBuf,
    pub origin: Option<Origin>,
}

impl Problem {
    fn new(status: Status, path: impl Into<PathBuf>, origin: Option<Origin>) -> Problem {
        Problem {
            status,
            path: path.into(),
            origin,
        }
    }
}

/// One emitted file and the state `status` reports for it.
///
/// Unlike [`Problem`] this is never suppressed: a file under a blocked asset
/// root still has a state of its own, so `status` never calls a file clean
/// when the run is going to rewrite it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub path: PathBuf,
    pub origin: Origin,
    pub status: Status,
}

/// Everything `check`, `sync` and `render` need to know about a run.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Every file the config produces: `(path relative to the project, bytes)`.
    pub tree: Vec<(PathBuf, Vec<u8>)>,
    /// The same content in manifest shape, ready to write on a clean sync.
    pub manifest: Manifest,
    /// What has to change on disk.
    pub actions: Vec<Action>,
    /// What `check` reports, in the order it prints them.
    pub problems: Vec<Problem>,
    /// Every emitted file with its state, in `tree` order. `status` reads
    /// this instead of `problems`.
    pub states: Vec<FileState>,
    /// Files under a category root that neither this run nor the previous
    /// one owns. They are never touched; `status` counts them so a reader
    /// can tell ours from theirs at a glance.
    pub custom: Vec<PathBuf>,
}

// ------------------------------------------------------------ classification

/// What is sitting at a path the plan wants to own.
enum Disk {
    Absent,
    File(Vec<u8>),
    /// A symlink, or a thing of the wrong kind.
    Obstacle,
}

/// A settled file, before it becomes an action and a problem.
struct Outcome {
    path: PathBuf,
    origin: Origin,
    bytes: Vec<u8>,
    disk: Disk,
    /// The asset root above this file is an obstacle that has to go first.
    /// The action is still planned, but the asset's own `DRIFT` already says
    /// what is wrong and per-file noise would only repeat it.
    blocked: bool,
}

/// Render every import, then work out what the project needs.
pub fn plan(
    config: &Config,
    registry: &Registry,
    spec: &Spec,
    manifest: Option<&Manifest>,
    project_root: &Path,
) -> Result<Plan, String> {
    let resolved = resolve(config, registry, spec)?;

    // Nothing the plan touches may be reached through a symlinked directory:
    // the write would land outside the project. Directory asset roots are
    // the exception — an obstacle there is cleared before anything is
    // written under it.
    let replaceable: BTreeSet<PathBuf> = resolved
        .iter()
        .map(|asset| asset.asset_path.clone())
        .collect();
    for asset in &resolved {
        guard_ancestors(project_root, &asset.asset_path, &replaceable)?;
        for (path, _) in &asset.files {
            guard_ancestors(project_root, path, &replaceable)?;
        }
    }

    // An obstacle at a directory asset has to go before anything is written
    // under it. Files below one are classified as fresh rather than read,
    // because reading them would follow the link out of the project.
    let mut prepares: Vec<Action> = Vec::new();
    let mut blocked: BTreeSet<PathBuf> = BTreeSet::new();
    let mut asset_problems: Vec<Problem> = Vec::new();
    for asset in &resolved {
        if asset.kind != AssetKind::Dir {
            continue;
        }
        if dir_obstacle(project_root, &asset.asset_path)?.is_some() {
            blocked.insert(asset.asset_path.clone());
            prepares.push(Action::PrepareDir {
                path: asset.asset_path.clone(),
            });
            // `check` reports the wrong kind of thing as drift whether or not
            // we own it; only `sync` gets to decide whether to replace it.
            asset_problems.push(Problem::new(
                Status::Drift,
                asset.asset_path.clone(),
                Some(asset.origin.clone()),
            ));
        }
    }

    let mut outcomes = Vec::new();
    for asset in &resolved {
        for (path, bytes) in &asset.files {
            let blocked = blocked.iter().any(|root| path.starts_with(root));
            let disk = if blocked {
                Disk::Absent
            } else {
                disk_file(&project_root.join(path))?
            };
            outcomes.push(Outcome {
                path: path.clone(),
                origin: asset.origin.clone(),
                bytes: bytes.clone(),
                disk,
                blocked,
            });
        }
    }

    let mut writes: Vec<Action> = Vec::new();
    let mut file_problems: Vec<Problem> = Vec::new();
    let mut states: Vec<FileState> = Vec::new();
    for outcome in &outcomes {
        classify(
            outcome,
            manifest,
            &mut writes,
            &mut file_problems,
            &mut states,
        )?;
    }

    // Directory assets are immutable after injection: anything we did not
    // render is not a user file, it is something that should not be there.
    let mut removes: Vec<Action> = Vec::new();
    let mut extra_problems: Vec<Problem> = Vec::new();
    for asset in &resolved {
        if asset.kind != AssetKind::Dir {
            continue;
        }
        for path in extra_files(project_root, &asset.asset_path, &asset.files)? {
            guard_ancestors(project_root, &path, &BTreeSet::new())?;
            removes.push(Action::Remove { path: path.clone() });
            extra_problems.push(Problem::new(Status::Extra, path, None));
        }
    }

    // Anything the manifest owns that we no longer produce goes whole.
    let mut orphan_problems: Vec<Problem> = Vec::new();
    if let Some(manifest) = manifest {
        let produced: BTreeSet<&Path> = resolved
            .iter()
            .map(|asset| asset.asset_path.as_path())
            .collect();
        for owned in &manifest.0 {
            if !produced.contains(owned.path.as_path()) {
                guard_ancestors(project_root, &owned.path, &BTreeSet::new())?;
                removes.push(Action::Remove {
                    path: owned.path.clone(),
                });
                orphan_problems.push(Problem::new(Status::Orphan, owned.path.clone(), None));
            }
        }
    }
    // JSON injections merge into files the project owns; they are never in
    // the manifest and never deleted.
    let (merges, inject_problems) = plan_injections(config, &resolved, project_root)?;

    // `check` reports asset obstacles first, then files, then extras,
    // orphans and injections.
    let mut problems = asset_problems;
    problems.extend(file_problems);
    problems.extend(extra_problems);
    problems.extend(orphan_problems);
    problems.extend(inject_problems);

    // `sync` works in the order that keeps the tree consistent: clear
    // obstacles, merge JSON, write files, then remove.
    let mut actions = prepares;
    actions.extend(merges);
    actions.extend(writes);
    actions.extend(removes);

    // Nobody owns these, so nobody touches them; `status` only counts them.
    let custom = custom_files(project_root, config, spec, &resolved, manifest)?;

    // In `tree` order, so `states` and `tree` walk together.
    states.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Plan {
        tree: tree_of(&resolved),
        manifest: manifest_of(&resolved),
        actions,
        problems,
        states,
        custom,
    })
}

/// The rendered tree, without looking at the project.
///
/// This is what `render` writes to `--out`: exactly what a clean `sync`
/// would produce, with no classification and no JSON injections.
pub fn expected_tree(
    config: &Config,
    registry: &Registry,
    spec: &Spec,
) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    Ok(tree_of(&resolve(config, registry, spec)?))
}

/// One import, rendered and placed.
struct Resolved {
    origin: Origin,
    asset_path: PathBuf,
    kind: AssetKind,
    files: Vec<(PathBuf, Vec<u8>)>,
}

/// Render every import into the project-relative paths it owns. This is
/// where name, destination and rename are turned into real paths, and where
/// the plan refuses to start if two imports would fight over one.
fn resolve(config: &Config, registry: &Registry, spec: &Spec) -> Result<Vec<Resolved>, String> {
    let mut resolved: Vec<Resolved> = Vec::new();

    for (category_name, category) in &config.0 {
        let root = category.effective_root(category_name, spec)?;
        for import in &category.imports {
            let asset = registry.load(category_name.as_str(), import.name.as_str())?;
            let kind = asset.kind();
            let target = import.target_dir(&root);
            let name = import.output_name().as_str();
            let asset_path = match kind {
                AssetKind::File => target.as_path().join(format!("{name}.md")),
                AssetKind::Dir => target.as_path().join(name),
            };

            let mut frontmatter = category.injections.frontmatter.clone();
            frontmatter.extend(import.injections.frontmatter.clone());

            let files = match render::render_asset(&asset, &frontmatter)? {
                Rendered::File(bytes) => vec![(asset_path.clone(), bytes)],
                Rendered::Dir(entries) => entries
                    .into_iter()
                    .map(|(relative, bytes)| (asset_path.join(relative), bytes))
                    .collect(),
            };

            resolved.push(Resolved {
                origin: Origin {
                    category: category_name.to_string(),
                    name: import.name.to_string(),
                },
                asset_path,
                kind,
                files,
            });
        }
    }

    reject_collisions(&resolved)?;
    Ok(resolved)
}

/// Two imports must never fight over one path, and one asset must never land
/// inside another — a directory asset is immutable, so a file dropped into it
/// would be `EXTRA` the moment it was written.
fn reject_collisions(resolved: &[Resolved]) -> Result<(), String> {
    let mut assets: BTreeSet<&Path> = BTreeSet::new();
    for asset in resolved {
        if !assets.insert(asset.asset_path.as_path()) {
            return Err(format!(
                "two imports emit the same output path: {}",
                asset.asset_path.display()
            ));
        }
    }
    for outer in resolved {
        for inner in resolved {
            if outer.asset_path != inner.asset_path
                && inner.asset_path.starts_with(&outer.asset_path)
            {
                return Err(format!(
                    "{} is emitted inside the asset {}",
                    inner.asset_path.display(),
                    outer.asset_path.display()
                ));
            }
        }
    }
    let mut files: BTreeSet<&Path> = BTreeSet::new();
    for asset in resolved {
        for (path, _) in &asset.files {
            if !files.insert(path.as_path()) {
                return Err(format!(
                    "two imports emit the same file: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// One planned file against the project tree.
fn classify(
    outcome: &Outcome,
    manifest: Option<&Manifest>,
    writes: &mut Vec<Action>,
    problems: &mut Vec<Problem>,
    states: &mut Vec<FileState>,
) -> Result<(), String> {
    let Outcome {
        path,
        origin,
        bytes,
        disk,
        blocked,
    } = outcome;

    let write = |kind: WriteKind| Action::Write {
        path: path.clone(),
        bytes: bytes.clone(),
        kind,
        origin: origin.clone(),
    };

    // A file under a blocked asset root still gets written — the obstacle
    // above it is cleared first — but the asset's own `DRIFT` already says
    // what is wrong, so it is not repeated per file. The state is recorded
    // either way: `status` must not call a file clean when the run is going
    // to rewrite it.
    let mut report = |status: Status, problems: &mut Vec<Problem>| {
        states.push(FileState {
            path: path.clone(),
            origin: origin.clone(),
            status,
        });
        if !blocked {
            problems.push(Problem::new(status, path.clone(), Some(origin.clone())));
        }
    };

    match disk {
        Disk::Absent => {
            writes.push(write(WriteKind::Fresh));
            report(Status::Missing, problems);
        }
        Disk::File(on_disk) if on_disk == bytes => {
            states.push(FileState {
                path: path.clone(),
                origin: origin.clone(),
                status: Status::Clean,
            });
        }
        Disk::File(on_disk) => {
            let written = hash_bytes(on_disk);
            let ours = manifest.and_then(|m| m.hash_of(path)) == Some(written.as_str());
            if ours {
                writes.push(write(WriteKind::Update));
                report(Status::Drift, problems);
            } else {
                writes.push(write(WriteKind::Conflict));
                report(Status::Conflict, problems);
            }
        }
        Disk::Obstacle => {
            // A symlink, or a thing of the wrong kind. `check` calls it
            // drift whatever we own; `sync` decides separately whether it is
            // ours to replace.
            writes.push(write(WriteKind::Fresh));
            report(Status::Drift, problems);
        }
    }
    Ok(())
}

/// What the JSON injections need from the project.
fn plan_injections(
    config: &Config,
    resolved: &[Resolved],
    project_root: &Path,
) -> Result<(Vec<Action>, Vec<Problem>), String> {
    let mut actions = Vec::new();
    let mut problems = Vec::new();

    for (target, content) in combine_injections(config)? {
        for asset in resolved {
            if target == asset.asset_path || target.starts_with(&asset.asset_path) {
                return Err(format!(
                    "injection target {} collides with the asset {}",
                    target.display(),
                    asset.asset_path.display()
                ));
            }
        }
        guard_ancestors(project_root, &target, &BTreeSet::new())?;

        let absolute = project_root.join(&target);
        let on_disk = match std::fs::read_to_string(&absolute) {
            Ok(text) => Some(
                serde_json::from_str::<Value>(&text)
                    .map_err(|e| format!("{} is not valid JSON: {e}", target.display()))?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("cannot read {}: {e}", target.display())),
        };

        let needs_work = match &on_disk {
            Some(existing) => !merge::is_noop(existing, &content),
            None => true,
        };
        if needs_work {
            let merged = on_disk
                .map(|existing| merge::merge(&existing, &content))
                .unwrap_or(content);
            actions.push(Action::MergeJson {
                path: target.clone(),
                content: merged,
            });
            problems.push(Problem::new(Status::Inject, target, None));
        }
    }

    Ok((actions, problems))
}

/// Fold every `injections.json` chunk aimed at one file into a single chunk,
/// in config order.
fn combine_injections(config: &Config) -> Result<Vec<(PathBuf, Value)>, String> {
    let mut by_target: BTreeMap<PathBuf, Vec<Value>> = BTreeMap::new();
    for category in config.0.values() {
        for chunk in &category.injections.json {
            let content = Value::Object(chunk.content.clone().into_iter().collect());
            by_target
                .entry(chunk.file.as_path().to_path_buf())
                .or_default()
                .push(content);
        }
    }
    Ok(by_target
        .into_iter()
        .map(|(target, chunks)| (target, merge::merge_chunks(chunks)))
        .collect())
}

// ------------------------------------------------------------- project tree

fn disk_file(absolute: &Path) -> Result<Disk, String> {
    let metadata = match std::fs::symlink_metadata(absolute) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Disk::Absent),
        Err(e) => return Err(format!("cannot inspect {}: {e}", absolute.display())),
    };
    if metadata.file_type().is_symlink() {
        return Ok(Disk::Obstacle);
    }
    if metadata.is_file() {
        let bytes = std::fs::read(absolute)
            .map_err(|e| format!("cannot read {}: {e}", absolute.display()))?;
        return Ok(Disk::File(bytes));
    }
    Ok(Disk::Obstacle)
}

/// `Some` when something that is not a real directory sits at `path`.
fn dir_obstacle(project_root: &Path, path: &Path) -> Result<Option<()>, String> {
    let absolute = project_root.join(path);
    let metadata = match std::fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot inspect {}: {e}", absolute.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(Some(()));
    }
    Ok(None)
}

/// Files and symlinks under a managed asset directory that we did not
/// render.
fn extra_files(
    project_root: &Path,
    asset_path: &Path,
    planned: &[(PathBuf, Vec<u8>)],
) -> Result<Vec<PathBuf>, String> {
    let absolute = project_root.join(asset_path);
    let Ok(metadata) = std::fs::symlink_metadata(&absolute) else {
        return Ok(Vec::new());
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(Vec::new());
    }

    let expected: BTreeSet<&Path> = planned.iter().map(|(path, _)| path.as_path()).collect();
    let mut found = Vec::new();
    collect_strays(&absolute, asset_path, &expected, &mut found)?;
    found.sort();
    Ok(found)
}

fn collect_strays(
    absolute: &Path,
    relative: &Path,
    expected: &BTreeSet<&Path>,
    found: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for name in files::read_dir(absolute)? {
        let child_relative = relative.join(&name);
        let child_absolute = absolute.join(&name);
        // Anything that is not a directory is a stray, symlinks included: a
        // link inside a managed asset is something we did not render.
        if files::is_real_dir(&child_absolute)? {
            collect_strays(&child_absolute, &child_relative, expected, found)?;
        } else if !expected.contains(child_relative.as_path()) {
            found.push(child_relative);
        }
    }
    Ok(())
}

/// Files under a category root that no import produced and no manifest row
/// owns: the project's own additions to a shared namespace. The engine never
/// touches them, so `status` only counts them.
fn custom_files(
    project_root: &Path,
    config: &Config,
    spec: &Spec,
    resolved: &[Resolved],
    previous: Option<&Manifest>,
) -> Result<Vec<PathBuf>, String> {
    let mut owned: BTreeSet<PathBuf> = resolved
        .iter()
        .map(|asset| asset.asset_path.clone())
        .collect();
    if let Some(previous) = previous {
        for row in &previous.0 {
            owned.insert(row.path.clone());
        }
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    for (category, entry) in &config.0 {
        roots.push(
            entry
                .effective_root(category, spec)?
                .as_path()
                .to_path_buf(),
        );
    }
    roots.sort();
    roots.dedup();

    let mut found = Vec::new();
    for root in roots {
        let absolute = project_root.join(&root);
        if !files::is_real_dir(&absolute)? {
            continue;
        }
        files::walk_files(&absolute, &root, &mut found)?;
    }
    found.retain(|path| {
        !owned
            .iter()
            .any(|asset| path == asset || path.starts_with(asset))
    });
    found.sort();
    found.dedup();
    Ok(found)
}

// --------------------------------------------------------------- projections

fn tree_of(resolved: &[Resolved]) -> Vec<(PathBuf, Vec<u8>)> {
    let mut tree: Vec<(PathBuf, Vec<u8>)> = resolved
        .iter()
        .flat_map(|asset| asset.files.iter().cloned())
        .collect();
    tree.sort_by(|a, b| a.0.cmp(&b.0));
    tree
}

fn manifest_of(resolved: &[Resolved]) -> Manifest {
    let assets = resolved
        .iter()
        .map(|asset| manifest::Asset {
            path: asset.asset_path.clone(),
            files: asset
                .files
                .iter()
                .map(|(path, bytes)| manifest::FileEntry {
                    path: path.clone(),
                    hash: hash_bytes(bytes),
                })
                .collect(),
        })
        .collect();
    Manifest(assets).sorted()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::report::Status;
    use crate::spec::Spec;
    use std::fs;
    use tempfile::TempDir;

    const RULE: &str = "## Summary\n\nKeep secrets safe.\n\n## Guidelines\n\n- A.\n";

    /// A throwaway project and registry, with one file asset and one
    /// directory asset available.
    struct World {
        _tmp: TempDir,
        registry: Registry,
        project: PathBuf,
        spec: Spec,
    }

    fn world() -> World {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path().join("assets");
        fs::create_dir_all(assets.join("rules")).unwrap();
        fs::create_dir_all(assets.join("skills/tdd")).unwrap();
        fs::write(assets.join("rules/security.md"), RULE).unwrap();
        fs::write(assets.join("skills/tdd/SKILL.md"), "## Description\n\nD.\n").unwrap();
        fs::write(assets.join("skills/tdd/checklist.md"), "- a\n").unwrap();

        let project = tmp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        World {
            _tmp: tmp,
            registry: Registry::with_defaults(assets, &Spec::parse().unwrap()),
            project,
            spec: Spec::parse().unwrap(),
        }
    }

    fn run(world: &World, config: &str, manifest: Option<&Manifest>) -> Plan {
        let config = config::parse(config).unwrap();
        plan(
            &config,
            &world.registry,
            &world.spec,
            manifest,
            &world.project,
        )
        .unwrap()
    }

    fn kinds(plan: &Plan) -> Vec<(&'static str, String)> {
        plan.problems
            .iter()
            .map(|p| (p.status.word(), p.path.display().to_string()))
            .collect()
    }

    fn write_kinds(plan: &Plan) -> Vec<(&'static str, String)> {
        plan.actions
            .iter()
            .filter_map(|a| match a {
                Action::Write { path, kind, .. } => {
                    let word = match kind {
                        WriteKind::Fresh => "WRITE",
                        WriteKind::Update => "UPDATE",
                        WriteKind::Conflict => "CONFLICT",
                    };
                    Some((word, path.display().to_string()))
                }
                _ => None,
            })
            .collect()
    }

    const CONFIG: &str = r#"{
        "rules": {
            "root": "./.opencode/rules",
            "imports": ["security"],
            "injections": {
                "json": [{ "file": "./opencode.json", "content": {"a": 1} }]
            }
        },
        "skills": { "root": "./.opencode/skills", "imports": ["tdd"] }
    }"#;

    // ------------------------------------------------------------- baseline

    #[test]
    fn a_first_run_writes_everything_and_creates_the_injection_target() {
        let world = world();
        let plan = run(&world, CONFIG, None);
        assert_eq!(
            kinds(&plan),
            vec![
                ("MISSING", ".opencode/rules/security.md".to_string()),
                ("MISSING", ".opencode/skills/tdd/SKILL.md".to_string()),
                ("MISSING", ".opencode/skills/tdd/checklist.md".to_string()),
                ("INJECT", "opencode.json".to_string()),
            ]
        );
        assert_eq!(
            write_kinds(&plan),
            vec![
                ("WRITE", ".opencode/rules/security.md".to_string()),
                ("WRITE", ".opencode/skills/tdd/SKILL.md".to_string()),
                ("WRITE", ".opencode/skills/tdd/checklist.md".to_string()),
            ]
        );
    }

    #[test]
    fn the_manifest_records_one_row_per_import_with_hashes() {
        let world = world();
        let plan = run(&world, CONFIG, None);
        assert_eq!(plan.manifest.0.len(), 2);
        let skill = &plan.manifest.0[1];
        assert_eq!(skill.path, PathBuf::from(".opencode/skills/tdd"));
        assert_eq!(skill.files.len(), 2);
        for file in &skill.files {
            assert!(file.hash.starts_with("sha256:"), "{}", file.hash);
        }
    }

    // -------------------------------------------------------------- writing

    #[test]
    fn once_written_the_plan_is_empty() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let second = run(&world, CONFIG, Some(&first.manifest));
        assert_eq!(kinds(&second), Vec::<(&str, String)>::new());
        assert_eq!(second.actions, Vec::<Action>::new());
    }

    #[test]
    fn a_changed_source_is_an_update() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(
            world.registry.root().unwrap().join("rules/security.md"),
            "## Summary\n\nNew text.\n\n## Guidelines\n\n- A.\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        assert_eq!(
            kinds(&second),
            vec![("DRIFT", ".opencode/rules/security.md".to_string())]
        );
        assert_eq!(
            write_kinds(&second),
            vec![("UPDATE", ".opencode/rules/security.md".to_string())]
        );
    }

    #[test]
    fn a_hand_edit_is_a_conflict_that_sync_will_not_touch() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(
            world.project.join(".opencode/rules/security.md"),
            "local hack\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        assert_eq!(
            kinds(&second),
            vec![("CONFLICT", ".opencode/rules/security.md".to_string())]
        );
        assert_eq!(
            write_kinds(&second),
            vec![("CONFLICT", ".opencode/rules/security.md".to_string())]
        );
    }

    #[test]
    fn force_turns_a_conflict_into_an_update() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(
            world.project.join(".opencode/rules/security.md"),
            "local hack\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        let lines = crate::apply::run(&second, &world.project, &first.manifest, true).unwrap();
        assert!(
            lines.iter().all(|line| line.status != Status::Conflict),
            "{lines:?}"
        );
        assert_eq!(
            fs::read_to_string(world.project.join(".opencode/rules/security.md")).unwrap(),
            RULE
        );
    }

    #[test]
    fn without_force_a_conflict_is_reported_and_left_alone() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(
            world.project.join(".opencode/rules/security.md"),
            "local hack\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        let lines = crate::apply::run(&second, &world.project, &first.manifest, false).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].status, Status::Conflict);
        assert_eq!(
            fs::read_to_string(world.project.join(".opencode/rules/security.md")).unwrap(),
            "local hack\n"
        );
    }

    // ------------------------------------------------------ unowned files

    #[test]
    fn custom_files_in_the_category_root_are_ignored() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(world.project.join(".opencode/rules/custom.md"), "mine\n").unwrap();
        fs::create_dir_all(world.project.join(".opencode/skills/mine")).unwrap();
        fs::write(
            world.project.join(".opencode/skills/mine/SKILL.md"),
            "mine\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        assert_eq!(kinds(&second), Vec::<(&str, String)>::new());
    }

    #[test]
    fn a_stray_file_inside_a_managed_asset_is_extra() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        fs::write(
            world.project.join(".opencode/skills/tdd/stray.md"),
            "not mine\n",
        )
        .unwrap();
        let second = run(&world, CONFIG, Some(&first.manifest));
        assert_eq!(
            kinds(&second),
            vec![("EXTRA", ".opencode/skills/tdd/stray.md".to_string())]
        );
    }

    // -------------------------------------------------------------- orphans

    #[test]
    fn a_dropped_import_becomes_an_orphan() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let without = CONFIG.replace(r#""imports": ["security"],"#, r#""imports": [],"#);
        let second = run(&world, &without, Some(&first.manifest));
        assert_eq!(
            kinds(&second),
            vec![("ORPHAN", ".opencode/rules/security.md".to_string())]
        );
    }

    #[test]
    fn orphans_are_skipped_entirely_without_a_manifest() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let without = CONFIG.replace(r#""imports": ["security"],"#, r#""imports": [],"#);
        let second = run(&world, &without, None);
        assert_eq!(kinds(&second), Vec::<(&str, String)>::new());
    }

    // ---------------------------------------------------------- injections

    #[test]
    fn an_injection_target_is_never_deleted() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let without = CONFIG.replace(r#""imports": ["security"],"#, r#""imports": [],"#);
        let second = run(&world, &without, Some(&first.manifest));
        assert!(
            !second.actions.iter().any(
                |a| matches!(a, Action::Remove { path } if path == Path::new("opencode.json"))
            )
        );
    }

    #[test]
    fn an_injection_target_that_overlaps_an_asset_is_refused() {
        let world = world();
        let bad = r#"{
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{ "file": "./.opencode/rules/security.md", "content": {"a": 1} }]
                }
            }
        }"#;
        let config = config::parse(bad).unwrap();
        let error = plan(&config, &world.registry, &world.spec, None, &world.project).unwrap_err();
        assert!(error.contains("collides with the asset"), "{error}");
    }

    // ----------------------------------------------------------- refusals

    #[test]
    fn two_imports_may_not_emit_the_same_path() {
        let world = world();
        let bad = r#"{
            "rules": {
                "root": "./.opencode/rules",
                "imports": [
                    {"name": "security", "rename": "same"},
                    {"name": "security", "rename": "same"}
                ]
            }
        }"#;
        let config = config::parse(bad).unwrap();
        let error = plan(&config, &world.registry, &world.spec, None, &world.project).unwrap_err();
        assert!(error.contains("same output path"), "{error}");
    }

    #[test]
    fn an_unowned_obstacle_is_refused_before_anything_is_written() {
        let world = world();
        fs::create_dir_all(world.project.join(".opencode/rules")).unwrap();
        std::os::unix::fs::symlink(
            world.project.join("outside.md"),
            world.project.join(".opencode/rules/security.md"),
        )
        .unwrap();

        let error = refuse(&world, None);
        assert!(error.contains("refusing to replace unowned"), "{error}");
        // The refusal is decided before the run, so nothing at all lands.
        assert!(!world.project.join("opencode.json").exists());
        assert!(!world.project.join("agent-assets.lock").exists());
    }

    #[test]
    fn an_owned_symlinked_asset_directory_is_replaced_and_not_followed() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let outside = world.project.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), "secret\n").unwrap();
        let target = world.project.join(".opencode/skills/tdd");
        fs::remove_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&outside, &target).unwrap();

        let planned = run(&world, CONFIG, Some(&first.manifest));
        assert!(planned.actions.iter().any(|a| matches!(
            a,
            Action::PrepareDir { path } if path == Path::new(".opencode/skills/tdd")
        )));
        // The files below the link are classified as fresh, never read
        // through it: the outside directory holds `SKILL.md` and reading it
        // would make this look like a conflict.
        assert!(
            planned
                .problems
                .iter()
                .all(|p| p.status != Status::Conflict),
            "{:?}",
            planned.problems
        );

        let lines = crate::apply::run(&planned, &world.project, &first.manifest, false).unwrap();
        assert!(!lines.is_empty());
        assert_eq!(
            fs::read_to_string(outside.join("SKILL.md")).unwrap(),
            "secret\n"
        );
        assert_eq!(
            fs::read_to_string(world.project.join(".opencode/skills/tdd/SKILL.md")).unwrap(),
            "## Description\n\nD.\n"
        );
    }

    // ------------------------------------------------------------ projection

    #[test]
    fn the_tree_is_sorted_and_matches_the_manifest() {
        let world = world();
        let plan = run(&world, CONFIG, None);
        let paths: Vec<_> = plan.tree.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from(".opencode/rules/security.md"),
                PathBuf::from(".opencode/skills/tdd/SKILL.md"),
                PathBuf::from(".opencode/skills/tdd/checklist.md"),
            ]
        );
        assert_eq!(plan.manifest.files().count(), plan.tree.len());
    }

    // ------------------------------------------------------- path safety

    /// Plan a run and refuse it, expecting a specific message. The refusal
    /// may come from the planner (a path it will not read through) or from
    /// `sync` (an obstacle it does not own).
    fn refuse(world: &World, manifest: Option<&Manifest>) -> String {
        let config = config::parse(CONFIG).unwrap();
        match plan(
            &config,
            &world.registry,
            &world.spec,
            manifest,
            &world.project,
        ) {
            Err(error) => error,
            Ok(planned) => {
                let previous = manifest.cloned().unwrap_or_default();
                crate::apply::run(&planned, &world.project, &previous, false).unwrap_err()
            }
        }
    }

    #[test]
    fn a_dangling_symlink_at_a_file_path_is_refused() {
        let world = world();
        fs::create_dir_all(world.project.join(".opencode/rules")).unwrap();
        std::os::unix::fs::symlink(
            world.project.join("ghost.md"),
            world.project.join(".opencode/rules/security.md"),
        )
        .unwrap();
        let error = refuse(&world, None);
        assert!(error.contains("refusing to replace unowned"), "{error}");
    }

    #[test]
    fn a_symlink_to_a_file_outside_the_project_is_refused_and_the_referent_is_untouched() {
        let world = world();
        let outside = world.project.join("..").join("outside.md");
        fs::write(&outside, "secret\n").unwrap();
        fs::create_dir_all(world.project.join(".opencode/rules")).unwrap();
        std::os::unix::fs::symlink(&outside, world.project.join(".opencode/rules/security.md"))
            .unwrap();

        let error = refuse(&world, None);
        assert!(error.contains("refusing to replace unowned"), "{error}");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "secret\n");
    }

    #[test]
    fn an_owned_symlink_at_a_file_path_is_replaced_not_followed() {
        let world = world();
        let first = run(&world, CONFIG, None);
        crate::apply::run(&first, &world.project, &Manifest::default(), false).unwrap();

        let referent = world.project.join("..").join("referent.md");
        fs::write(&referent, "secret\n").unwrap();
        let target = world.project.join(".opencode/rules/security.md");
        fs::remove_file(&target).unwrap();
        std::os::unix::fs::symlink(&referent, &target).unwrap();

        let second = run(&world, CONFIG, Some(&first.manifest));
        assert!(second.actions.iter().any(|a| matches!(
            a,
            Action::Write { path, kind: WriteKind::Fresh, .. }
                if path == Path::new(".opencode/rules/security.md")
        )));
        crate::apply::run(&second, &world.project, &first.manifest, false).unwrap();

        assert!(!target.is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), RULE);
        assert_eq!(fs::read_to_string(&referent).unwrap(), "secret\n");
    }

    #[test]
    fn an_unowned_symlink_at_a_directory_asset_path_is_refused_and_the_referent_is_untouched() {
        let world = world();
        let outside = world.project.join("..").join("outside-skill");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep.md"), "keep me\n").unwrap();
        fs::create_dir_all(world.project.join(".opencode/skills")).unwrap();
        std::os::unix::fs::symlink(&outside, world.project.join(".opencode/skills/tdd")).unwrap();

        let error = refuse(&world, None);
        assert!(error.contains("refusing to replace unowned"), "{error}");
        assert_eq!(
            fs::read_to_string(outside.join("keep.md")).unwrap(),
            "keep me\n"
        );
    }
}
