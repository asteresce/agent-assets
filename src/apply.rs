//! Perform a plan's actions.
//!
//! The planner has already decided every refusal; this module only carries
//! them out. Writes are atomic and never follow a symlink out of the project.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::files::{guard_ancestors, install_file, is_under};
use crate::manifest::Manifest;
use crate::plan::{Action, Plan, WriteKind};
use crate::report::{Line, Status};

/// Carry out every action, returning one line per change actually made.
///
/// A `CONFLICT` is reported and left alone unless `force` is set; the count
/// of skipped conflicts is what makes `sync` exit non-zero, so an
/// unattended run cannot look successful while leaving work undone.
pub fn run(
    plan: &Plan,
    project_root: &Path,
    previous: &Manifest,
    force: bool,
) -> Result<Vec<Line>, String> {
    // Replacing what is on the disk is only ours to do where a previous run
    // put it there; deleting is decided by the plan against the assets it
    // manages, so that set is the union.
    let replaceable = owned_paths(previous);
    let removable: BTreeSet<PathBuf> = replaceable
        .iter()
        .cloned()
        .chain(plan.manifest.0.iter().map(|asset| asset.path.clone()))
        .collect();
    let mut lines = Vec::new();

    // Refuse before any write. An obstacle we do not own is the user's file,
    // and a run that half applies is worse than one that never starts.
    for action in &plan.actions {
        let (path, want_dir) = match action {
            Action::PrepareDir { path } => (path, true),
            Action::Write { path, .. } => (path, false),
            Action::Remove { .. } | Action::MergeJson { .. } => continue,
        };
        let absolute = project_root.join(path);
        if is_mismatch(&absolute, want_dir)? && !owns(&replaceable, path) {
            return Err(format!(
                "refusing to replace unowned {}",
                absolute.display()
            ));
        }
    }

    for action in &plan.actions {
        // The planner already ran this guard, but a tree can change between
        // planning and applying, and every mutation here is destructive.
        let path = match action {
            Action::PrepareDir { path } => path,
            Action::Write { path, .. } => path,
            Action::Remove { path } => path,
            Action::MergeJson { path, .. } => path,
        };
        guard_ancestors(project_root, path, &BTreeSet::new())?;

        match action {
            Action::PrepareDir { path } => {
                replace_obstacle(project_root, path, &replaceable, true)?;
                let absolute = project_root.join(path);
                std::fs::create_dir_all(&absolute)
                    .map_err(|e| format!("cannot create {}: {e}", absolute.display()))?;
            }

            Action::Write {
                path,
                bytes,
                kind,
                origin,
            } => {
                if *kind == WriteKind::Conflict && !force {
                    lines.push(Line::new(
                        Status::Conflict,
                        path.clone(),
                        Some(origin.clone()),
                    ));
                    continue;
                }
                let status = match kind {
                    WriteKind::Fresh => Status::Write,
                    // With `--force` a conflict becomes an ordinary rewrite.
                    WriteKind::Update | WriteKind::Conflict => Status::Update,
                };
                replace_obstacle(project_root, path, &replaceable, false)?;
                install_file(&project_root.join(path), bytes)?;
                lines.push(Line::new(status, path.clone(), None));
            }

            Action::Remove { path } => {
                remove_and_prune(project_root, path, &removable)?;
                lines.push(Line::new(Status::Remove, path.clone(), None));
            }

            Action::MergeJson { path, content } => {
                merge_json(project_root, path, content)?;
                lines.push(Line::new(Status::Merge, path.clone(), None));
            }
        }
    }

    Ok(lines)
}

/// Paths a manifest says a previous run created.
fn owned_paths(manifest: &Manifest) -> BTreeSet<PathBuf> {
    manifest.0.iter().map(|asset| asset.path.clone()).collect()
}

fn owns(owned: &BTreeSet<PathBuf>, path: &Path) -> bool {
    owned.iter().any(|asset| is_under(path, asset))
}

/// A path the manifest does not own is not ours to replace or remove. This
/// is the refusal every destructive action goes through first.
fn require_owned(
    owned: &BTreeSet<PathBuf>,
    project_root: &Path,
    path: &Path,
    verb: &str,
) -> Result<(), String> {
    if owns(owned, path) {
        Ok(())
    } else {
        Err(format!(
            "refusing to {verb} unowned {}",
            project_root.join(path).display()
        ))
    }
}

// ---------------------------------------------------------------- writes

/// Replace whatever sits at `path` when it is not the kind of thing we want
/// there. `rm -f` unlinks a symlink rather than following it, so a link out
/// of the project is never traversed.
fn replace_obstacle(
    project_root: &Path,
    path: &Path,
    owned: &BTreeSet<PathBuf>,
    want_dir: bool,
) -> Result<(), String> {
    let absolute = project_root.join(path);
    if !is_mismatch(&absolute, want_dir)? {
        return Ok(());
    }
    require_owned(owned, project_root, path, "replace")?;
    let metadata = std::fs::symlink_metadata(&absolute)
        .map_err(|e| format!("cannot inspect {}: {e}", absolute.display()))?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        std::fs::remove_file(&absolute)
            .map_err(|e| format!("cannot replace {}: {e}", absolute.display()))?;
    } else {
        std::fs::remove_dir_all(&absolute)
            .map_err(|e| format!("cannot replace {}: {e}", absolute.display()))?;
    }
    Ok(())
}

/// True when `absolute` is not the kind of thing the action wants there:
/// a symlink always counts, and so does a directory where a file belongs (or
/// the reverse).
fn is_mismatch(absolute: &Path, want_dir: bool) -> Result<bool, String> {
    let metadata = match std::fs::symlink_metadata(absolute) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("cannot inspect {}: {e}", absolute.display())),
    };
    if metadata.file_type().is_symlink() {
        return Ok(true);
    }
    Ok(if want_dir {
        !metadata.is_dir()
    } else {
        metadata.is_dir()
    })
}

/// Delete an owned path and prune any directories it leaves empty.
fn remove_and_prune(
    project_root: &Path,
    path: &Path,
    owned: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    require_owned(owned, project_root, path, "remove")?;
    let absolute = project_root.join(path);
    match std::fs::symlink_metadata(&absolute) {
        Ok(metadata) if metadata.file_type().is_symlink() => std::fs::remove_file(&absolute),
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(&absolute),
        Ok(_) => std::fs::remove_file(&absolute),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => return Err(format!("cannot remove {}: {e}", absolute.display())),
    }
    .map_err(|e| format!("cannot remove {}: {e}", absolute.display()))?;
    prune_empty_dirs(project_root, path);
    Ok(())
}

/// Walk upward removing empty directories, stopping at the project root.
fn prune_empty_dirs(project_root: &Path, path: &Path) {
    let mut current = path.parent();
    while let Some(directory) = current {
        if directory.as_os_str().is_empty() || directory == Path::new(".") {
            break;
        }
        if std::fs::remove_dir(project_root.join(directory)).is_err() {
            break;
        }
        current = directory.parent();
    }
}

// --------------------------------------------------------------- injections

/// Write the merged JSON at `path`, creating it if it is not there.
fn merge_json(project_root: &Path, path: &Path, content: &Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(content)
        .map_err(|e| format!("cannot serialise {}: {e}", path.display()))?;
    text.push('\n');
    install_file(&project_root.join(path), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pruning_empty_directories_stops_at_the_project_root() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("x/y/z")).unwrap();

        prune_empty_dirs(&project, Path::new("x/y/z/some.md"));
        assert!(project.is_dir(), "the project root must survive");
        assert!(!project.join("x").exists());
    }

    #[test]
    fn pruning_stops_at_the_first_directory_that_still_holds_something() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("x/y/z")).unwrap();
        std::fs::write(project.join("x/keep.md"), "mine\n").unwrap();

        prune_empty_dirs(&project, Path::new("x/y/z/some.md"));
        assert!(!project.join("x/y").exists());
        assert!(project.join("x").is_dir(), "x still holds keep.md");
    }

    // ----------------------------------------------------- what is ours

    fn owned(paths: &[&str]) -> BTreeSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    /// The refusal behind every destructive action. It is the only thing
    /// standing between a mis-declared path and deleting somebody's file, and
    /// the integration suite exercises it only through `sync`.
    #[test]
    fn a_path_the_manifest_does_not_own_is_refused() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("yours.md"), "not mine\n").unwrap();

        let manifest = owned(&["mine.md"]);
        assert!(require_owned(&manifest, &project, Path::new("mine.md"), "remove").is_ok());

        let error = require_owned(&manifest, &project, Path::new("yours.md"), "remove")
            .expect_err("an unowned path must be refused");
        assert!(error.contains("refusing to remove unowned"), "{error}");
        assert!(error.ends_with("yours.md"), "it names the path: {error}");
        assert!(project.join("yours.md").exists(), "and touches nothing");
    }

    /// A directory asset owns everything inside it, not just the asset path
    /// itself -- which is why dropping a whole skill directory works.
    #[test]
    fn everything_under_an_owned_asset_is_owned_as_well() {
        let manifest = owned(&["skills/tdd"]);

        assert!(owns(&manifest, Path::new("skills/tdd")));
        assert!(owns(&manifest, Path::new("skills/tdd/SKILL.md")));
        assert!(owns(&manifest, Path::new("skills/tdd/deep/nested.md")));

        assert!(!owns(&manifest, Path::new("skills")));
        assert!(
            !owns(&manifest, Path::new("skills/tdd-other")),
            "a sibling is not under it"
        );
        assert!(!owns(&manifest, Path::new("skills/tddx/SKILL.md")));
    }

    #[test]
    fn removing_something_unowned_is_refused_before_it_is_touched() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("mine")).unwrap();
        std::fs::write(project.join("yours.md"), "not mine\n").unwrap();

        let manifest = owned(&["mine"]);
        remove_and_prune(&project, Path::new("yours.md"), &manifest)
            .expect_err("an unowned path must be refused");
        assert!(project.join("yours.md").exists());
    }

    // ------------------------------------------------------- what we write

    #[test]
    fn a_missing_path_is_not_a_mismatch() {
        let dir = TempDir::new().unwrap();
        assert!(!is_mismatch(&dir.path().join("nothing"), false).unwrap());
        assert!(!is_mismatch(&dir.path().join("nothing"), true).unwrap());
    }

    #[test]
    fn the_wrong_kind_of_thing_at_a_path_is_a_mismatch() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file = root.join("file.md");
        let directory = root.join("directory");
        std::fs::write(&file, "x\n").unwrap();
        std::fs::create_dir(&directory).unwrap();

        assert!(
            !is_mismatch(&file, false).unwrap(),
            "a file where a file belongs"
        );
        assert!(
            !is_mismatch(&directory, true).unwrap(),
            "a directory where one belongs"
        );
        assert!(
            is_mismatch(&directory, false).unwrap(),
            "a directory where a file belongs"
        );
        assert!(
            is_mismatch(&file, true).unwrap(),
            "a file where a directory belongs"
        );
    }

    /// A symlink is never the kind of thing we want, whatever it points at:
    /// accepting one would mean following it out of the project.
    #[test]
    #[cfg(unix)]
    fn a_symlink_at_a_path_is_always_a_mismatch() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let target = root.join("target");
        let link = root.join("link");
        std::fs::write(&target, "x\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(is_mismatch(&link, false).unwrap());
        assert!(is_mismatch(&link, true).unwrap());
    }

    /// A link out of the project is unlinked, never traversed, so whatever it
    /// points at survives.
    #[test]
    #[cfg(unix)]
    fn removing_a_symlink_removes_the_link_and_leaves_the_referent() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(project.join("skills")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("precious.md"), "keep me\n").unwrap();
        std::os::unix::fs::symlink(&outside, project.join("skills/tdd")).unwrap();

        remove_and_prune(&project, Path::new("skills/tdd"), &owned(&["skills/tdd"])).unwrap();

        assert!(!project.join("skills/tdd").exists(), "the link is gone");
        assert!(
            outside.join("precious.md").exists(),
            "the referent is untouched"
        );
    }

    #[test]
    fn merge_json_creates_a_file_that_is_not_there() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();

        merge_json(
            &project,
            Path::new("opencode.json"),
            &serde_json::json!({"a": 1}),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(project.join("opencode.json")).unwrap(),
            "{\n  \"a\": 1\n}\n"
        );
    }

    #[test]
    fn merge_json_replaces_what_was_there() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("opencode.json"), "{ \"user\": 1 }\n").unwrap();

        // The merge already happened where the plan was built; this writes the
        // result, so an unrelated key that was there does not come back.
        merge_json(
            &project,
            Path::new("opencode.json"),
            &serde_json::json!({"a": 1}),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(project.join("opencode.json")).unwrap(),
            "{\n  \"a\": 1\n}\n"
        );
    }
}
