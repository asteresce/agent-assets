//! The asset registry.
//!
//! Entries come from either the directory laid out as `<root>/<category>/`
//! or the table compiled into the binary, and come in two shapes: a single
//! Markdown file (`<name>.<extension>`) or a directory (`<name>/`) whose
//! entry file carries the sections and whose other files are free-form
//! artifacts. Which shape a category has — and what its entry file is called
//! — is declared in the registry's own `spec.json`, so adding a category is
//! one more declaration there rather than a rule in this crate.
//!
//! Loading reads the bytes eagerly, so an asset is a pure value afterwards:
//! rendering, `show` and `list` all work off it without touching a filesystem.

use std::path::{Path, PathBuf};

use crate::config::Name;
use crate::files;
use crate::sections;
use crate::spec::{AssetSpec, AssetSpecs};

pub use crate::spec::AssetKind;

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

/// The source suffix a `file` category declares. The declaration enforces
/// this at parse time; the error keeps `load` total regardless.
fn extension_of(spec: &AssetSpec) -> Result<&str, String> {
    spec.extension
        .as_deref()
        .filter(|extension| !extension.is_empty())
        .ok_or_else(|| "the category declares file assets but no `extension`".to_string())
}

/// The entry file a `dir` category declares, for the same reason.
fn entry_of(spec: &AssetSpec) -> Result<&str, String> {
    spec.entry_file
        .as_deref()
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "the category declares directory assets but no `entryFile`".to_string())
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
            summary: sections::summary(asset.entry_bytes()),
        }
    }
}

/// One thing a registry's source holds, before the declaration decides
/// whether it is an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub category: String,
    /// The file name as it sits on disk (a file) or the directory name (a
    /// directory).
    pub child: String,
    /// The shape the layout gives it, which may not be the declared one.
    pub shape: AssetKind,
}

/// A registry of shared assets.
#[derive(Debug, Clone)]
pub struct Registry {
    source: Source,
    specs: AssetSpecs,
}

impl Registry {
    /// Open a registry directory. The directory must carry its own
    /// `spec.json`: that is where the categories, their shapes and their
    /// section schemas come from.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        let path = root.join("spec.json");
        let text = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("asset directory {} has no spec.json", root.display())
            } else {
                format!("cannot read {}: {e}", path.display())
            }
        })?;
        let specs = AssetSpecs::parse(&text, &path.display().to_string())?;
        Ok(Registry {
            source: Source::Directory { root },
            specs,
        })
    }

    /// The registry compiled into this binary, with the declaration embedded
    /// from `assets/`.
    pub fn embedded() -> Result<Self, String> {
        Ok(Registry {
            source: Source::Embedded,
            specs: AssetSpecs::embedded()?,
        })
    }

    /// The declaration this registry runs on.
    pub fn specs(&self) -> &AssetSpecs {
        &self.specs
    }

    /// The directory this registry reads, when it reads one.
    pub fn root(&self) -> Option<&Path> {
        match &self.source {
            Source::Directory { root } => Some(root.as_path()),
            Source::Embedded => None,
        }
    }

    /// Load one asset. Its category must be declared and its name must match
    /// the declared shape: `<name>.<extension>` for a file category,
    /// `<name>/` with the declared entry file for a directory one.
    pub fn load(&self, category: &str, name: &str) -> Result<Asset, String> {
        let category = Name::try_from(category.to_string())?;
        let name = Name::try_from(name.to_string())?;
        if is_catalog_index(name.as_str()) {
            return Err(format!(
                "asset not found: {category}/{name} (a catalog index is not an asset)"
            ));
        }
        let Some(spec) = self.specs.get(category.as_str()) else {
            return Err(format!(
                "category {category} is not declared in the registry's spec.json"
            ));
        };
        match &self.source {
            Source::Directory { root } => self.load_directory(root, &category, &name, spec),
            Source::Embedded => self.load_embedded(&category, &name, spec),
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

    /// Every category child the source holds — including shapes and
    /// categories the declaration does not admit, which `list` and `load`
    /// skip and `validate` reports.
    pub fn inventory(&self) -> Result<Vec<Found>, String> {
        match &self.source {
            Source::Directory { root } => {
                let mut out = Vec::new();
                for category in files::read_dir(root)? {
                    let category_dir = root.join(&category);
                    if !files::is_real_dir(&category_dir)? {
                        continue;
                    }
                    for child in files::read_dir(&category_dir)? {
                        let path = category_dir.join(&child);
                        if files::is_real_dir(&path)? {
                            if is_catalog_index(&child) {
                                continue;
                            }
                            out.push(Found {
                                category: category.clone(),
                                child,
                                shape: AssetKind::Dir,
                            });
                        } else if files::is_real_file(&path)? {
                            if child == "README.md" {
                                continue;
                            }
                            out.push(Found {
                                category: category.clone(),
                                child,
                                shape: AssetKind::File,
                            });
                        }
                    }
                }
                out.sort_by(|a, b| (&a.category, &a.child).cmp(&(&b.category, &b.child)));
                Ok(out)
            }
            Source::Embedded => {
                let mut out = Vec::new();
                for asset in embedded::EMBEDDED {
                    if is_catalog_index(asset.name) {
                        continue;
                    }
                    match asset.kind {
                        AssetKind::File => {
                            let Some(file) = asset.files.first() else {
                                continue;
                            };
                            out.push(Found {
                                category: asset.category.to_string(),
                                child: file.rel.to_string(),
                                shape: AssetKind::File,
                            });
                        }
                        AssetKind::Dir => out.push(Found {
                            category: asset.category.to_string(),
                            child: asset.name.to_string(),
                            shape: AssetKind::Dir,
                        }),
                    }
                }
                Ok(out)
            }
        }
    }

    /// Every `(category, name)` the source holds that the declaration
    /// admits, sorted. A name `load` would refuse is not listed, so `list`
    /// and `load` always agree.
    fn names(&self) -> Result<Vec<(String, String)>, String> {
        let mut out: Vec<(String, String)> = match &self.source {
            Source::Directory { root } => names_of(root, &self.specs)?,
            Source::Embedded => embedded::EMBEDDED
                .iter()
                .filter(|asset| self.specs.get(asset.category).is_some())
                .map(|asset| (asset.category.to_string(), asset.name.to_string()))
                .collect(),
        };
        out.retain(|(_, name)| !is_catalog_index(name));
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn load_directory(
        &self,
        root: &Path,
        category: &Name,
        name: &Name,
        spec: &AssetSpec,
    ) -> Result<Asset, String> {
        let category_dir = root.join(category.as_str());
        match spec.kind {
            AssetKind::File => {
                let extension = extension_of(spec)?;
                let file_path = category_dir.join(format!("{}.{}", name.as_str(), extension));
                if !file_path.is_file() {
                    return Err(format!(
                        "asset not found: {category}/{name} (expected {})",
                        file_path.display()
                    ));
                }
                Ok(Asset::File(FileAsset {
                    bytes: read_bytes(&file_path)?,
                }))
            }
            AssetKind::Dir => {
                let entry = PathBuf::from(entry_of(spec)?);
                let dir_path = category_dir.join(name.as_str());
                let entry_path = dir_path.join(&entry);
                if !dir_path.is_dir() {
                    return Err(format!(
                        "asset not found: {category}/{name} (expected {})",
                        dir_path.display()
                    ));
                }
                if !entry_path.is_file() {
                    return Err(format!(
                        "asset {category}/{name} has no {} entry file (expected {})",
                        entry.display(),
                        entry_path.display()
                    ));
                }
                let mut paths = Vec::new();
                files::walk_files(&dir_path, Path::new(""), &mut paths)?;
                paths.sort();
                let mut files = Vec::new();
                for relative in paths {
                    let absolute = dir_path.join(&relative);
                    files.push((relative, read_bytes(&absolute)?));
                }
                Ok(Asset::Dir(DirAsset { entry, files }))
            }
        }
    }

    fn load_embedded(
        &self,
        category: &Name,
        name: &Name,
        spec: &AssetSpec,
    ) -> Result<Asset, String> {
        let Some(asset) = embedded::EMBEDDED
            .iter()
            .find(|asset| asset.category == category.as_str() && asset.name == name.as_str())
        else {
            return Err(format!(
                "asset not found: {category}/{name} (not in the registry compiled into this binary)"
            ));
        };
        if asset.kind != spec.kind {
            return Err(format!(
                "embedded {category}/{name} is a {:?} asset but spec.json declares {:?} \
                 (the binary's registry and its declaration disagree)",
                asset.kind, spec.kind
            ));
        }
        match spec.kind {
            AssetKind::File => Ok(Asset::File(FileAsset {
                bytes: asset.files[0].bytes.to_vec(),
            })),
            AssetKind::Dir => {
                let entry = PathBuf::from(entry_of(spec)?);
                let files: Vec<(PathBuf, Vec<u8>)> = asset
                    .files
                    .iter()
                    .map(|file| (PathBuf::from(file.rel), file.bytes.to_vec()))
                    .collect();
                if !files.iter().any(|(rel, _)| *rel == entry) {
                    return Err(format!(
                        "asset {category}/{name} has no {} entry file",
                        entry.display()
                    ));
                }
                Ok(Asset::Dir(DirAsset { entry, files }))
            }
        }
    }
}

/// Every `(category, name)` a registry directory holds that the declaration
/// admits, without reading any content. A file asset is `<stem>.<extension>`;
/// a directory asset is a directory. Undeclared categories and the wrong
/// shape are not assets — `validate` reports them through [`Registry::inventory`].
fn names_of(root: &Path, specs: &AssetSpecs) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for category in files::read_dir(root)? {
        let category_dir = root.join(&category);
        if !files::is_real_dir(&category_dir)? {
            continue;
        }
        let Some(spec) = specs.get(&category) else {
            continue;
        };
        for child in files::read_dir(&category_dir)? {
            let path = category_dir.join(&child);
            match spec.kind {
                AssetKind::Dir => {
                    if files::is_real_dir(&path)? {
                        out.push((category.clone(), child));
                    }
                }
                AssetKind::File => {
                    if files::is_real_file(&path)? {
                        let suffix = format!(".{}", extension_of(spec)?);
                        if let Some(stem) = child.strip_suffix(&suffix) {
                            out.push((category.clone(), stem.to_string()));
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Two categories: one file, one directory, with just enough schema to
    /// load what the tests write.
    const SPEC: &str = r#"{
        "rules": {
            "kind": "file",
            "extension": "md",
            "headings": [{ "name": "Summary", "required": true }]
        },
        "skills": {
            "kind": "dir",
            "entryFile": "SKILL.md",
            "headings": [{ "name": "Description", "required": true }]
        }
    }"#;

    fn registry() -> (TempDir, Registry) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::create_dir_all(root.join("skills")).unwrap();
        fs::write(root.join("spec.json"), SPEC).unwrap();
        let registry = Registry::new(root).unwrap();
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
    fn a_missing_asset_names_the_path_it_tried() {
        let (_dir, registry) = registry();
        let message = registry.load("rules", "nope").unwrap_err();
        assert!(message.contains("asset not found"), "{message}");
        assert!(message.contains("nope.md"), "{message}");
    }

    #[test]
    fn the_declared_shape_decides_and_the_other_is_ignored() {
        let (dir, registry) = registry();
        // A file and a directory with the same name in a file category: the
        // file is the asset, the directory is not one.
        write(&dir.path().join("assets/rules/twin.md"), "x\n");
        write(&dir.path().join("assets/rules/twin/SKILL.md"), "x\n");
        // A file in a directory category is not an asset either.
        write(&dir.path().join("assets/skills/loose.md"), "x\n");

        match registry.load("rules", "twin").unwrap() {
            Asset::File(_) => {}
            other => panic!("expected a file asset, got {other:?}"),
        }
        let message = registry.load("skills", "loose").unwrap_err();
        assert!(message.contains("asset not found"), "{message}");

        let names: Vec<String> = registry
            .list()
            .unwrap()
            .iter()
            .map(|entry| format!("{}/{}", entry.category, entry.name))
            .collect();
        assert_eq!(names, vec!["rules/twin"]);
    }

    #[test]
    fn an_undeclared_category_is_not_an_asset() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/agents/build.md"), "## Role\n");

        let message = registry.load("agents", "build").unwrap_err();
        assert!(message.contains("not declared"), "{message}");
        assert_eq!(registry.list().unwrap(), Vec::<Entry>::new());

        let found = registry.inventory().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].category, "agents");
    }

    #[test]
    fn a_registry_without_a_spec_json_is_refused() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        fs::create_dir_all(&root).unwrap();
        let message = Registry::new(&root).unwrap_err();
        assert!(message.contains("spec.json"), "{message}");
    }

    #[test]
    fn an_invalid_spec_json_is_refused_with_its_path() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("assets");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("spec.json"), "{ not json").unwrap();
        let message = Registry::new(&root).unwrap_err();
        assert!(message.contains("spec.json"), "{message}");
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
    fn the_entry_file_comes_from_the_category_declaration() {
        let (dir, registry) = registry();
        write(
            &dir.path().join("assets/skills/tdd/SKILL.md"),
            "## Description\n",
        );

        match registry.load("skills", "tdd").unwrap() {
            Asset::Dir(asset) => assert_eq!(asset.entry, PathBuf::from("SKILL.md")),
            other => panic!("expected a directory asset, got {other:?}"),
        }
        assert_eq!(
            registry
                .specs()
                .get("skills")
                .unwrap()
                .entry_file
                .as_deref(),
            Some("SKILL.md")
        );
    }

    // ----------------------------------------------------------- embedded

    #[test]
    fn the_embedded_registry_holds_the_shipped_assets() {
        let registry = Registry::embedded().unwrap();
        assert_eq!(registry.root(), None);
        assert!(registry.load("rules", "conventional-commits").is_ok());
        assert!(registry.load("skills", "tdd").is_ok());
        assert!(registry.load("agents", "code-reviewer").is_ok());
    }

    #[test]
    fn the_embedded_registry_rejects_a_catalog_index() {
        let registry = Registry::embedded().unwrap();
        assert!(registry.load("rules", "README").is_err());
    }

    #[test]
    fn the_embedded_registry_agrees_with_the_directory_one() {
        let embedded = Registry::embedded().unwrap();
        let on_disk = Registry::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).unwrap();

        let from_embedded = embedded.list().unwrap();
        let from_disk = on_disk.list().unwrap();
        assert_eq!(from_embedded, from_disk);
        assert!(!from_embedded.is_empty());
    }

    #[test]
    fn a_directory_entry_carries_the_shipped_tdd_skill() {
        let registry = Registry::embedded().unwrap();
        match registry.load("skills", "tdd").unwrap() {
            Asset::Dir(asset) => {
                let entry = registry
                    .specs()
                    .get("skills")
                    .unwrap()
                    .entry_file
                    .clone()
                    .unwrap();
                assert_eq!(asset.entry, PathBuf::from(entry));
                assert_eq!(
                    rels(&asset),
                    vec![PathBuf::from("SKILL.md"), PathBuf::from("checklist.md")]
                );
                assert!(
                    sections::summary(asset.file(&asset.entry).unwrap_or(&[])).starts_with("Test")
                );
            }
            other => panic!("expected a directory asset, got {other:?}"),
        }
    }
}
