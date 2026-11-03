//! The ownership manifest.
//!
//! One row per *imported asset*, not per file: orphan removal deletes whole
//! assets — a file, or a directory asset's entire directory — and the row is
//! what names it. Each row also records the hash of every rendered file
//! inside it, which is the engine's memory of what it last wrote.
//!
//! ```json
//! [
//!   {
//!     "path": ".opencode/skills/tdd",
//!     "files": [
//!       { "path": ".opencode/skills/tdd/SKILL.md", "hash": "sha256:…" },
//!       { "path": ".opencode/skills/tdd/checklist.md", "hash": "sha256:…" }
//!     ]
//!   }
//! ]
//! ```
//!
//! No category, no name, no kind: those are derivable from the path and the
//! config, and no question the engine asks needs them. Nothing here is a hash
//! of the *source* — hashing the rendered output catches an injection change
//! too.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::files::install_file;

/// The manifest: every asset this project's sync owns.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Manifest(pub Vec<Asset>);

/// One imported asset and the files rendered inside it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// The emitted asset path: a file, or a directory for a directory asset.
    pub path: PathBuf,
    /// Every emitted file inside the asset. A file asset has exactly one.
    pub files: Vec<FileEntry>,
}

/// One rendered file and the hash of the bytes it had when last written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: PathBuf,
    /// `sha256:<lowercase hex>` of the rendered bytes.
    pub hash: String,
}

impl Manifest {
    /// Rows and files ordered by path, so the manifest's bytes do not depend
    /// on the order imports happened to be written in.
    pub fn sorted(mut self) -> Self {
        self.0.sort_by(|a, b| a.path.cmp(&b.path));
        for asset in &mut self.0 {
            asset.files.sort_by(|a, b| a.path.cmp(&b.path));
        }
        self
    }

    /// Every `(file path, hash)` pair, in manifest order.
    pub fn files(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.0.iter().flat_map(|asset| {
            asset
                .files
                .iter()
                .map(|entry| (entry.path.as_path(), entry.hash.as_str()))
        })
    }

    /// The hash last written for `relpath`, if the manifest records it.
    pub fn hash_of(&self, relpath: &Path) -> Option<&str> {
        self.files()
            .find(|(path, _)| *path == relpath)
            .map(|(_, hash)| hash)
    }

    /// True when `relpath` is an owned asset path or lives inside one.
    ///
    /// This is what makes a path ours to overwrite or delete. Everything else
    /// under a category root belongs to the project and is never touched.
    pub fn owns(&self, relpath: &Path) -> bool {
        self.0
            .iter()
            .any(|asset| relpath == asset.path || relpath.starts_with(&asset.path))
    }
}

/// Read the manifest.
///
/// A missing file is `Ok(None)`: there is nothing to reconcile against yet.
/// A file that exists but cannot be read or parsed is `Err`, and the caller
/// decides — `sync` warns and skips orphan removal rather than guessing.
pub fn load(path: &Path) -> Result<Option<Manifest>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("unreadable manifest {}: {e}", path.display())),
    };
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|e| format!("unreadable manifest {}: {e}", path.display()))?;
    Ok(Some(manifest))
}

/// Write the manifest atomically, rows sorted for stable bytes.
pub fn store(path: &Path, manifest: &Manifest) -> Result<(), String> {
    let sorted = manifest.clone().sorted();
    let mut json = serde_json::to_string_pretty(&sorted.0)
        .map_err(|e| format!("cannot serialise the manifest: {e}"))?;
    json.push('\n');
    install_file(path, json.as_bytes())
}

/// The hash of `bytes` as the manifest records it.
pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(7 + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn entry(path: &str, hash: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            hash: hash.to_string(),
        }
    }

    fn asset(path: &str, files: Vec<FileEntry>) -> Asset {
        Asset {
            path: PathBuf::from(path),
            files,
        }
    }

    fn sample() -> Manifest {
        Manifest(vec![
            asset(
                ".opencode/skills/tdd",
                vec![
                    entry(".opencode/skills/tdd/checklist.md", "sha256:bb"),
                    entry(".opencode/skills/tdd/SKILL.md", "sha256:aa"),
                ],
            ),
            asset(
                ".opencode/rules/commits.md",
                vec![entry(".opencode/rules/commits.md", "sha256:cc")],
            ),
        ])
    }

    // ------------------------------------------------------------- hashing

    #[test]
    fn the_hash_matches_the_standard_vectors() {
        assert_eq!(
            hash_bytes(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_bytes(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_hash_is_deterministic_and_content_addressed() {
        assert_eq!(hash_bytes(b"same"), hash_bytes(b"same"));
        assert_ne!(hash_bytes(b"same"), hash_bytes(b"diff"));
    }

    #[test]
    fn the_hash_is_prefixed_so_the_algorithm_is_visible() {
        assert!(hash_bytes(b"x").starts_with("sha256:"));
    }

    // ------------------------------------------------------------ ownership

    #[test]
    fn owns_accepts_a_listed_path() {
        let manifest = sample();
        assert!(manifest.owns(Path::new(".opencode/rules/commits.md")));
        assert!(manifest.owns(Path::new(".opencode/skills/tdd")));
    }

    #[test]
    fn owns_accepts_a_path_inside_a_listed_directory() {
        let manifest = sample();
        assert!(manifest.owns(Path::new(".opencode/skills/tdd/SKILL.md")));
        assert!(manifest.owns(Path::new(".opencode/skills/tdd/nested/deep.md")));
    }

    #[test]
    fn owns_rejects_a_sibling_and_an_unrelated_path() {
        let manifest = sample();
        assert!(!manifest.owns(Path::new(".opencode/skills/tddd")));
        assert!(!manifest.owns(Path::new(".opencode/skills/tdd-old")));
        assert!(!manifest.owns(Path::new(".opencode/rules/custom.md")));
        assert!(!manifest.owns(Path::new("opencode.json")));
    }

    #[test]
    fn owns_rejects_the_prefix_without_a_separator() {
        let manifest = Manifest(vec![asset("a/b", vec![entry("a/b/c.md", "sha256:aa")])]);
        assert!(manifest.owns(Path::new("a/b/c.md")));
        assert!(!manifest.owns(Path::new("a/bc")));
    }

    // -------------------------------------------------------------- lookups

    #[test]
    fn files_yields_every_pair_flat() {
        let manifest = sample();
        let pairs: Vec<_> = manifest.files().collect();
        assert_eq!(pairs.len(), 3);
        assert!(pairs.iter().any(|(path, hash)| *path
            == Path::new(".opencode/skills/tdd/SKILL.md")
            && *hash == "sha256:aa"));
    }

    #[test]
    fn hash_of_finds_a_recorded_file_and_misses_an_unrecorded_one() {
        let manifest = sample();
        assert_eq!(
            manifest.hash_of(Path::new(".opencode/skills/tdd/SKILL.md")),
            Some("sha256:aa")
        );
        assert_eq!(
            manifest.hash_of(Path::new(".opencode/skills/tdd/other.md")),
            None
        );
    }

    // ---------------------------------------------------------- persistence

    #[test]
    fn storing_and_loading_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agent-assets.lock");
        let manifest = sample().sorted();
        store(&path, &manifest).unwrap();
        assert_eq!(load(&path).unwrap(), Some(manifest));
    }

    #[test]
    fn storing_twice_produces_identical_bytes() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agent-assets.lock");
        store(&path, &sample()).unwrap();
        let first = std::fs::read(&path).unwrap();
        store(&path, &sample()).unwrap();
        let second = std::fs::read(&path).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn storing_sorts_rows_and_files_by_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agent-assets.lock");
        store(&path, &sample()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let reloaded: Manifest = serde_json::from_str(&text).unwrap();
        let paths: Vec<_> = reloaded.0.iter().map(|a| a.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from(".opencode/rules/commits.md"),
                PathBuf::from(".opencode/skills/tdd"),
            ]
        );
        let files: Vec<_> = reloaded.0[1].files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(
            files,
            vec![
                PathBuf::from(".opencode/skills/tdd/SKILL.md"),
                PathBuf::from(".opencode/skills/tdd/checklist.md"),
            ]
        );
    }

    #[test]
    fn storing_ends_with_a_newline() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agent-assets.lock");
        store(&path, &Manifest::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn storing_creates_the_parent_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("deep/agent-assets.lock");
        store(&path, &Manifest::default()).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn a_missing_manifest_loads_as_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(load(&dir.path().join("absent.lock")).unwrap(), None);
    }

    #[test]
    fn an_empty_array_is_an_empty_manifest() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        std::fs::write(&path, "[]\n").unwrap();
        assert_eq!(load(&path).unwrap(), Some(Manifest::default()));
    }

    #[test]
    fn malformed_json_is_reported_as_unreadable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        std::fs::write(&path, "{ not json").unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.contains("unreadable manifest"), "{error}");
    }

    #[test]
    fn a_manifest_of_the_wrong_shape_is_reported_as_unreadable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        std::fs::write(&path, "{\"a\": 1}").unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.contains("unreadable manifest"), "{error}");
    }

    #[test]
    fn a_row_with_a_missing_hash_is_reported_as_unreadable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        std::fs::write(&path, r#"[{"path": "a", "files": [{"path": "a"}]}]"#).unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.contains("unreadable manifest"), "{error}");
    }

    #[test]
    fn the_written_shape_matches_the_documented_one() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        let manifest = Manifest(vec![asset(
            ".opencode/rules/commits.md",
            vec![entry(".opencode/rules/commits.md", "sha256:aa")],
        )]);
        store(&path, &manifest).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            value,
            serde_json::json!([
                {
                    "path": ".opencode/rules/commits.md",
                    "files": [
                        { "path": ".opencode/rules/commits.md", "hash": "sha256:aa" }
                    ]
                }
            ])
        );
    }

    /// The manifest's bytes are output too, so they are pinned rather than
    /// left to whatever the serialiser happens to do.
    #[test]
    fn the_written_bytes_are_pinned() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("m.lock");
        let manifest = Manifest(vec![asset(
            ".opencode/rules/commits.md",
            vec![entry(".opencode/rules/commits.md", "sha256:aa")],
        )]);
        store(&path, &manifest).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[\n  {\n    \"path\": \".opencode/rules/commits.md\",\n    \"files\": [\n      {\n        \"path\": \".opencode/rules/commits.md\",\n        \"hash\": \"sha256:aa\"\n      }\n    ]\n  }\n]\n"
        );
    }
}
