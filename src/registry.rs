//! The asset registry.
//!
//! Entries come from either the table compiled into the binary or a directory
//! laid out as `<root>/<category>/`, and come in two shapes: a single Markdown
//! file (`<name>.md`) or a directory (`<name>/`) whose entry file carries the
//! sections and whose other files are free-form artifacts. The engine infers
//! which shape it is from the layout, so adding an asset needs no
//! registration.
//!
//! Loading reads the bytes eagerly, so an asset is a pure value afterwards:
//! rendering, `show` and `list` all work off it without touching a filesystem.

use std::path::{Path, PathBuf};

use crate::config::Name;
use crate::spec::Spec;

/// Which shape a registry entry has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    File,
    Dir,
}

/// A single-file asset: one Markdown document, emitted as one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAsset {
    /// The source document, exactly as it sits in the registry.
    pub bytes: Vec<u8>,
}

/// A directory asset: an entry file plus free-form artifacts, emitted as a
/// directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirAsset {
    /// Entry file, relative to the asset root. This is the one that receives
    /// injected frontmatter.
    pub entry: PathBuf,
    /// Every file in the asset, relative to the asset root, sorted, including
    /// `entry`, with its bytes.
    pub files: Vec<(PathBuf, Vec<u8>)>,
}

impl DirAsset {
    /// True when `rel` is the entry file.
    pub fn is_entry(&self, rel: &Path) -> bool {
        rel == self.entry
    }

    /// Every file except the entry, in the same order as `files`.
    pub fn artifacts(&self) -> impl Iterator<Item = (&Path, &[u8])> {
        self.files
            .iter()
            .filter(|(rel, _)| *rel != self.entry)
            .map(|(rel, bytes)| (rel.as_path(), bytes.as_slice()))
    }

    /// The bytes of `rel`, when the asset carries it.
    pub fn file(&self, rel: &Path) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|(path, _)| path == rel)
            .map(|(_, bytes)| bytes.as_slice())
    }
}

/// A loaded registry entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asset {
    File(FileAsset),
    Dir(DirAsset),
}

impl Asset {
    pub fn kind(&self) -> AssetKind {
        match self {
            Asset::File(_) => AssetKind::File,
            Asset::Dir(_) => AssetKind::Dir,
        }
    }

    /// Files the asset carries: one for a file asset, entry plus artifacts
    /// for a directory asset.
    pub fn file_count(&self) -> usize {
        match self {
            Asset::File(_) => 1,
            Asset::Dir(dir) => dir.files.len(),
        }
    }

    /// Total bytes across those files.
    pub fn total_size(&self) -> usize {
        match self {
            Asset::File(file) => file.bytes.len(),
            Asset::Dir(dir) => dir.files.iter().map(|(_, bytes)| bytes.len()).sum(),
        }
    }

    /// The document that carries the sections: the file itself, or the
    /// directory's entry file.
    pub fn entry_bytes(&self) -> &[u8] {
        match self {
            Asset::File(file) => &file.bytes,
            Asset::Dir(dir) => dir.file(&dir.entry).unwrap_or(&[]),
        }
    }
}

// ------------------------------------------------------------- the sources

/// One file of an entry of the registry compiled into the binary, as
/// `build.rs` emits it.
pub struct EmbeddedFile {
    /// Path relative to the asset root.
    pub rel: &'static str,
    pub bytes: &'static [u8],
}

/// One entry of the registry compiled into the binary.
pub struct EmbeddedAsset {
    pub category: &'static str,
    pub name: &'static str,
    pub kind: AssetKind,
    /// Sorted; a file asset carries exactly one.
    pub files: &'static [EmbeddedFile],
}

mod embedded {
    use super::{AssetKind, EmbeddedAsset, EmbeddedFile};

    include!(concat!(env!("OUT_DIR"), "/embedded_registry.rs"));
}

/// Where a registry's entries live.
#[derive(Debug, Clone)]
enum Source {
    /// A directory laid out as `<root>/<category>/<name>`.
    Directory { root: PathBuf },
    /// The table `build.rs` compiled in. This is the default, so a downloaded
    /// or `cargo install`ed binary is self-contained.
    Embedded,
}

/// The name a category reserves for its catalog index: documentation about
/// the category, not an importable asset. `tests/registry.rs` makes the same
/// call when it conforms the registry.
fn is_catalog_index(name: &str) -> bool {
    name == "README"
}

/// One importable entry, as `list` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub category: String,
    pub name: String,
    pub kind: AssetKind,
    /// Files the asset carries: one for a file asset, entry plus artifacts
    /// for a directory asset.
    pub files: usize,
    /// Total bytes across those files.
    pub size: usize,
    /// First non-blank line of the entry file's first `##` section.
    pub summary: String,
}

impl Entry {
    fn of(category: &str, name: &str, asset: &Asset) -> Entry {
        Entry {
            category: category.to_string(),
            name: name.to_string(),
            kind: asset.kind(),
            files: asset.file_count(),
            size: asset.total_size(),
            summary: summary_of(asset.entry_bytes()),
        }
    }
}

/// The first non-blank line inside the first `##` section. Assets carry no
/// preamble and their first section is required, so this is always defined
/// for a conforming asset.
fn summary_of(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut in_section = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return trimmed.to_string();
    }
    String::new()
}

/// A registry of shared assets.
#[derive(Debug, Clone)]
pub struct Registry {
    source: Source,
    skill_entry: String,
}

impl Registry {
    pub fn new(root: impl Into<PathBuf>, skill_entry: impl Into<String>) -> Self {
        Registry {
            source: Source::Directory { root: root.into() },
            skill_entry: skill_entry.into(),
        }
    }

    /// Build from the declaration, so the entry-file convention comes from
    /// `spec.json` rather than being restated here.
    pub fn with_defaults(root: impl Into<PathBuf>, spec: &Spec) -> Self {
        Registry::new(root, spec.defaults.skill_entry.clone())
    }

    /// The registry compiled into this binary.
    pub fn embedded(skill_entry: impl Into<String>) -> Self {
        Registry {
            source: Source::Embedded,
            skill_entry: skill_entry.into(),
        }
    }

    /// The compiled-in registry, with the entry-file convention from the
    /// declaration.
    pub fn embedded_with_defaults(spec: &Spec) -> Self {
        Registry::embedded(spec.defaults.skill_entry.clone())
    }

    /// The directory this registry reads, when it reads one.
    pub fn root(&self) -> Option<&Path> {
        match &self.source {
            Source::Directory { root } => Some(root.as_path()),
            Source::Embedded => None,
        }
    }

    /// Load one asset. Exactly one of `<name>.md` and `<name>/` may exist.
    pub fn load(&self, category: &str, name: &str) -> Result<Asset, String> {
        let category = Name::try_from(category.to_string())?;
        let name = Name::try_from(name.to_string())?;
        if is_catalog_index(name.as_str()) {
            return Err(format!(
                "asset not found: {category}/{name} (a catalog index is not an asset)"
            ));
        }
        match &self.source {
            Source::Directory { root } => self.load_directory(root, &category, &name),
            Source::Embedded => self.load_embedded(&category, &name),
        }
    }

    /// Every importable entry, in `(category, name)` order.
    pub fn list(&self) -> Result<Vec<Entry>, String> {
        let mut out = Vec::new();
        for (category, name) in self.names()? {
            let asset = self.load(&category, &name)?;
            out.push(Entry::of(&category, &name, &asset));
        }
        Ok(out)
    }

    /// Every `(category, name)` the source holds, sorted. A name `load` would
    /// refuse is not listed, so `list` and `load` always agree.
    fn names(&self) -> Result<Vec<(String, String)>, String> {
        let mut out: Vec<(String, String)> = match &self.source {
            Source::Directory { root } => names_of(root)?,
            Source::Embedded => embedded::EMBEDDED
                .iter()
                .map(|asset| (asset.category.to_string(), asset.name.to_string()))
                .collect(),
        };
        out.retain(|(_, name)| !is_catalog_index(name));
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn load_directory(&self, root: &Path, category: &Name, name: &Name) -> Result<Asset, String> {
        let category_dir = root.join(category.as_str());
        let file_path = category_dir.join(format!("{}.md", name.as_str()));
        let dir_path = category_dir.join(name.as_str());

        let has_file = file_path.is_file();
        let has_dir = dir_path.is_dir();

        match (has_file, has_dir) {
            (true, false) => Ok(Asset::File(FileAsset {
                bytes: read_bytes(&file_path)?,
            })),
            (false, true) => {
                let entry = PathBuf::from(self.skill_entry.clone());
                let entry_path = dir_path.join(&entry);
                if !entry_path.is_file() {
                    return Err(format!(
                        "asset {category}/{name} has no {} entry file (expected {})",
                        self.skill_entry,
                        entry_path.display()
                    ));
                }
                let mut files = Vec::new();
                for relative in walk_files(&dir_path)? {
                    let absolute = dir_path.join(&relative);
                    files.push((relative, read_bytes(&absolute)?));
                }
                Ok(Asset::Dir(DirAsset { entry, files }))
            }
            (true, true) => Err(format!(
                "asset {category}/{name} is ambiguous: both {} and {} exist",
                file_path.display(),
                dir_path.display()
            )),
            (false, false) => Err(format!(
                "asset not found: {category}/{name} (expected {} or {})",
                file_path.display(),
                dir_path.display()
            )),
        }
    }

    fn load_embedded(&self, category: &Name, name: &Name) -> Result<Asset, String> {
        let Some(asset) = embedded::EMBEDDED
            .iter()
            .find(|asset| asset.category == category.as_str() && asset.name == name.as_str())
        else {
            return Err(format!(
                "asset not found: {category}/{name} (not in the registry compiled into this binary)"
            ));
        };

        match asset.kind {
            AssetKind::File => Ok(Asset::File(FileAsset {
                bytes: asset.files[0].bytes.to_vec(),
            })),
            AssetKind::Dir => {
                let entry = PathBuf::from(self.skill_entry.clone());
                let files: Vec<(PathBuf, Vec<u8>)> = asset
                    .files
                    .iter()
                    .map(|file| (PathBuf::from(file.rel), file.bytes.to_vec()))
                    .collect();
                if !files.iter().any(|(rel, _)| *rel == entry) {
                    return Err(format!(
                        "asset {category}/{name} has no {} entry file",
                        self.skill_entry
                    ));
                }
                Ok(Asset::Dir(DirAsset { entry, files }))
            }
        }
    }
}

/// Every `(category, name)` a registry directory holds, without reading any
/// content. A file asset is `<stem>.md`; a directory asset is a directory.
fn names_of(root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for category in read_dir(root)? {
        let category_dir = root.join(&category);
        if !is_real_dir(&category_dir) {
            continue;
        }
        for child in read_dir(&category_dir)? {
            let path = category_dir.join(&child);
            if is_real_dir(&path) {
                out.push((category.clone(), child));
            } else if is_real_file(&path) {
                let Some(stem) = child.strip_suffix(".md") else {
                    continue;
                };
                out.push((category.clone(), stem.to_string()));
            }
        }
    }
    Ok(out)
}

/// Every regular file under `root`, as paths relative to `root`, sorted.
///
/// Symlinks are skipped rather than followed: a link inside the registry is
/// not content the engine should copy, and following one could read outside
/// the registry entirely.
fn walk_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    collect(root, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

fn collect(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let dir = if rel.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    for name in read_dir(&dir)? {
        let child_rel = rel.join(&name);
        let path = dir.join(&name);
        if is_real_file(&path) {
            out.push(child_rel);
        } else if is_real_dir(&path) {
            collect(root, &child_rel, out)?;
        }
    }
    Ok(())
}

/// The names in `dir`, sorted. An unreadable directory is an error: a
/// registry that cannot be listed is a registry that cannot be trusted.
fn read_dir(dir: &Path) -> Result<Vec<String>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        out.push(entry.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    Ok(out)
}

fn is_real_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn registry() -> (TempDir, Registry) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::create_dir_all(root.join("skills")).unwrap();
        let registry = Registry::new(root, "SKILL.md");
        (dir, registry)
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn rels(asset: &DirAsset) -> Vec<PathBuf> {
        asset.files.iter().map(|(rel, _)| rel.clone()).collect()
    }

    #[test]
    fn a_file_asset_is_loaded() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/rules/security.md"), "## Summary\n");
        match registry.load("rules", "security").unwrap() {
            Asset::File(asset) => assert_eq!(asset.bytes, b"## Summary\n"),
            other => panic!("expected a file asset, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_asset_records_its_entry_and_artifacts() {
        let (dir, registry) = registry();
        let skill = dir.path().join("assets/skills/tdd");
        write(&skill.join("SKILL.md"), "## Description\n");
        write(&skill.join("checklist.md"), "- a\n");
        write(&skill.join("nested/notes.md"), "notes\n");

        match registry.load("skills", "tdd").unwrap() {
            Asset::Dir(asset) => {
                assert_eq!(asset.entry, PathBuf::from("SKILL.md"));
                assert!(asset.is_entry(Path::new("SKILL.md")));
                assert_eq!(
                    rels(&asset),
                    vec![
                        PathBuf::from("SKILL.md"),
                        PathBuf::from("checklist.md"),
                        PathBuf::from("nested/notes.md"),
                    ]
                );
                assert_eq!(
                    asset.artifacts().map(|(rel, _)| rel).collect::<Vec<_>>(),
                    vec![Path::new("checklist.md"), Path::new("nested/notes.md")]
                );
                assert_eq!(
                    asset.file(Path::new("checklist.md")),
                    Some(b"- a\n".as_slice())
                );
            }
            other => panic!("expected a directory asset, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_entry_file_is_an_error() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/skills/tdd/checklist.md"), "- a\n");
        let message = registry.load("skills", "tdd").unwrap_err();
        assert!(message.contains("no SKILL.md entry file"), "{message}");
    }

    #[test]
    fn a_missing_asset_names_both_paths_tried() {
        let (_dir, registry) = registry();
        let message = registry.load("rules", "nope").unwrap_err();
        assert!(message.contains("asset not found"), "{message}");
        assert!(message.contains("nope.md"), "{message}");
        assert!(message.contains("/nope"), "{message}");
    }

    #[test]
    fn both_shapes_at_once_is_ambiguous() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/rules/twin.md"), "x\n");
        write(&dir.path().join("assets/rules/twin/SKILL.md"), "x\n");
        let message = registry.load("rules", "twin").unwrap_err();
        assert!(message.contains("ambiguous"), "{message}");
    }

    #[test]
    fn invalid_names_are_rejected() {
        let (_dir, registry) = registry();
        assert!(registry.load("rules", "a/b").is_err());
        assert!(registry.load("rules", "").is_err());
    }

    #[test]
    fn symlinks_in_a_directory_asset_are_skipped() {
        let (dir, registry) = registry();
        let skill = dir.path().join("assets/skills/tdd");
        write(&skill.join("SKILL.md"), "## Description\n");
        write(&dir.path().join("outside.md"), "secret\n");
        std::os::unix::fs::symlink(dir.path().join("outside.md"), skill.join("link.md")).unwrap();

        match registry.load("skills", "tdd").unwrap() {
            Asset::Dir(asset) => assert_eq!(rels(&asset), vec![PathBuf::from("SKILL.md")]),
            other => panic!("expected a directory asset, got {other:?}"),
        }
    }

    #[test]
    fn a_catalog_index_is_not_an_asset() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/rules/README.md"), "# Rules\n");
        let message = registry.load("rules", "README").unwrap_err();
        assert!(message.contains("catalog index"), "{message}");
        assert_eq!(registry.list().unwrap(), Vec::<Entry>::new());
    }

    #[test]
    fn with_defaults_takes_the_entry_name_from_the_declaration() {
        let spec = Spec::parse().unwrap();
        let registry = Registry::with_defaults("/nowhere", &spec);
        assert_eq!(registry.skill_entry, spec.defaults.skill_entry);
    }

    // ----------------------------------------------------------- embedded

    #[test]
    fn the_embedded_registry_holds_the_shipped_assets() {
        let spec = Spec::parse().unwrap();
        let registry = Registry::embedded_with_defaults(&spec);
        assert_eq!(registry.root(), None);
        assert!(registry.load("rules", "conventional-commits").is_ok());
        assert!(registry.load("skills", "tdd").is_ok());
        assert!(registry.load("agents", "code-reviewer").is_ok());
    }

    #[test]
    fn the_embedded_registry_rejects_a_catalog_index() {
        let spec = Spec::parse().unwrap();
        let registry = Registry::embedded_with_defaults(&spec);
        assert!(registry.load("rules", "README").is_err());
    }

    #[test]
    fn the_embedded_registry_agrees_with_the_directory_one() {
        let spec = Spec::parse().unwrap();
        let embedded = Registry::embedded_with_defaults(&spec);
        let on_disk =
            Registry::with_defaults(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"), &spec);

        let from_embedded = embedded.list().unwrap();
        let from_disk = on_disk.list().unwrap();
        assert_eq!(from_embedded, from_disk);
        assert!(!from_embedded.is_empty());
    }

    #[test]
    fn a_directory_entry_carries_the_shipped_tdd_skill() {
        let spec = Spec::parse().unwrap();
        let registry = Registry::embedded_with_defaults(&spec);
        match registry.load("skills", "tdd").unwrap() {
            Asset::Dir(asset) => {
                assert_eq!(asset.entry, PathBuf::from(spec.defaults.skill_entry));
                assert_eq!(
                    rels(&asset),
                    vec![PathBuf::from("SKILL.md"), PathBuf::from("checklist.md")]
                );
                assert!(summary_of(asset.file(&asset.entry).unwrap_or(&[])).starts_with("Test"));
            }
            other => panic!("expected a directory asset, got {other:?}"),
        }
    }

    #[test]
    fn a_summary_is_the_first_line_of_the_first_section() {
        assert_eq!(
            summary_of(b"## Summary\n\nKeep secrets safe.\n\n## Guidelines\n\n- A.\n"),
            "Keep secrets safe."
        );
        assert_eq!(
            summary_of(b"## Description\n### A subheading\n\nTest first.\n"),
            "Test first."
        );
        assert_eq!(summary_of(b"# A title\n"), "");
    }
}
