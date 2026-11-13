//! The package's defaults.
//!
//! `defaults.json` is the single source of truth for the defaults
//! (`defaults.config`, `defaults.manifest`, …), embedded at build time —
//! the file *is* the defaults object, so a call site reads
//! `defaults.manifest` exactly as the file reads `manifest`. This module
//! reads that one declaration; nothing else in the crate restates a value
//! from it.
//!
//! The package's identity — name, description, version — is `Cargo.toml`'s
//! and is read through `env!`; it is deliberately not duplicated here.

use serde::Deserialize;

/// The declaration, embedded at build time.
pub const DEFAULTS_JSON: &str = include_str!("../defaults.json");

/// The defaults, as `defaults.json` declares them. Every field is read by
/// the engine: `config` and `project` by [`crate::cli`], `manifest` by the
/// sync commands, `rootPrefix` by [`crate::config`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Defaults {
    pub config: String,
    pub manifest: String,
    pub project: String,
    pub root_prefix: String,
}

impl Defaults {
    /// Parse the embedded declaration. Fails only if the build shipped a
    /// malformed `defaults.json`, which is a packaging bug rather than user
    /// error.
    pub fn parse() -> Result<Self, String> {
        serde_json::from_str(DEFAULTS_JSON)
            .map_err(|e| format!("embedded defaults.json is invalid: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_defaults_parse() {
        let defaults = Defaults::parse().expect("embedded defaults.json should parse");
        assert!(!defaults.config.is_empty());
        assert!(!defaults.manifest.is_empty());
        assert!(!defaults.project.is_empty());
        assert!(!defaults.root_prefix.is_empty());
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        let message = serde_json::from_str::<Defaults>(r#"{ "name": "agent-assets" }"#)
            .unwrap_err()
            .to_string();
        assert!(message.contains("unknown field `name`"), "{message}");
        assert!(message.contains("`manifest`"), "{message}");
    }

    #[test]
    fn a_missing_key_is_rejected() {
        serde_json::from_str::<Defaults>(r#"{ "config": "c", "manifest": "m" }"#)
            .expect_err("every default must be declared");
    }

    #[test]
    fn embedded_declaration_matches_the_file_on_disk() {
        // Guards against shipping a binary built from a stale declaration.
        assert_eq!(DEFAULTS_JSON, include_str!("../defaults.json"));
    }
}
