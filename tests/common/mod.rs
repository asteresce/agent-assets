//! Shared scaffolding for the behaviour suite.
//!
//! Every test gets a throwaway registry and project tree and drives the
//! built binary through explicit `--config`, `--assets`, `--project` and
//! `--manifest` flags, so nothing leaks between them.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Value, json};
use tempfile::TempDir;

// The fixture assets. The fixture registry carries the shipped
// `assets/spec.json`, so `validate` checks these against the real schemas;
// `tests/registry.rs` is what conforms the shipped assets themselves.
pub const SECURITY: &str = "## Summary\n\nKeep secrets safe.\n";
pub const CODE_STYLE: &str = "## Summary\n\nPrefer clarity.\n";
pub const SKILL_MD: &str = "## Description\n\nTest first.\n";
pub const CHECKLIST: &str = "- red\n- green\n- refactor\n";
pub const BUILD: &str = "## Role\n\nBuilder.\n";

// The rendered form of SECURITY/CODE_STYLE under the base config.
pub const RULES_FM: &str = "---\nowner: default\nteam: platform\n---\n\n";
pub const STYLE_FM: &str = "---\nowner: import-team\nteam: platform\n---\n\n";

/// A throwaway registry and project tree.
pub struct World {
    pub tmp: TempDir,
    pub registry: PathBuf,
    pub project: PathBuf,
}

/// One run of the engine.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn ok(&self) -> &Run {
        assert_eq!(
            self.code, 0,
            "expected success\nstdout: {}\nstderr: {}",
            self.stdout, self.stderr
        );
        self
    }

    pub fn fails(&self) -> &Run {
        assert_ne!(self.code, 0, "expected failure\nstdout: {}", self.stdout);
        self
    }

    /// Assert the whole of stdout, ignoring stderr and the exit code.
    pub fn stdout_is(&self, expected: &str) -> &Run {
        assert_eq!(self.stdout, expected, "stdout differed");
        self
    }

    pub fn stderr_contains(&self, needle: &str) -> &Run {
        assert!(
            self.stderr.contains(needle),
            "{needle:?} missing from {:?}",
            self.stderr
        );
        self
    }
}

impl World {
    /// An empty registry and an empty config. The registry carries the
    /// shipped declaration, which every registry directory must.
    pub fn new() -> World {
        let tmp = TempDir::new().expect("temp dir");
        let registry = tmp.path().join("registry");
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&registry).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            registry.join("spec.json"),
            include_str!("../../assets/spec.json"),
        )
        .unwrap();
        World {
            tmp,
            registry,
            project,
        }
    }

    /// The fixture most tests start from: one rule, one renamed rule under a
    /// subdirectory, one directory asset with an artifact, one agent, and a
    /// JSON injection.
    pub fn fixture() -> World {
        let world = World::new();
        world.write_asset("rules/security.md", SECURITY);
        world.write_asset("rules/code-style.md", CODE_STYLE);
        world.write_asset("skills/tdd/SKILL.md", SKILL_MD);
        world.write_asset("skills/tdd/checklist.md", CHECKLIST);
        world.write_asset("agents/build.md", BUILD);
        world.write_config(&base_config());
        world
    }

    // ------------------------------------------------------------- files

    pub fn write_asset(&self, relative: &str, contents: &str) {
        self.write(&self.registry, relative, contents);
    }

    pub fn write_project(&self, relative: &str, contents: &str) {
        self.write(&self.project, relative, contents);
    }

    /// Take an asset out of the registry, as if upstream dropped it.
    pub fn remove_asset(&self, relative: &str) {
        std::fs::remove_file(self.registry.join(relative)).unwrap();
    }

    fn write(&self, root: &Path, relative: &str, contents: &str) {
        let target = root.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, contents).unwrap();
    }

    pub fn read_project(&self, relative: &str) -> String {
        std::fs::read_to_string(self.project.join(relative)).unwrap_or_else(|e| {
            panic!("cannot read {relative}: {e}");
        })
    }

    pub fn exists(&self, relative: &str) -> bool {
        std::fs::symlink_metadata(self.project.join(relative)).is_ok()
    }

    pub fn remove_project(&self, relative: &str) {
        std::fs::remove_file(self.project.join(relative)).unwrap();
    }

    pub fn write_config(&self, config: &Value) {
        std::fs::write(
            self.project.join("agent-assets.json"),
            format!("{config}\n"),
        )
        .unwrap();
    }

    /// The manifest the last sync left behind.
    pub fn manifest(&self) -> Value {
        let text = std::fs::read_to_string(self.project.join("agent-assets.lock"))
            .expect("no manifest was written");
        serde_json::from_str(&text).expect("manifest is not valid JSON")
    }

    pub fn manifest_text(&self) -> String {
        std::fs::read_to_string(self.project.join("agent-assets.lock")).expect("no manifest")
    }

    // ------------------------------------------------------------- runs

    /// Run the engine with explicit paths, so discovery never surprises a
    /// test.
    pub fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, "agent-assets.lock", &[])
    }

    /// `run` with a chosen manifest path and extra environment.
    pub fn run_with(&self, args: &[&str], manifest: &str, env: &[(&str, &str)]) -> Run {
        let mut command = Command::cargo_bin("agent-assets").expect("built binary");
        command
            .args(args)
            .args(["--config"])
            .arg(self.project.join("agent-assets.json"))
            .args(["--assets"])
            .arg(&self.registry)
            .args(["--project"])
            .arg(&self.project)
            .args(["--manifest"])
            .arg(manifest)
            .env_remove("NO_COLOR")
            .env_remove("AGENT_ASSETS_COLOR");
        for (key, value) in env {
            command.env(key, value);
        }
        let output = command.output().expect("run");
        Run {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    pub fn sync(&self) -> Run {
        self.run(&["sync"])
    }

    pub fn sync_forced(&self) -> Run {
        self.run(&["sync", "--force"])
    }

    pub fn check(&self) -> Run {
        self.run(&["check"])
    }

    pub fn render(&self, out: &Path) -> Run {
        self.run(&["render", "--out", &out.to_string_lossy()])
    }
}

/// The fixture config: two rules (one renamed into a subdirectory), a
/// directory asset, an agent, and a JSON injection.
pub fn base_config() -> Value {
    json!({
        "rules": {
            "root": "./.opencode/rules",
            "imports": [
                "security",
                {
                    "name": "code-style",
                    "rename": "style",
                    "destination": "base",
                    "injections": {"frontmatter": {"owner": "import-team"}}
                }
            ],
            "injections": {
                "frontmatter": {"team": "platform", "owner": "default"},
                "json": [
                    {
                        "file": "./opencode.json",
                        "content": {
                            "instructions": [".opencode/rules/**/*.md"],
                            "nested": {"a": 1}
                        }
                    }
                ]
            }
        },
        "skills": {"root": "./.opencode/skills", "imports": ["tdd"]},
        "agents": {"root": "./.opencode/agents", "imports": ["build"]}
    })
}

/// The emitted form of the fixture skill, for `SKILL.md`.
pub fn skill(relative_dir: &str) -> String {
    format!(".opencode/skills/{relative_dir}/SKILL.md")
}

/// The default output root, read from the declaration rather than restated.
pub fn default_root() -> String {
    let defaults = agent_assets::defaults::Defaults::parse().expect("defaults");
    agent_assets::config::normalise_relpath(&defaults.root_prefix)
}
