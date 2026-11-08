//! agent-assets — render, sync and check shared agent assets.
//!
//! The binary is a thin shell over `agent_assets`: parse arguments, resolve
//! the project and config, plan, then either report (`check`) or apply
//! (`sync`). `render` writes the expected tree somewhere else entirely.

use std::path::Path;
use std::process::ExitCode;

use agent_assets::cli::{self, Cli, Command};
use agent_assets::config::{Config, RelPath};
use agent_assets::manifest::{self, Manifest};
use agent_assets::plan;
use agent_assets::registry::Registry;
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

/// The exit code: 0 when there was nothing to say, 1 when there was.
fn run() -> Result<u8, String> {
    let args = Cli::parse_args();
    if args.common.force && !matches!(args.command, Command::Sync) {
        return Err("--force only applies to sync".to_string());
    }
    let spec = Spec::parse()?;

    let project = cli::resolve_project_root(&args, &spec)?;
    let config_path = cli::resolve_config_path(&args, &spec, &project)?;
    let config = Config::read(&config_path)?;

    let registry = match &args.common.assets {
        Some(root) => Registry::with_defaults(root.clone(), &spec),
        None => Registry::embedded_with_defaults(&spec),
    };
    let manifest_path = project.join(resolve_manifest(&args, &spec)?.as_path());
    let reporter = Reporter::new(args.common.color);

    match &args.command {
        Command::Render { out } => render(&config, &registry, &spec, out),

        Command::Check => {
            let previous = load_manifest(&manifest_path)?;
            let plan = plan::plan(&config, &registry, &spec, previous.as_ref(), &project)?;
            let lines = agent_assets::check::run(&plan);
            for line in &lines {
                reporter.print(line);
            }
            // Every line `check` prints is a problem.
            Ok(u8::from(!lines.is_empty()))
        }

        Command::Sync => {
            let previous = load_manifest(&manifest_path)?;
            let plan = plan::plan(&config, &registry, &spec, previous.as_ref(), &project)?;
            let lines = agent_assets::apply::run(
                &plan,
                &project,
                previous.as_ref().unwrap_or(&Manifest::default()),
                args.common.force,
            )?;
            for line in &lines {
                reporter.print(line);
            }
            manifest::store(&manifest_path, &plan.manifest)?;
            // Writing files is the point of `sync`; only a conflict it had to
            // leave alone means the run is incomplete.
            let conflicted = lines.iter().any(|line| line.status == Status::Conflict);
            Ok(u8::from(conflicted))
        }
    }
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
