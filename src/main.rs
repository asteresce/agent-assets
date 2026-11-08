//! agent-assets — render, sync and check shared agent assets.
//!
//! The binary is a thin shell over `agent_assets`: parse arguments, resolve
//! the project and config, plan, then either report (`check`, `status`) or
//! apply (`sync`). `render` writes the expected tree somewhere else entirely,
//! and `list` and `show` read the registry without a project at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

use agent_assets::cli::{self, Cli, Command};
use agent_assets::config::{Config, RelPath};
use agent_assets::manifest::{self, Manifest};
use agent_assets::plan;
use agent_assets::registry::{Asset, Registry};
use agent_assets::render;
use agent_assets::report::{Reporter, Status};
use agent_assets::spec::Spec;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("agent-assets: error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Everything a command needs from the project: where it is, what it
/// declares, and where the manifest lives.
struct Project {
    root: PathBuf,
    config: Config,
    manifest: PathBuf,
}

/// The exit code: 0 when there was nothing to say, 1 when there was.
/// `status` is the exception: it always reports and always exits 0.
fn run() -> Result<u8, String> {
    let args = Cli::parse_args();
    if args.common.force && !matches!(args.command, Command::Sync) {
        return Err("--force only applies to sync".to_string());
    }
    let spec = Spec::parse()?;

    let registry = match &args.common.assets {
        Some(root) => Registry::with_defaults(root.clone(), &spec),
        None => Registry::embedded_with_defaults(&spec),
    };
    let reporter = Reporter::new(args.common.color);

    match &args.command {
        Command::List => list(&registry),

        Command::Show { target, rendered } => {
            // The rendered form depends on the project's injections, so only
            // `--rendered` needs one. The bare form reads the registry alone.
            let project = if *rendered {
                Some(open_project(&args, &spec)?)
            } else {
                None
            };
            show(&registry, target, project.as_ref())
        }

        Command::Render { out } => {
            let project = open_project(&args, &spec)?;
            render(&project.config, &registry, &spec, out)
        }

        Command::Check => {
            let project = open_project(&args, &spec)?;
            let plan = plan_of(&project, &registry, &spec)?;
            let lines = agent_assets::check::run(&plan);
            for line in &lines {
                reporter.print(line);
            }
            // Every line `check` prints is a problem.
            Ok(u8::from(!lines.is_empty()))
        }

        Command::Status => {
            let project = open_project(&args, &spec)?;
            let plan = plan_of(&project, &registry, &spec)?;
            for line in &agent_assets::check::status(&plan) {
                reporter.print(line);
            }
            println!("{}", agent_assets::check::summary(&plan));
            Ok(0)
        }

        Command::Sync => {
            let project = open_project(&args, &spec)?;
            let previous = load_manifest(&project.manifest)?;
            let plan = plan::plan(
                &project.config,
                &registry,
                &spec,
                previous.as_ref(),
                &project.root,
            )?;
            let lines = agent_assets::apply::run(
                &plan,
                &project.root,
                previous.as_ref().unwrap_or(&Manifest::default()),
                args.common.force,
            )?;
            for line in &lines {
                reporter.print(line);
            }
            manifest::store(&project.manifest, &plan.manifest)?;
            // Writing files is the point of `sync`; only a conflict it had to
            // leave alone means the run is incomplete.
            let conflicted = lines.iter().any(|line| line.status == Status::Conflict);
            Ok(u8::from(conflicted))
        }
    }
}

fn open_project(args: &Cli, spec: &Spec) -> Result<Project, String> {
    let root = cli::resolve_project_root(args, spec)?;
    let config_path = cli::resolve_config_path(args, spec, &root)?;
    let config = Config::read(&config_path)?;
    let manifest = root.join(resolve_manifest(args, spec)?.as_path());
    Ok(Project {
        root,
        config,
        manifest,
    })
}

fn plan_of(project: &Project, registry: &Registry, spec: &Spec) -> Result<plan::Plan, String> {
    let previous = load_manifest(&project.manifest)?;
    plan::plan(
        &project.config,
        registry,
        spec,
        previous.as_ref(),
        &project.root,
    )
}

/// The manifest from the previous run, or `None` when there is nothing to
/// reconcile against. An unreadable manifest is not fatal: `sync` warns and
/// carries on without orphan removal, rather than guessing.
fn load_manifest(path: &Path) -> Result<Option<Manifest>, String> {
    match manifest::load(path) {
        Ok(manifest) => Ok(manifest),
        Err(message) => {
            eprintln!("agent-assets: warning: {message}; skipping orphan removal");
            Ok(None)
        }
    }
}

/// `render` writes the expected tree and touches nothing else: no manifest,
/// no JSON injections, no project.
fn render(config: &Config, registry: &Registry, spec: &Spec, out: &Path) -> Result<u8, String> {
    for (path, bytes) in plan::expected_tree(config, registry, spec)? {
        let target = out.join(&path);
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        std::fs::write(&target, bytes)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    }
    Ok(0)
}

fn resolve_manifest(args: &Cli, spec: &Spec) -> Result<RelPath, String> {
    match &args.common.manifest {
        Some(path) => RelPath::new(&path.to_string_lossy()),
        None => RelPath::new(&spec.defaults.manifest),
    }
}

// ------------------------------------------------------------ the registry

/// `list` reads the registry and nothing else, so it works wherever the
/// binary lands.
fn list(registry: &Registry) -> Result<u8, String> {
    let entries = registry.list()?;
    let names: Vec<String> = entries
        .iter()
        .map(|entry| format!("{}/{}", entry.category, entry.name))
        .collect();
    let width = names.iter().map(String::len).max().unwrap_or(0);

    for (name, entry) in names.iter().zip(&entries) {
        let kind = match entry.kind {
            agent_assets::registry::AssetKind::File => "file",
            agent_assets::registry::AssetKind::Dir => "dir",
        };
        println!(
            "{name:<width$}  {kind:<4}  {:>6}  {}",
            entry.size,
            entry.summary,
            name = name,
            width = width
        );
    }

    let total: usize = entries.iter().map(|entry| entry.size).sum();
    let per_category: Vec<String> = {
        let mut seen: Vec<&str> = entries
            .iter()
            .map(|entry| entry.category.as_str())
            .collect();
        seen.sort();
        seen.dedup();
        seen.into_iter()
            .map(|category| {
                let n = entries
                    .iter()
                    .filter(|entry| entry.category == category)
                    .count();
                format!("{category}: {n}")
            })
            .collect()
    };
    println!(
        "{} · {total} bytes · {}",
        counted(entries.len(), "asset", "assets"),
        per_category.join(", ")
    );
    Ok(0)
}

/// `show` prints one asset: its source, or the form `sync` would write.
fn show(registry: &Registry, target: &str, project: Option<&Project>) -> Result<u8, String> {
    let (category, name) = cli::split_target(target)?;
    let asset = registry.load(category, name)?;

    let files: Vec<(PathBuf, Vec<u8>)> = match project {
        Some(project) => {
            let frontmatter = frontmatter_for(&project.config, category, name);
            match render::render_asset(&asset, &frontmatter)? {
                render::Rendered::File(bytes) => vec![(PathBuf::new(), bytes)],
                render::Rendered::Dir(entries) => entries,
            }
        }
        None => match &asset {
            Asset::File(file) => vec![(PathBuf::new(), file.bytes.clone())],
            Asset::Dir(dir) => dir.files.clone(),
        },
    };

    page(&blocks(&files))
}

/// The frontmatter `sync` would inject: the category's, then the first import
/// naming the asset — the one whose output the reader is looking at.
fn frontmatter_for(config: &Config, category: &str, name: &str) -> BTreeMap<String, Value> {
    let Some(entry) = config.get(category) else {
        return BTreeMap::new();
    };
    let mut out = entry.injections.frontmatter.clone();
    if let Some(import) = entry.imports.iter().find(|i| i.name.as_str() == name) {
        out.extend(import.injections.frontmatter.clone());
    }
    out
}

/// One asset as something to read: a single document, or every file behind a
/// `==> <rel> <==` header the way `head` and `tar` separate them.
fn blocks(files: &[(PathBuf, Vec<u8>)]) -> Vec<u8> {
    if files.len() == 1 {
        return files[0].1.clone();
    }
    let mut out = Vec::new();
    for (relative, bytes) in files {
        out.extend_from_slice(format!("==> {} <==\n", relative.display()).as_bytes());
        out.extend_from_slice(bytes);
        if !bytes.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.push(b'\n');
    }
    out
}

/// Send `bytes` to the reader: `bat` when it is there, then `less`, then
/// `cat`. Only a terminal gets a pager — a pipe takes the bytes as they are,
/// so `show` still composes with the rest of the shell.
fn page(bytes: &[u8]) -> Result<u8, String> {
    use std::io::{IsTerminal, Write};
    use std::process::{Command, Stdio};

    if std::io::stdout().is_terminal() {
        for (program, args) in [
            (
                "bat",
                vec!["--paging=always", "--style=plain", "--language=markdown"],
            ),
            ("less", vec!["-R"]),
            ("cat", vec![]),
        ] {
            let Ok(mut child) = Command::new(program)
                .args(&args)
                .stdin(Stdio::piped())
                .spawn()
            else {
                continue;
            };
            let mut stdin = child.stdin.take().expect("piped stdin");
            stdin
                .write_all(bytes)
                .map_err(|e| format!("cannot write to {program}: {e}"))?;
            drop(stdin);
            child
                .wait()
                .map_err(|e| format!("cannot run {program}: {e}"))?;
            return Ok(0);
        }
    }

    std::io::stdout()
        .write_all(bytes)
        .map_err(|e| format!("cannot write: {e}"))?;
    Ok(0)
}

fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
