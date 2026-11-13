//! Is this config, and this registry, sound?
//!
//! `plan` reports the first thing that goes wrong and stops. This collects
//! them instead: a config with four mistakes should say all four, not one per
//! run. Two separate questions, deliberately kept apart:
//!
//! - [`config_problems`] — will a run be well-defined? Every command that
//!   touches the project refuses on these first.
//! - [`registry_problems`] — does the registry match the declaration its
//!   `spec.json` makes, and does each asset follow its category's section
//!   schema? Reported, never a block: a malformed registry is a quality
//!   problem in the source, not in this project's config.

use std::path::Path;

use crate::config::Config;
use crate::defaults::Defaults;
use crate::plan;
use crate::registry::{AssetKind, Registry};
use crate::sections;

/// Everything that would make a run undefined: imports that do not resolve,
/// two imports fighting over a path, injections aimed at a file an asset
/// owns, and injection targets that cannot be read or parsed. Empty when the
/// config is sound.
pub fn config_problems(
    config: &Config,
    registry: &Registry,
    defaults: &Defaults,
    project_root: &Path,
) -> Vec<String> {
    let mut problems = plan::resolve(config, registry, defaults)
        .err()
        .unwrap_or_default();
    // A target that is not JSON would stop the run later, and the config is
    // the thing that aimed at it, so it belongs here with the rest.
    if let Err(errors) = plan::injection_targets(config, project_root) {
        problems.extend(errors);
    }
    problems
}

/// Everything in the registry that departs from its own `spec.json`: an
/// undeclared category, a shape the category does not declare, an asset
/// without its entry file, or `##` sections that depart from the schema.
/// Empty when the registry conforms.
pub fn registry_problems(registry: &Registry) -> Vec<String> {
    let found = match registry.inventory() {
        Ok(found) => found,
        Err(e) => return vec![e],
    };
    let specs = registry.specs();
    let mut problems = Vec::new();
    for entry in found {
        let Some(decl) = specs.get(&entry.category) else {
            problems.push(format!(
                "{}/{}: category is not declared in the registry's spec.json",
                entry.category, entry.child
            ));
            continue;
        };
        // The declaration decides what an asset is; anything else in the
        // category is a problem in the registry, not an asset to lint.
        let name = match (decl.kind, entry.shape) {
            (AssetKind::File, AssetKind::File) => {
                let extension = decl.extension.as_deref().unwrap_or("");
                let stem = (!extension.is_empty())
                    .then(|| entry.child.strip_suffix(&format!(".{extension}")))
                    .flatten();
                let Some(stem) = stem else {
                    problems.push(format!(
                        "{}/{}: not a .{extension} file, though {category} declares file assets",
                        entry.category,
                        entry.child,
                        category = entry.category
                    ));
                    continue;
                };
                stem.to_string()
            }
            (AssetKind::Dir, AssetKind::Dir) => entry.child.clone(),
            (AssetKind::File, AssetKind::Dir) => {
                problems.push(format!(
                    "{}/{}: a directory, but {category} declares file assets (.{extension})",
                    entry.category,
                    entry.child,
                    category = entry.category,
                    extension = decl.extension.as_deref().unwrap_or("")
                ));
                continue;
            }
            (AssetKind::Dir, AssetKind::File) => {
                problems.push(format!(
                    "{}/{}: a file, but {category} declares directory assets (entry {entry_file})",
                    entry.category,
                    entry.child,
                    category = entry.category,
                    entry_file = decl.entry_file.as_deref().unwrap_or("")
                ));
                continue;
            }
        };
        match registry.load(&entry.category, &name) {
            Ok(asset) => {
                let text = String::from_utf8_lossy(asset.entry_bytes());
                for problem in sections::problems(&text, &decl.headings) {
                    problems.push(format!("{}/{}: {problem}", entry.category, name));
                }
            }
            Err(e) => problems.push(e),
        }
    }
    problems
}

/// Both questions, config first, for `agent-assets validate`.
pub fn validate(
    config: &Config,
    registry: &Registry,
    defaults: &Defaults,
    project_root: &Path,
) -> Vec<String> {
    let mut problems = config_problems(config, registry, defaults, project_root);
    problems.extend(registry_problems(registry));
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::fs;
    use tempfile::TempDir;

    const RULE: &str = "## Summary\n\nKeep secrets safe.\n\n## Guidelines\n\n- A.\n";

    /// A throwaway registry: one conforming rule, one that is not.
    fn fixture() -> (TempDir, Registry, Defaults) {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path().join("assets");
        fs::create_dir_all(assets.join("rules")).unwrap();
        fs::write(
            assets.join("spec.json"),
            include_str!("../assets/spec.json"),
        )
        .unwrap();
        fs::write(assets.join("rules/security.md"), RULE).unwrap();
        fs::write(
            assets.join("rules/broken.md"),
            "## Summary\n\nNo guidelines.\n",
        )
        .unwrap();

        let defaults = Defaults::parse().expect("repo defaults parse");
        let registry = Registry::new(&assets).unwrap();
        (tmp, registry, defaults)
    }

    fn config_from(json: &str) -> Config {
        config::parse(json).expect("test config parses")
    }

    #[test]
    fn a_sound_config_has_no_problems() {
        let (tmp, registry, defaults) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["security"]}}"#);

        assert_eq!(
            config_problems(&config, &registry, &defaults, tmp.path()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_empty_config_is_sound() {
        let (tmp, registry, defaults) = fixture();

        assert_eq!(
            config_problems(&config_from("{}"), &registry, &defaults, tmp.path()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_unresolvable_import_is_reported_not_raised() {
        let (tmp, registry, defaults) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["no-such-asset"]}}"#);

        let problems = config_problems(&config, &registry, &defaults, tmp.path());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("no-such-asset"),
            "unexpected problem: {}",
            problems[0]
        );
    }

    #[test]
    fn several_mistakes_are_all_reported_at_once() {
        let (tmp, registry, defaults) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["missing-one", "missing-two"]}}"#);

        let problems = config_problems(&config, &registry, &defaults, tmp.path());
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("missing-one"), "{problems:?}");
        assert!(problems[1].contains("missing-two"), "{problems:?}");
    }

    #[test]
    fn an_injection_targeting_an_asset_is_a_config_problem() {
        let (tmp, registry, defaults) = fixture();
        let config = config_from(
            r#"{
                "rules": {
                    "root": "./.opencode/rules",
                    "imports": ["security"],
                    "injections": {
                        "json": [{ "file": "./.opencode/rules/security.md", "content": {"a": 1} }]
                    }
                }
            }"#,
        );

        let problems = config_problems(&config, &registry, &defaults, tmp.path());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("collides with the asset"),
            "{}",
            problems[0]
        );
    }

    /// The config is sound on its own; the file it aims at is the problem,
    /// and that is still a reason the run would stop.
    #[test]
    fn an_injection_target_that_is_not_json_is_a_config_problem() {
        let (tmp, registry, defaults) = fixture();
        fs::write(tmp.path().join("opencode.json"), "not json").unwrap();
        let config = config_from(
            r#"{
                "rules": {
                    "root": "./.opencode/rules",
                    "imports": ["security"],
                    "injections": {
                        "json": [{ "file": "./opencode.json", "content": {"a": 1} }]
                    }
                }
            }"#,
        );

        let problems = config_problems(&config, &registry, &defaults, tmp.path());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("is not valid JSON"),
            "unexpected problem: {}",
            problems[0]
        );
    }

    #[test]
    fn a_conformant_registry_says_nothing() {
        let (_tmp, registry, _spec) = fixture();

        // `broken.md` is deliberately bad, so only `security` is conformant.
        let problems = registry_problems(&registry);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with("rules/broken: "),
            "expected a rules/broken problem, got {:?}",
            problems[0]
        );
        assert!(
            problems[0].contains("missing required section 'Guidelines'"),
            "{}",
            problems[0]
        );
    }

    #[test]
    fn an_undeclared_category_is_a_registry_problem() {
        let (tmp, registry, _spec) = fixture();
        fs::create_dir_all(tmp.path().join("assets/commands")).unwrap();
        fs::write(tmp.path().join("assets/commands/ci.md"), "## Summary\nA\n").unwrap();

        let problems = registry_problems(&registry);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("category is not declared")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_wrong_shape_entry_is_a_registry_problem() {
        let (tmp, registry, _spec) = fixture();
        fs::create_dir_all(tmp.path().join("assets/rules/box")).unwrap();

        let problems = registry_problems(&registry);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("a directory, but rules declares file assets")),
            "{problems:?}"
        );
    }

    #[test]
    fn the_shipped_registry_conforms_to_its_own_spec() {
        let registry = Registry::embedded().unwrap();

        assert_eq!(
            registry_problems(&registry),
            Vec::<String>::new(),
            "every asset must follow the declaration its registry's spec.json makes"
        );
    }

    #[test]
    fn validate_reports_both_kinds_of_problem() {
        let (tmp, registry, defaults) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["missing-thing"]}}"#);

        let problems = validate(&config, &registry, &defaults, tmp.path());
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("missing-thing"), "{problems:?}");
        assert!(problems[1].starts_with("rules/broken: "), "{problems:?}");
    }
}
