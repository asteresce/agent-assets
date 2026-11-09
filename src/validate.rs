//! Is this config, and this registry, sound?
//!
//! `plan` reports the first thing that goes wrong and stops. This collects
//! them instead: a config with four mistakes should say all four, not one per
//! run. Two separate questions, deliberately kept apart:
//!
//! - [`config_problems`] — will a run be well-defined? Every command that
//!   touches the project refuses on these first.
//! - [`registry_problems`] — does each asset follow the section schema its
//!   category declares? Reported, never a block: a malformed asset is a
//!   quality problem in the registry, not in this project's config.

use crate::config::Config;
use crate::plan;
use crate::registry::Registry;
use crate::sections;
use crate::spec::Spec;

/// Everything that would make a run undefined: imports that do not resolve,
/// two imports fighting over a path, and injections aimed at a file an asset
/// owns. Empty when the config is sound.
pub fn config_problems(config: &Config, registry: &Registry, spec: &Spec) -> Vec<String> {
    plan::resolve(config, registry, spec)
        .err()
        .unwrap_or_default()
}

/// Every asset in the registry that does not follow its category's declared
/// section schema, as `category/name: problem`.
pub fn registry_problems(registry: &Registry, spec: &Spec) -> Vec<String> {
    let entries = match registry.list() {
        Ok(entries) => entries,
        Err(e) => return vec![e],
    };
    let mut problems = Vec::new();
    for entry in entries {
        let asset = match registry.load(entry.category.as_str(), entry.name.as_str()) {
            Ok(asset) => asset,
            Err(e) => {
                problems.push(e);
                continue;
            }
        };
        let Some(schema) = spec.headings.get(entry.category.as_str()) else {
            continue;
        };
        let text = String::from_utf8_lossy(asset.entry_bytes());
        for problem in sections::problems(&text, schema) {
            problems.push(format!("{}/{}: {problem}", entry.category, entry.name));
        }
    }
    problems
}

/// Both questions, config first, for `agent-assets validate`.
pub fn validate(config: &Config, registry: &Registry, spec: &Spec) -> Vec<String> {
    let mut problems = config_problems(config, registry, spec);
    problems.extend(registry_problems(registry, spec));
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
    fn fixture() -> (TempDir, Registry, Spec) {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path().join("assets");
        fs::create_dir_all(assets.join("rules")).unwrap();
        fs::write(assets.join("rules/security.md"), RULE).unwrap();
        fs::write(
            assets.join("rules/broken.md"),
            "## Summary\n\nNo guidelines.\n",
        )
        .unwrap();

        let spec = Spec::parse().expect("repo spec parses");
        let registry = Registry::with_defaults(&assets, &spec);
        (tmp, registry, spec)
    }

    fn config_from(json: &str) -> Config {
        config::parse(json).expect("test config parses")
    }

    #[test]
    fn a_sound_config_has_no_problems() {
        let (_tmp, registry, spec) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["security"]}}"#);

        assert_eq!(
            config_problems(&config, &registry, &spec),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_empty_config_is_sound() {
        let (_tmp, registry, spec) = fixture();

        assert_eq!(
            config_problems(&config_from("{}"), &registry, &spec),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_unresolvable_import_is_reported_not_raised() {
        let (_tmp, registry, spec) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["no-such-asset"]}}"#);

        let problems = config_problems(&config, &registry, &spec);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("no-such-asset"),
            "unexpected problem: {}",
            problems[0]
        );
    }

    #[test]
    fn several_mistakes_are_all_reported_at_once() {
        let (_tmp, registry, spec) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["missing-one", "missing-two"]}}"#);

        let problems = config_problems(&config, &registry, &spec);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("missing-one"), "{problems:?}");
        assert!(problems[1].contains("missing-two"), "{problems:?}");
    }

    #[test]
    fn an_injection_targeting_an_asset_is_a_config_problem() {
        let (_tmp, registry, spec) = fixture();
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

        let problems = config_problems(&config, &registry, &spec);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("collides with the asset"),
            "{}",
            problems[0]
        );
    }

    #[test]
    fn a_conformant_registry_says_nothing() {
        let (_tmp, registry, spec) = fixture();

        // `broken.md` is deliberately bad, so only `security` is conformant.
        let problems = registry_problems(&registry, &spec);
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
    fn the_shipped_registry_conforms_to_its_own_spec() {
        let spec = Spec::parse().expect("repo spec parses");
        let registry = Registry::embedded_with_defaults(&spec);

        assert_eq!(
            registry_problems(&registry, &spec),
            Vec::<String>::new(),
            "every asset must follow the section schema its category declares"
        );
    }

    #[test]
    fn validate_reports_both_kinds_of_problem() {
        let (_tmp, registry, spec) = fixture();
        let config = config_from(r#"{"rules": {"imports": ["missing-thing"]}}"#);

        let problems = validate(&config, &registry, &spec);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("missing-thing"), "{problems:?}");
        assert!(problems[1].starts_with("rules/broken: "), "{problems:?}");
    }
}
