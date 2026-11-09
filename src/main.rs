//! agent-assets — render, sync and check shared agent assets.
//!
//! The binary is a thin shell over `agent_assets`: parse arguments, resolve
//! the project and config, plan, then either report (`check`, `status`) or
//! apply (`sync`). `validate` reports config and registry problems without
//! writing anything, `render` writes the expected tree somewhere else
//! entirely, and `list` and `show` read the registry without a project at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use agent_assets::cli::{self, Cli, Command};
use agent_assets::config::{Config, RelPath};
use agent_assets::manifest::{self, Manifest};
use agent_assets::plan;
use agent_assets::registry::{Asset, AssetKind, Registry};
use agent_assets::report::{self, Reporter, Status};
use agent_assets::spec::Spec;
use agent_assets::validate;

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
                Some(open_valid_project(&args, &spec, &registry)?)
            } else {
                None
            };
            show(&registry, &spec, target, project.as_ref())
        }

        Command::Render { out } => {
            let project = open_valid_project(&args, &spec, &registry)?;
            render(&project.config, &registry, &spec, out)
        }

        Command::Check => {
            let project = open_valid_project(&args, &spec, &registry)?;
            let plan = plan_of(&project, &registry, &spec)?;
            let lines = agent_assets::check::run(&plan);
            for line in &lines {
                reporter.print(line);
            }
            // Every line `check` prints is a problem.
            Ok(u8::from(!lines.is_empty()))
        }

        Command::Status => {
            let project = open_valid_project(&args, &spec, &registry)?;
            let plan = plan_of(&project, &registry, &spec)?;
            for line in &agent_assets::check::status(&plan) {
                reporter.print(line);
            }
            println!("{}", agent_assets::check::summary(&plan));
            Ok(0)
        }

        Command::Validate => {
            let project = open_project(&args, &spec)?;
            let problems = validate::validate(&project.config, &registry, &spec);
            for problem in &problems {
                println!("{problem}");
            }
            Ok(u8::from(!problems.is_empty()))
        }

        Command::Sync => {
            let project = open_valid_project(&args, &spec, &registry)?;
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

/// The project, but only when its config is sound.
///
/// A config that cannot produce a well-defined run — an import that does not
/// resolve, two imports fighting over a path, an injection aimed at a file an
/// asset owns — is refused here with every reason at once, before anything is
/// written. Registry problems are not a gate: `validate` reports those, and a
/// malformed asset still syncs.
fn open_valid_project(args: &Cli, spec: &Spec, registry: &Registry) -> Result<Project, String> {
    let project = open_project(args, spec)?;
    let problems = validate::config_problems(&project.config, registry, spec);
    if problems.is_empty() {
        Ok(project)
    } else {
        Err(problems.join("\n"))
    }
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
            AssetKind::File => "file",
            AssetKind::Dir => "dir",
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
    let mut per_category: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &entries {
        *per_category.entry(entry.category.as_str()).or_default() += 1;
    }
    let per_category = per_category
        .iter()
        .map(|(category, n)| format!("{category}: {n}"))
        .collect::<Vec<_>>();
    println!(
        "{} · {total} bytes · {}",
        report::counted(entries.len(), "asset", "assets"),
        per_category.join(", ")
    );
    Ok(0)
}

/// `show` prints one asset: its source, or the files `sync` would write for it.
fn show(
    registry: &Registry,
    spec: &Spec,
    target: &str,
    project: Option<&Project>,
) -> Result<u8, String> {
    let (category, name) = cli::split_target(target)?;
    let asset = registry.load(category, name)?;

    let files: Vec<(PathBuf, Vec<u8>)> = match project {
        // `--rendered` is the emitted reality: every file `sync` would write
        // for this asset, wherever it lands. The planner resolves it, so the
        // placement, the frontmatter and the refusals are `sync`'s own
        // answers rather than a second implementation of them.
        Some(project) => {
            let files = plan::renders_for(&project.config, registry, spec, category, name)?;
            if files.is_empty() {
                return Err(format!(
                    "{target} is not imported; nothing would be written"
                ));
            }
            files
        }
        None => match &asset {
            Asset::File(file) => vec![(PathBuf::new(), file.bytes.clone())],
            Asset::Dir(dir) => dir.files.clone(),
        },
    };

    page(&blocks(&files))
}

/// One asset as something to read: a single document, or every file behind a
/// `==> <path> <==` header the way `head` and `tar` separate them. The path
/// is whatever namespace the caller used — registry-relative for a source,
/// output-relative for a render.
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
