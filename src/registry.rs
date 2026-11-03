//! The asset registry.
//!
//! Sources live under `<assets>/<category>/` and come in two shapes: a single
//! Markdown file (`<name>.md`) or a directory (`<name>/`) whose entry file
//! carries the sections and whose other files are free-form artifacts. The
//! engine infers which shape it is from the layout, so adding an asset needs
//! no registration.

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
    /// Absolute path of the source file in the registry.
    pub path: PathBuf,
}

/// A directory asset: an entry file plus free-form artifacts, emitted as a
/// directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirAsset {
    /// Absolute path of the source directory in the registry.
    pub path: PathBuf,
    /// Entry file, relative to `path`. This is the one that receives
    /// injected frontmatter.
    pub entry: PathBuf,
    /// Every file in the asset, relative to `path`, sorted, including `entry`.
    pub files: Vec<PathBuf>,
}

impl DirAsset {
    /// True when `rel` is the entry file.
    pub fn is_entry(&self, rel: &Path) -> bool {
        rel == self.entry
    }

    /// Every file except the entry, in the same order as `files`.
    pub fn artifacts(&self) -> impl Iterator<Item = &Path> {
        self.files
            .iter()
            .filter(|p| **p != self.entry)
            .map(|p| p.as_path())
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
}

/// A registry rooted at a directory laid out as `assets/<category>/<name>`.
#[derive(Debug, Clone)]
pub struct Registry {
    root: PathBuf,
    skill_entry: String,
}

impl Registry {
    pub fn new(root: impl Into<PathBuf>, skill_entry: impl Into<String>) -> Self {
        Registry {
            root: root.into(),
            skill_entry: skill_entry.into(),
        }
    }

    /// Build from the declaration, so the entry-file convention comes from
    /// `spec.json` rather than being restated here.
    pub fn with_defaults(root: impl Into<PathBuf>, spec: &Spec) -> Self {
        Registry::new(root, spec.defaults.skill_entry.clone())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load one asset. Exactly one of `<name>.md` and `<name>/` may exist.
    pub fn load(&self, category: &str, name: &str) -> Result<Asset, String> {
        let category = Name::try_from(category.to_string())?;
        let name = Name::try_from(name.to_string())?;
        let category_dir = self.root.join(category.as_str());
        let file_path = category_dir.join(format!("{}.md", name.as_str()));
        let dir_path = category_dir.join(name.as_str());

        let has_file = file_path.is_file();
        let has_dir = dir_path.is_dir();

        match (has_file, has_dir) {
            (true, false) => Ok(Asset::File(FileAsset { path: file_path })),
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
                let files = walk_files(&dir_path)?;
                Ok(Asset::Dir(DirAsset {
                    path: dir_path,
                    entry,
                    files,
                }))
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
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let child_rel = rel.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|e| format!("cannot stat {}: {e}", entry.path().display()))?;
        let file_type = metadata.file_type();
        if file_type.is_file() {
            out.push(child_rel);
        } else if file_type.is_dir() {
            collect(root, &child_rel, out)?;
        }
    }
    Ok(())
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

    #[test]
    fn a_file_asset_is_loaded() {
        let (dir, registry) = registry();
        write(&dir.path().join("assets/rules/security.md"), "## Summary\n");
        match registry.load("rules", "security").unwrap() {
            Asset::File(asset) => {
                assert!(asset.path.ends_with("rules/security.md"));
            }
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
                    asset.files,
                    vec![
                        PathBuf::from("SKILL.md"),
                        PathBuf::from("checklist.md"),
                        PathBuf::from("nested/notes.md"),
                    ]
                );
                assert_eq!(
                    asset.artifacts().collect::<Vec<_>>(),
                    vec![Path::new("checklist.md"), Path::new("nested/notes.md")]
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
            Asset::Dir(asset) => assert_eq!(asset.files, vec![PathBuf::from("SKILL.md")]),
            other => panic!("expected a directory asset, got {other:?}"),
        }
    }

    #[test]
    fn with_defaults_takes_the_entry_name_from_the_declaration() {
        let spec = Spec::parse().unwrap();
        let registry = Registry::with_defaults("/nowhere", &spec);
        assert_eq!(registry.skill_entry, spec.defaults.skill_entry);
    }
}
