//! Writing into the project tree: atomically, and never through a symlink.
//!
//! Everything that mutates a file goes through here. The two rules this
//! module enforces are the ones that make a sync safe to run unattended:
//!
//! * a write is atomic — a reader sees either the old content or the new,
//!   never half of either;
//! * no path between the project root and the leaf is a symlink, so a write
//!   can never land outside the project.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Write `bytes` at `absolute`, atomically, creating the parent directory.
pub fn install_file(absolute: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = parent_of(absolute);
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;

    let temporary = temporary_sibling(absolute);
    if let Err(e) = std::fs::write(&temporary, bytes) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("cannot write {}: {e}", absolute.display()));
    }
    if let Err(e) = std::fs::rename(&temporary, absolute) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("cannot write {}: {e}", absolute.display()));
    }
    Ok(())
}

/// Refuse to touch `relative` unless every directory above it is real.
///
/// The walk is `lstat`-only: a symlink anywhere between the project root and
/// the leaf would make the write land outside the project, so it is refused
/// rather than followed. `replaceable` names paths the run is about to clear
/// anyway — a directory asset root being replaced — which the walk lets
/// through.
pub fn guard_ancestors(
    project_root: &Path,
    relative: &Path,
    replaceable: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    let mut accumulated = PathBuf::new();
    let mut ancestors: Vec<PathBuf> = Vec::new();
    for component in relative.components() {
        accumulated.push(component);
        ancestors.push(accumulated.clone());
    }
    // The leaf is not an ancestor; its own kind is a separate question.
    ancestors.pop();

    for ancestor in ancestors {
        if replaceable.contains(&ancestor) {
            continue;
        }
        let absolute = project_root.join(&ancestor);
        let metadata = match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("cannot inspect {}: {e}", absolute.display())),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "refusing to write through the symlinked directory {}",
                absolute.display()
            ));
        }
        if !metadata.is_dir() {
            return Err(format!(
                "refusing to write through {} which is not a directory",
                absolute.display()
            ));
        }
    }
    Ok(())
}

/// A sibling temp name that cannot collide with another writer in this
/// process or another one.
fn temporary_sibling(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    path.with_file_name(format!("{name}.tmp.{}.{unique}", std::process::id()))
}

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn install_file_writes_bytes_and_leaves_no_temporary_behind() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("a/b.md");
        install_file(&target, b"new\n").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new\n");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("a"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("b.md")]);
    }

    #[test]
    fn install_file_replaces_content_in_place() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("b.md");
        install_file(&target, b"first\n").unwrap();
        install_file(&target, b"second\n").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second\n");
    }

    /// A write that cannot complete must leave the old thing exactly as it
    /// was. Here the rename fails because the destination is a non-empty
    /// directory, which is the closest portable stand-in for a crash between
    /// the temp write and the rename.
    #[test]
    fn install_file_leaves_the_old_content_when_the_write_fails() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("victim");
        std::fs::create_dir_all(target.join("keep")).unwrap();
        std::fs::write(target.join("keep/inside.md"), "old\n").unwrap();

        let error = install_file(&target, b"new\n").unwrap_err();
        assert!(error.contains("cannot write"), "{error}");

        assert_eq!(
            std::fs::read_to_string(target.join("keep/inside.md")).unwrap(),
            "old\n"
        );
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("victim")]);
    }

    #[test]
    fn install_file_unlinks_a_symlink_rather_than_following_it() {
        let dir = TempDir::new().unwrap();
        let referent = dir.path().join("referent.md");
        std::fs::write(&referent, "secret\n").unwrap();
        let target = dir.path().join("link.md");
        std::os::unix::fs::symlink(&referent, &target).unwrap();

        install_file(&target, b"mine\n").unwrap();
        assert!(!target.is_symlink());
        assert_eq!(std::fs::read(&target).unwrap(), b"mine\n");
        assert_eq!(std::fs::read(&referent).unwrap(), b"secret\n");
    }

    #[test]
    fn the_walk_allows_a_directory_that_is_not_there_yet() {
        let dir = TempDir::new().unwrap();
        guard_ancestors(dir.path(), Path::new("a/b/c.md"), &BTreeSet::new()).unwrap();
    }

    #[test]
    fn the_walk_refuses_a_symlinked_directory() {
        let dir = TempDir::new().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("a")).unwrap();
        let error =
            guard_ancestors(dir.path(), Path::new("a/b/c.md"), &BTreeSet::new()).unwrap_err();
        assert!(error.contains("refusing to write through"), "{error}");
    }

    #[test]
    fn the_walk_refuses_a_file_where_a_directory_belongs() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a"), "not a directory\n").unwrap();
        let error = guard_ancestors(dir.path(), Path::new("a/b.md"), &BTreeSet::new()).unwrap_err();
        assert!(error.contains("which is not a directory"), "{error}");
    }

    #[test]
    fn the_walk_lets_a_replaceable_path_through() {
        let dir = TempDir::new().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("a")).unwrap();
        let replaceable: BTreeSet<PathBuf> = [PathBuf::from("a")].into_iter().collect();
        guard_ancestors(dir.path(), Path::new("a/b/c.md"), &replaceable).unwrap();
    }
}
