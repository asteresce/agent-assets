//! Consumer configuration.
//!
//! The JSON document a project writes to say which assets it imports and how
//! they are adapted for its agent. Parsing is strict: unknown keys are
//! rejected, every error names the offending JSON path, and path-like fields
//! must stay inside the project.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{Error as DeError, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::defaults::Defaults;

// ----------------------------------------------------------------- names

/// A single path segment: non-empty, no separator, no control characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Name(String);

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Name {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, String> {
        if raw.is_empty() {
            return Err("invalid asset name: ''".to_string());
        }
        if raw.contains('/') {
            return Err(format!("invalid asset name: {raw:?}"));
        }
        // A name becomes a path segment, so `.` and `..` would point at the
        // category directory or above it.
        if raw == "." || raw == ".." {
            return Err(format!("invalid asset name: {raw:?}"));
        }
        if raw.chars().any(char::is_control) {
            return Err(format!("invalid asset name: {raw:?}"));
        }
        Ok(Name(raw))
    }
}

// --------------------------------------------------------------- paths

/// A project-relative path: no absolute prefix, no `.` or `..` segment, no
/// control characters. Leading `./` and trailing `/` are stripped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RelPath(PathBuf);

impl RelPath {
    pub fn new(raw: &str) -> Result<Self, String> {
        validate_relpath(raw).map(RelPath)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, name: &str) -> Self {
        RelPath(self.0.join(name))
    }

    pub fn as_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl TryFrom<String> for RelPath {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, String> {
        RelPath::new(&raw)
    }
}

/// Strip one leading `./` and any trailing `/` — the same normalisation the
/// engine has always applied to computed output paths.
pub fn normalise_relpath(raw: &str) -> String {
    let mut p = raw;
    while let Some(rest) = p.strip_prefix("./") {
        p = rest;
    }
    p.trim_end_matches('/').to_string()
}

fn validate_relpath(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() {
        return Err("empty path".to_string());
    }
    if raw.chars().any(char::is_control) {
        return Err(format!("path contains control characters: {raw:?}"));
    }
    if raw.starts_with('/') {
        return Err(format!("path must be project-relative: {raw}"));
    }
    let normalised = normalise_relpath(raw);
    if normalised.is_empty() {
        return Err("empty path".to_string());
    }
    for segment in normalised.split('/') {
        if segment == "." || segment == ".." {
            return Err(format!("path must not contain '.' or '..': {raw}"));
        }
    }
    Ok(PathBuf::from(normalised))
}

// ------------------------------------------------------------ injections

/// Structured content the engine merges into a file the project owns.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Injections {
    /// Replaces any frontmatter the source asset carries.
    #[serde(default)]
    pub frontmatter: BTreeMap<String, Value>,
    /// Ensures these chunks are present in their target files.
    #[serde(default)]
    pub json: Vec<JsonChunk>,
}

/// One `injections.json` entry: ensure `content` is present in `file`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonChunk {
    pub file: RelPath,
    pub content: BTreeMap<String, Value>,
}

// ------------------------------------------------------------- category

/// One top-level config key: where and how a category of assets is emitted.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Category {
    /// Output directory; defaults to `defaults.rootPrefix` + the category.
    pub root: Option<RelPath>,
    pub imports: Vec<Import>,
    pub injections: Injections,
}

/// The same fields, as serde sees them. Kept separate so a wrong-shaped value
/// reports `expected an object` instead of naming a Rust type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryRepr {
    #[serde(default)]
    root: Option<RelPath>,
    #[serde(default)]
    imports: Vec<Import>,
    #[serde(default)]
    injections: Injections,
}

impl<'de> Deserialize<'de> for Category {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CategoryVisitor;

        impl<'de> Visitor<'de> for CategoryVisitor {
            type Value = Category;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }

            fn visit_map<A>(self, map: A) -> Result<Category, A::Error>
            where
                A: MapAccess<'de>,
            {
                let repr =
                    CategoryRepr::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Category {
                    root: repr.root,
                    imports: repr.imports,
                    injections: repr.injections,
                })
            }
        }

        deserializer.deserialize_map(CategoryVisitor)
    }
}

impl Category {
    /// The effective output root for `category`, honouring `defaults.rootPrefix`.
    pub fn effective_root(&self, category: &Name, defaults: &Defaults) -> Result<RelPath, String> {
        match &self.root {
            Some(root) => Ok(root.clone()),
            None => {
                let joined = Path::new(&defaults.root_prefix).join(category.as_str());
                RelPath::new(&joined.to_string_lossy())
            }
        }
    }
}

// --------------------------------------------------------------- import

/// One asset to emit. A bare string is shorthand for `{ "name": <string> }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub name: Name,
    pub rename: Option<Name>,
    pub destination: Option<RelPath>,
    pub injections: Injections,
}

impl Import {
    /// The directory this import is emitted into, under the category root.
    pub fn target_dir(&self, root: &RelPath) -> RelPath {
        match &self.destination {
            Some(destination) => RelPath::new(&format!("{root}/{destination}"))
                .unwrap_or_else(|_| root.join(&destination.as_str())),
            None => root.clone(),
        }
    }

    /// The output basename: `rename` when given, otherwise the asset name.
    pub fn output_name(&self) -> &Name {
        self.rename.as_ref().unwrap_or(&self.name)
    }
}

/// The map form of an import, as serde sees it. Kept separate for the same
/// reason `CategoryRepr` is: a derived `deny_unknown_fields` beats a
/// hand-rolled key loop, and the messages stay serde's.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRepr {
    name: Name,
    #[serde(default)]
    rename: Option<Name>,
    #[serde(default)]
    destination: Option<RelPath>,
    #[serde(default)]
    injections: Injections,
}

impl<'de> Deserialize<'de> for Import {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ImportVisitor;

        impl<'de> Visitor<'de> for ImportVisitor {
            type Value = Import;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or an object")
            }

            fn visit_str<E>(self, value: &str) -> Result<Import, E>
            where
                E: DeError,
            {
                let name = Name::try_from(value.to_string()).map_err(E::custom)?;
                Ok(Import {
                    name,
                    rename: None,
                    destination: None,
                    injections: Injections::default(),
                })
            }

            fn visit_map<A>(self, map: A) -> Result<Import, A::Error>
            where
                A: MapAccess<'de>,
            {
                let repr =
                    ImportRepr::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Import {
                    name: repr.name,
                    rename: repr.rename,
                    destination: repr.destination,
                    injections: repr.injections,
                })
            }
        }

        deserializer.deserialize_any(ImportVisitor)
    }
}

// ---------------------------------------------------------------- config

/// The whole config: one entry per asset category.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Config(pub BTreeMap<Name, Category>);

impl Config {
    pub fn get(&self, category: &str) -> Option<&Category> {
        self.0.get(&Name::try_from(category.to_string()).ok()?)
    }

    /// Read and parse a config file. A missing file is reported as such so
    /// the message distinguishes it from a parse failure.
    pub fn read(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("config not found: {}", path.display())
            } else {
                format!("cannot read config {}: {e}", path.display())
            }
        })?;
        parse(&text)
    }
}

/// Parse a config document, reporting the JSON path of any problem.
pub fn parse(input: &str) -> Result<Config, String> {
    let value: Value =
        serde_json::from_str(input).map_err(|e| format!("config is not valid JSON: {e}"))?;
    if !value.is_object() {
        return Err("config must be a JSON object".to_string());
    }
    let mut de = serde_json::Deserializer::from_str(input);
    let result: Result<Config, _> = serde_path_to_error::deserialize(&mut de);
    result.map_err(|e| {
        let path = e.path().to_string();
        let inner = e.into_inner();
        if path.is_empty() || path == "." {
            inner.to_string()
        } else {
            format!("{path}: {inner}")
        }
    })
}

// ----------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn err(json: &str) -> String {
        parse(json).expect_err("expected a parse error")
    }

    fn ok(json: &str) -> Config {
        parse(json).unwrap_or_else(|e| panic!("expected a valid config, got error: {e}"))
    }

    #[test]
    fn an_empty_config_is_valid() {
        assert!(ok("{}").0.is_empty());
    }

    #[test]
    fn a_bare_string_import_is_shorthand_for_its_name() {
        let cfg = ok(r#"{"rules": {"imports": ["security"]}}"#);
        let import = &cfg.0.values().next().unwrap().imports[0];
        assert_eq!(import.name.as_str(), "security");
        assert!(import.rename.is_none());
        assert!(import.destination.is_none());
    }

    #[test]
    fn a_full_import_round_trips() {
        let cfg = ok(r#"{
                "rules": {
                  "root": "./.opencode/rules",
                  "imports": [{
                    "name": "code-style",
                    "rename": "style",
                    "destination": "base",
                    "injections": {"frontmatter": {"owner": "import-team"}}
                  }],
                  "injections": {
                    "frontmatter": {"team": "platform"},
                    "json": [{"file": "./opencode.json", "content": {"a": 1}}]
                  }
                }
            }"#);
        let category = cfg.get("rules").unwrap();
        let import = &category.imports[0];
        assert_eq!(import.name.as_str(), "code-style");
        assert_eq!(import.output_name().as_str(), "style");
        assert_eq!(import.destination.as_ref().unwrap().as_str(), "base");
        assert_eq!(
            category.injections.frontmatter.get("team").unwrap(),
            &Value::String("platform".into())
        );
        assert_eq!(category.injections.json[0].file.as_str(), "opencode.json");
    }

    #[test]
    fn rename_defaults_to_the_asset_name() {
        let cfg = ok(r#"{"rules": {"imports": ["security"]}}"#);
        let import = &cfg.get("rules").unwrap().imports[0];
        assert_eq!(import.output_name().as_str(), "security");
    }

    #[test]
    fn unknown_category_keys_are_rejected() {
        let message = err(r#"{"rules": {"root": "x", "nope": 1}}"#);
        assert!(message.contains("unknown field"), "{message}");
        assert!(message.starts_with("rules."), "{message}");
    }

    #[test]
    fn unknown_import_keys_are_rejected() {
        let message = err(r#"{"rules": {"imports": [{"name": "a", "nope": 1}]}}"#);
        assert!(message.contains("unknown field"), "{message}");
        assert!(message.starts_with("rules.imports[0]"), "{message}");
    }

    #[test]
    fn unknown_top_level_keys_are_categories_not_errors() {
        assert!(ok(r#"{"anything": {}}"#).get("anything").is_some());
    }

    #[test]
    fn a_non_object_category_is_rejected_with_its_path() {
        let message = err(r#"{"rules": "oops"}"#);
        assert!(message.starts_with("rules:"), "{message}");
    }

    #[test]
    fn a_malformed_import_names_its_index() {
        let message = err(r#"{"rules": {"imports": ["ok", 42]}}"#);
        assert!(message.starts_with("rules.imports[1]"), "{message}");
    }

    #[test]
    fn a_missing_import_name_is_rejected() {
        let message = err(r#"{"rules": {"imports": [{"rename": "x"}]}}"#);
        assert!(message.contains("missing field"), "{message}");
    }

    #[test]
    fn a_non_object_frontmatter_is_rejected() {
        let message = err(r#"{"rules": {"injections": {"frontmatter": "oops"}}}"#);
        assert!(
            message.starts_with("rules.injections.frontmatter"),
            "{message}"
        );
    }

    #[test]
    fn a_non_array_json_injection_is_rejected() {
        let message = err(r#"{"rules": {"injections": {"json": {}}}}"#);
        assert!(message.starts_with("rules.injections.json"), "{message}");
    }

    #[test]
    fn a_json_chunk_needs_a_file_and_an_object_content() {
        let message = err(r#"{"rules": {"injections": {"json": [{"content": {}}]}}}"#);
        assert!(message.starts_with("rules.injections.json[0]"), "{message}");
    }

    #[test]
    fn absolute_paths_are_rejected() {
        let message = err(r#"{"rules": {"root": "/etc"}}"#);
        assert!(message.contains("project-relative"), "{message}");
    }

    #[test]
    fn parent_directory_segments_are_rejected() {
        let message = err(r#"{"rules": {"imports": [{"name": "a", "destination": "../.."}]}}"#);
        assert!(message.contains("must not contain"), "{message}");
        assert!(
            message.starts_with("rules.imports[0].destination"),
            "{message}"
        );
    }

    #[test]
    fn dot_segments_are_rejected() {
        let message = err(r#"{"rules": {"imports": [{"name": "a", "destination": "base/."}]}}"#);
        assert!(message.contains("must not contain"), "{message}");
    }

    #[test]
    fn control_characters_are_rejected() {
        let message = err("{\"rules\": {\"root\": \"./x\\u001f/evil\"}}");
        assert!(message.contains("control characters"), "{message}");
    }

    #[test]
    fn names_cannot_carry_a_separator() {
        let message = err(r#"{"rules": {"imports": ["a/b"]}}"#);
        assert!(message.contains("invalid asset name"), "{message}");
    }

    #[test]
    fn names_cannot_be_dot_or_dot_dot() {
        assert!(err(r#"{"rules": {"imports": ["."]}}"#).contains("invalid asset name"));
        assert!(err(r#"{"rules": {"imports": [".."]}}"#).contains("invalid asset name"));
    }

    #[test]
    fn leading_dot_slash_and_trailing_slash_are_stripped() {
        let cfg = ok(r#"{"rules": {"root": "./.opencode/rules/"}}"#);
        assert_eq!(
            cfg.get("rules").unwrap().root.as_ref().unwrap().as_str(),
            ".opencode/rules"
        );
    }

    #[test]
    fn a_non_object_config_is_rejected() {
        assert_eq!(err("[]"), "config must be a JSON object");
        assert_eq!(err("42"), "config must be a JSON object");
    }

    #[test]
    fn invalid_json_is_reported_as_such() {
        assert!(err("{ not json").contains("not valid JSON"));
    }

    #[test]
    fn the_default_root_is_the_declared_prefix_plus_the_category() {
        let defaults = Defaults::parse().unwrap();
        let name = Name::try_from("rules".to_string()).unwrap();
        let root = Category::default()
            .effective_root(&name, &defaults)
            .unwrap();
        assert_eq!(
            root.as_path()
                .file_name()
                .map(|s| s.to_string_lossy().into_owned()),
            Some(name.as_str().to_string())
        );
        assert!(
            root.as_str()
                .starts_with(&normalise_relpath(&defaults.root_prefix)),
            "root {} should sit under the declared prefix",
            root
        );
    }

    #[test]
    fn an_explicit_root_wins_over_the_default() {
        let defaults = Defaults::parse().unwrap();
        let category = ok(r#"{"rules": {"root": "./r"}}"#);
        let category = category.get("rules").unwrap();
        let name = Name::try_from("rules".to_string()).unwrap();
        assert_eq!(
            category.effective_root(&name, &defaults).unwrap().as_str(),
            "r"
        );
    }

    #[test]
    fn target_dir_joins_the_destination() {
        let cfg = ok(r#"{"rules": {"imports": [{"name": "a", "destination": "base"}]}}"#);
        let import = &cfg.get("rules").unwrap().imports[0];
        let root = RelPath::new(".opencode/rules").unwrap();
        assert_eq!(import.target_dir(&root).as_str(), ".opencode/rules/base");
    }

    #[test]
    fn target_dir_is_the_root_without_a_destination() {
        let cfg = ok(r#"{"rules": {"imports": ["a"]}}"#);
        let import = &cfg.get("rules").unwrap().imports[0];
        let root = RelPath::new(".opencode/rules").unwrap();
        assert_eq!(import.target_dir(&root), root);
    }
}
