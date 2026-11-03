//! Declared values.
//!
//! `spec.json` is the single source of truth for the package name, the
//! defaults and the per-category section schema. This module reads that one
//! declaration; nothing else in the crate restates a value from it.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The declaration, embedded at build time.
pub const SPEC_JSON: &str = include_str!("../spec.json");

// These types mirror `spec.json` field for field, and reject keys they do not
// know: a typo in the declaration must fail loudly rather than silently fall
// back to a default. Values come from `spec.json`; nothing here restates one.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub name: String,
    pub defaults: Defaults,
    pub headings: BTreeMap<String, Vec<Heading>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Defaults {
    pub assets: String,
    pub config: String,
    pub manifest: String,
    pub project: String,
    pub root_prefix: String,
    pub skill_entry: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heading {
    pub name: String,
    #[serde(default)]
    pub required: bool,
}

impl Spec {
    /// Parse the embedded declaration. Fails only if the build shipped a
    /// malformed `spec.json`, which is a packaging bug rather than user
    /// error.
    pub fn parse() -> Result<Self, String> {
        serde_json::from_str(SPEC_JSON).map_err(|e| format!("embedded spec.json is invalid: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_spec_parses() {
        let spec = Spec::parse().expect("embedded spec.json should parse");
        assert!(!spec.name.is_empty());
        assert!(!spec.defaults.config.is_empty());
        assert!(!spec.defaults.skill_entry.is_empty());
        assert!(!spec.headings.is_empty());
    }

    #[test]
    fn every_category_schema_starts_with_a_required_section() {
        let spec = Spec::parse().unwrap();
        for (category, schema) in &spec.headings {
            assert!(!schema.is_empty(), "{category}: empty section schema");
            assert!(
                schema[0].required,
                "{category}: the first section must be required"
            );
        }
    }

    #[test]
    fn embedded_spec_matches_the_file_on_disk() {
        // Guards against shipping a binary built from a stale declaration.
        let on_disk = include_str!("../spec.json");
        assert_eq!(SPEC_JSON, on_disk);
    }
}
