//! Command-line interface.
//!
//! Flag *defaults* are never spelled out here: where a default comes from
//! `defaults.json` the help refers to the key (`defaults.config`, …) instead
//! of restating its value.

use std::env;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::defaults::Defaults;

// No `name`: clap derives it from the package name in Cargo.toml, so the
// binary cannot drift from the declaration.
#[derive(Debug, Parser)]
#[command(
    version,
    about = "Render, sync, check and validate shared agent assets",
    long_about = "Render, sync, check and validate shared agent assets.\n\n\
        sync renders the configured assets and reconciles them with the \
        project. check performs the same comparison and only reports. render \
        writes the expected asset tree to --out and touches nothing else. \
        validate reports every config and registry problem at once.\n\n\
        status is check without the filter: every path, matching or not. list \
        and show read the registry and need no project."
)]
pub struct Cli {
    #[command(flatten)]
    pub common: Common,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Args)]
pub struct Common {
    /// JSON config path (default `defaults.config`)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Asset registry root (default: the registry compiled into this binary)
    #[arg(long, global = true, value_name = "DIR")]
    pub assets: Option<PathBuf>,

    /// Project root to reconcile (default `defaults.project`)
    #[arg(long, global = true, value_name = "DIR")]
    pub project: Option<PathBuf>,

    /// Manifest path relative to --project (default `defaults.manifest`)
    #[arg(long, global = true, value_name = "PATH")]
    pub manifest: Option<PathBuf>,

    /// Replace human-edited owned files instead of reporting CONFLICT
    #[arg(long, global = true)]
    pub force: bool,

    /// When to colourise output: auto, always or never
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto")]
    pub color: ColorWhen,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Write the expected asset tree to DIR and nothing else
    Render {
        /// Destination tree
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },

    /// Write and update owned files, remove orphans, merge JSON
    Sync,

    /// Report problems; exit 1 if there are any
    Check,

    /// Report every path, matching or not; always exits 0
    Status,

    /// Report config and registry problems, all at once; exit 1 if any
    Validate,

    /// List the assets the registry holds
    List,

    /// Print one asset from the registry
    Show {
        /// The asset to print: `<category>/<name>`
        target: String,

        /// Print the rendered form — what sync would write — not the source
        #[arg(long)]
        rendered: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

impl Cli {
    pub fn parse_args() -> Self {
        <Self as Parser>::parse()
    }
}

/// Where the project lives.
///
/// `--project` wins. Otherwise, if the user pointed `--config` at a file, the
/// project is the directory holding it. Otherwise walk up from the current
/// directory looking for the declared config basename (as git does), and fall
/// back to [`default_root`] — the declared `defaults.project` — when nothing
/// is found.
pub fn resolve_project_root(args: &Cli, defaults: &Defaults) -> Result<PathBuf, String> {
    if let Some(project) = &args.common.project {
        return Ok(project.clone());
    }

    if let Some(config) = &args.common.config {
        let parent = config.parent().unwrap_or_else(|| Path::new(""));
        if !parent.as_os_str().is_empty() {
            return Ok(parent.to_path_buf());
        }
        return current_dir();
    }

    let basename = config_basename(defaults)?;
    let cwd = current_dir()?;
    let mut dir = cwd.clone();
    loop {
        if dir.join(&basename).is_file() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => return Ok(default_root(&cwd, defaults)),
        }
    }
}

/// The declared fallback root (`defaults.project`) under `cwd`: what the
/// project root is when neither a flag nor a discovered config says
/// otherwise. `.` is the current directory; any other value sits beneath it.
fn default_root(cwd: &Path, defaults: &Defaults) -> PathBuf {
    // Components normalise the trailing `.` away, so a `.` default yields
    // the directory itself rather than `/path/.`.
    cwd.join(&defaults.project).components().collect()
}

/// The config file to read: `--config`, or the declared default under the
/// project root.
pub fn resolve_config_path(
    args: &Cli,
    defaults: &Defaults,
    project: &Path,
) -> Result<PathBuf, String> {
    if let Some(config) = &args.common.config {
        return Ok(config.clone());
    }
    Ok(project.join(config_basename(defaults)?))
}

/// The file name the project's config is expected to carry.
pub fn config_basename(defaults: &Defaults) -> Result<PathBuf, String> {
    Path::new(&defaults.config)
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| format!("defaults.config has no file name: {}", defaults.config))
}

/// Split a `show` target into its category and its name.
pub fn split_target(target: &str) -> Result<(&str, &str), String> {
    let (category, name) = target
        .split_once('/')
        .ok_or_else(|| format!("expected <category>/<name>, got {target:?}"))?;
    if category.is_empty() || name.is_empty() || name.contains('/') {
        return Err(format!("expected <category>/<name>, got {target:?}"));
    }
    Ok((category, name))
}

fn current_dir() -> Result<PathBuf, String> {
    env::current_dir().map_err(|e| format!("cannot determine the current directory: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("agent-assets").chain(args.iter().copied()))
            .expect("arguments should parse")
    }

    #[test]
    fn subcommands_parse() {
        assert!(matches!(parse(&["check"]).command, Command::Check));
        assert!(matches!(parse(&["sync"]).command, Command::Sync));
        assert!(matches!(parse(&["status"]).command, Command::Status));
        assert!(matches!(parse(&["list"]).command, Command::List));
        assert!(matches!(
            parse(&["render", "--out", "x"]).command,
            Command::Render { .. }
        ));
    }

    #[test]
    fn show_takes_a_target_and_an_optional_rendered_flag() {
        assert!(matches!(
            parse(&["show", "rules/security"]).command,
            Command::Show {
                rendered: false,
                ..
            }
        ));
        assert!(matches!(
            parse(&["show", "--rendered", "skills/tdd"]).command,
            Command::Show { rendered: true, .. }
        ));
    }

    #[test]
    fn a_show_target_is_a_category_and_a_name() {
        assert_eq!(
            split_target("rules/security").unwrap(),
            ("rules", "security")
        );
        for bad in ["rules", "rules/a/b", "/security", "rules/", ""] {
            assert!(split_target(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn global_flags_work_after_the_subcommand() {
        let cli = parse(&["check", "--config", "c.json", "--force", "--color", "never"]);
        assert_eq!(
            cli.common.config.as_deref(),
            Some(std::path::Path::new("c.json"))
        );
        assert!(cli.common.force);
        assert_eq!(cli.common.color, ColorWhen::Never);
    }

    #[test]
    fn colour_defaults_to_auto() {
        assert_eq!(parse(&["check"]).common.color, ColorWhen::Auto);
    }

    #[test]
    fn render_requires_out() {
        assert!(Cli::try_parse_from(["agent-assets", "render"]).is_err());
    }

    #[test]
    fn explicit_project_wins() {
        let cli = parse(&[
            "check",
            "--project",
            "/tmp/p",
            "--config",
            "/elsewhere/c.json",
        ]);
        assert_eq!(
            resolve_project_root(&cli, &Defaults::parse().unwrap()).unwrap(),
            PathBuf::from("/tmp/p")
        );
    }

    #[test]
    fn config_parent_becomes_the_project_root() {
        let cli = parse(&["check", "--config", "/tmp/elsewhere/c.json"]);
        assert_eq!(
            resolve_project_root(&cli, &Defaults::parse().unwrap()).unwrap(),
            PathBuf::from("/tmp/elsewhere")
        );
    }

    #[test]
    fn bare_config_falls_back_to_the_current_directory() {
        let cli = parse(&["check", "--config", "c.json"]);
        let root = resolve_project_root(&cli, &Defaults::parse().unwrap()).unwrap();
        assert_eq!(root, env::current_dir().unwrap());
    }

    #[test]
    fn default_config_is_the_declared_one_under_the_project_root() {
        let cli = parse(&["check", "--project", "/tmp/p"]);
        let defaults = Defaults::parse().unwrap();
        let expected = Path::new("/tmp/p").join(config_basename(&defaults).unwrap());
        assert_eq!(
            resolve_config_path(&cli, &defaults, Path::new("/tmp/p")).unwrap(),
            expected
        );
    }

    #[test]
    fn explicit_config_is_taken_verbatim() {
        let cli = parse(&[
            "check",
            "--project",
            "/tmp/p",
            "--config",
            "/elsewhere/c.json",
        ]);
        let defaults = Defaults::parse().unwrap();
        assert_eq!(
            resolve_config_path(&cli, &defaults, Path::new("/tmp/p")).unwrap(),
            PathBuf::from("/elsewhere/c.json")
        );
    }

    #[test]
    fn a_bare_config_name_resolves_under_the_current_directory() {
        let cli = parse(&["check", "--config", "c.json"]);
        assert_eq!(
            resolve_project_root(&cli, &Defaults::parse().unwrap()).unwrap(),
            env::current_dir().unwrap()
        );
    }

    #[test]
    fn the_default_root_is_the_declared_project_under_the_cwd() {
        let mut defaults = Defaults::parse().unwrap();
        defaults.project = "work/tree".to_string();
        assert_eq!(
            default_root(Path::new("/somewhere"), &defaults),
            PathBuf::from("/somewhere/work/tree")
        );
    }

    #[test]
    fn a_dot_default_root_is_the_cwd_itself_not_a_trailing_dot() {
        let defaults = Defaults::parse().unwrap();
        assert_eq!(
            default_root(Path::new("/somewhere"), &defaults),
            PathBuf::from("/somewhere")
        );
    }
}
