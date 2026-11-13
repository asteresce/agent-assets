//! The registry's declaration.
//!
//! `assets/spec.json` is the single source of truth for the registry's
//! categories: one entry per category, its shape (`kind`, `extension`,
//! `entryFile`) and its section schema. It lives with the assets, travels
//! with `--assets`, and is embedded from `assets/` so a built binary carries
//! its registry's declaration too. This module reads that one file; nothing
//! else in the crate restates a value from it.
//!
//! The package's other declaration — the defaults — is [`crate::defaults`],
//! from `defaults.json`.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The registry's declaration, embedded at build time from `assets/`.
pub const ASSETS_SPEC_JSON: &str = include_str!("../assets/spec.json");

// These types mirror `assets/spec.json` field for field, and reject keys
// they do not know: a typo in the declaration must fail loudly rather than
// silently fall back to a default. Values come from the file; nothing here
// restates one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heading {
    pub name: String,
    #[serde(default)]
    pub required: bool,
}

/// Which shape a category's assets have. Declared, never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    File,
    Dir,
}

/// One category's declaration: the shape its assets have and the `##`
/// sections their entry files may carry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssetSpec {
    pub kind: AssetKind,
    /// Source suffix for a `file` category, without the dot. Required for
    /// `file`, forbidden for `dir`.
    #[serde(default)]
    pub extension: Option<String>,
    /// Entry file inside a `dir` category's asset directory. Required for
    /// `dir`, forbidden for `file`.
    #[serde(default)]
    pub entry_file: Option<String>,
    pub headings: Vec<Heading>,
}

/// Every category's declaration, keyed by category name, as `assets/spec.json`
/// declares it: the file's top-level keys are the categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSpecs(pub BTreeMap<String, AssetSpec>);

impl AssetSpecs {
    /// Parse a registry's declaration, `label` naming the file in errors.
    pub fn parse(text: &str, label: &str) -> Result<Self, String> {
        let map: BTreeMap<String, AssetSpec> =
            serde_json::from_str(text).map_err(|e| format!("{label} is not valid JSON: {e}"))?;
        let specs = AssetSpecs(map);
        specs.check(label)?;
        Ok(specs)
    }

    /// The declaration embedded from `assets/`.
    pub fn embedded() -> Result<Self, String> {
        Self::parse(ASSETS_SPEC_JSON, "the embedded assets/spec.json")
    }

    /// The declaration of `category`, when it has one.
    pub fn get(&self, category: &str) -> Option<&AssetSpec> {
        self.0.get(category)
    }

    /// Every declaration, in category order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &AssetSpec)> {
        self.0.iter()
    }

    /// Every way a declaration is inconsistent with itself: a shape that
    /// cannot name its source, or an empty schema. Empty when it is sound.
    fn check(&self, label: &str) -> Result<(), String> {
        for (category, spec) in &self.0 {
            match spec.kind {
                AssetKind::File => {
                    let Some(extension) = spec.extension.as_deref() else {
                        return Err(format!(
                            "{label}: {category}: a file category needs `extension`"
                        ));
                    };
                    if extension.is_empty() || extension.starts_with('.') || extension.contains('/')
                    {
                        return Err(format!(
                            "{label}: {category}: `extension` is a suffix without a dot or path, got {extension:?}"
                        ));
                    }
                    if spec.entry_file.is_some() {
                        return Err(format!(
                            "{label}: {category}: a file category has no `entryFile`"
                        ));
                    }
                }
                AssetKind::Dir => {
                    let Some(entry_file) = spec.entry_file.as_deref() else {
                        return Err(format!(
                            "{label}: {category}: a directory category needs `entryFile`"
                        ));
                    };
                    if entry_file.is_empty()
                        || entry_file.contains('/')
                        || entry_file.contains('\\')
                    {
                        return Err(format!(
                            "{label}: {category}: `entryFile` is a bare file name, got {entry_file:?}"
                        ));
                    }
                    if spec.extension.is_some() {
                        return Err(format!(
                            "{label}: {category}: a directory category has no `extension`"
                        ));
                    }
                }
            }
            if spec.headings.is_empty() {
                return Err(format!("{label}: {category}: empty section schema"));
            }
            if !spec.headings[0].required {
                return Err(format!(
                    "{label}: {category}: the first section must be required"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(kind: &str, extra: &str) -> String {
        format!(
            r#"{{ "rules": {{ "kind": "{kind}", {extra} "headings": [ {{ "name": "Summary", "required": true }} ] }} }}"#
        )
    }

    #[test]
    fn embedded_asset_specs_parse() {
        let specs = AssetSpecs::embedded().expect("embedded assets/spec.json should parse");
        assert!(!specs.0.is_empty());
    }

    #[test]
    fn a_file_category_needs_an_extension() {
        let message = AssetSpecs::parse(&declaration("file", ""), "test").unwrap_err();
        assert!(message.contains("needs `extension`"), "{message}");
    }

    #[test]
    fn a_directory_category_needs_an_entry_file() {
        let message = AssetSpecs::parse(&declaration("dir", ""), "test").unwrap_err();
        assert!(message.contains("needs `entryFile`"), "{message}");
    }

    #[test]
    fn entry_file_is_rejected_on_a_file_category() {
        let message = AssetSpecs::parse(
            &declaration("file", r#""extension": "md", "entryFile": "SKILL.md","#),
            "test",
        )
        .unwrap_err();
        assert!(message.contains("has no `entryFile`"), "{message}");
    }

    #[test]
    fn extension_is_rejected_on_a_directory_category() {
        let message = AssetSpecs::parse(
            &declaration("dir", r#""entryFile": "SKILL.md", "extension": "md","#),
            "test",
        )
        .unwrap_err();
        assert!(message.contains("has no `extension`"), "{message}");
    }

    #[test]
    fn an_extension_carries_no_dot() {
        let message =
            AssetSpecs::parse(&declaration("file", r#""extension": ".md","#), "test").unwrap_err();
        assert!(message.contains("without a dot"), "{message}");
    }

    #[test]
    fn an_entry_file_is_a_bare_file_name() {
        let message = AssetSpecs::parse(&declaration("dir", r#""entryFile": "a/b.md","#), "test")
            .unwrap_err();
        assert!(message.contains("bare file name"), "{message}");
    }

    #[test]
    fn every_category_schema_starts_with_a_required_section() {
        let specs = AssetSpecs::embedded().unwrap();
        for (category, spec) in specs.iter() {
            assert!(
                !spec.headings.is_empty(),
                "{category}: empty section schema"
            );
            assert!(
                spec.headings[0].required,
                "{category}: the first section must be required"
            );
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let message = AssetSpecs::parse(
            r#"{
                "rules": {
                    "kind": "file",
                    "extension": "md",
                    "sections": [{ "name": "Summary", "required": true }]
                }
            }"#,
            "test",
        )
        .unwrap_err();
        assert!(message.contains("unknown field"), "{message}");
    }

    #[test]
    fn embedded_declaration_matches_the_file_on_disk() {
        // Guards against shipping a binary built from a stale declaration.
        assert_eq!(ASSETS_SPEC_JSON, include_str!("../assets/spec.json"));
    }
}
