//! Render an asset to the bytes that belong on disk.
//!
//! Sources are provider-neutral: they carry no frontmatter and no framing
//! that belongs to a particular agent. Whatever frontmatter the project
//! configures is *replaced* into the entry file, so the rendered output is a
//! pure function of the source and the injection map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::registry::{Asset, DirAsset, FileAsset};
use crate::yaml_emit;

/// Drop a leading `---` … `---` frontmatter block.
///
/// A block with no closing delimiter is not frontmatter and is passed through
/// unchanged. Blank lines between the block and the body are part of the
/// separator; everything else is returned byte for byte.
pub fn strip_frontmatter(text: &str) -> &str {
    const OPEN: &str = "---\n";
    let Some(after_open) = text.strip_prefix(OPEN) else {
        return text;
    };

    let mut consumed = 0usize;
    let mut body = None;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        if trimmed == "---" {
            body = Some(&after_open[consumed + line.len()..]);
            break;
        }
        consumed += line.len();
    }

    let Some(body) = body else {
        return text;
    };
    body.trim_start_matches('\n')
}

/// The source body with any frontmatter replaced by `frontmatter`.
///
/// An empty map emits no block at all: it still removes whatever the source
/// carried, because frontmatter is the consumer's to decide.
pub fn render_markdown(source: &str, frontmatter: &BTreeMap<String, Value>) -> String {
    let body = strip_frontmatter(source);
    if frontmatter.is_empty() {
        return body.to_string();
    }
    format!(
        "---\n{}---\n\n{}",
        yaml_emit::mapping_to_yaml(frontmatter),
        body
    )
}

/// The rendered contents of one asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rendered {
    /// A file asset: the rendered bytes of the single file.
    File(Vec<u8>),
    /// A directory asset: `(path relative to the output directory, bytes)`
    /// for every file it holds, in the registry's sorted order.
    Dir(Vec<(PathBuf, Vec<u8>)>),
}

/// Render one asset: the entry file gets frontmatter, artifacts are copied
/// verbatim.
pub fn render_asset(
    asset: &Asset,
    frontmatter: &BTreeMap<String, Value>,
) -> Result<Rendered, String> {
    match asset {
        Asset::File(file) => Ok(Rendered::File(render_file(file, frontmatter)?)),
        Asset::Dir(dir) => Ok(Rendered::Dir(render_dir(dir, frontmatter)?)),
    }
}

fn render_file(
    asset: &FileAsset,
    frontmatter: &BTreeMap<String, Value>,
) -> Result<Vec<u8>, String> {
    let source = read_text(&asset.path)?;
    Ok(render_markdown(&source, frontmatter).into_bytes())
}

fn render_dir(
    asset: &DirAsset,
    frontmatter: &BTreeMap<String, Value>,
) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut out = Vec::with_capacity(asset.files.len());
    for relative in &asset.files {
        let absolute = asset.path.join(relative);
        let bytes = if asset.is_entry(relative) {
            let source = read_text(&absolute)?;
            render_markdown(&source, frontmatter).into_bytes()
        } else {
            std::fs::read(&absolute)
                .map_err(|e| format!("cannot read {}: {e}", absolute.display()))?
        };
        out.push((relative.clone(), bytes));
    }
    Ok(out)
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(value: Value) -> BTreeMap<String, Value> {
        value.as_object().unwrap().clone().into_iter().collect()
    }

    // ---------------------------------------------------- strip_frontmatter

    #[test]
    fn a_file_without_frontmatter_is_untouched() {
        assert_eq!(
            strip_frontmatter("## Summary\n\nBody.\n"),
            "## Summary\n\nBody.\n"
        );
    }

    #[test]
    fn a_leading_block_is_dropped() {
        assert_eq!(strip_frontmatter("---\na: 1\n---\n\nbody\n"), "body\n");
    }

    #[test]
    fn blank_lines_after_the_block_are_separator() {
        assert_eq!(strip_frontmatter("---\na: 1\n---\n\n\nbody\n"), "body\n");
    }

    #[test]
    fn a_body_that_leads_with_a_blank_line_keeps_it_when_theres_no_block() {
        assert_eq!(strip_frontmatter("\nbody\n"), "\nbody\n");
    }

    #[test]
    fn an_unterminated_block_is_not_frontmatter() {
        let text = "---\na: 1\nbody\n";
        assert_eq!(strip_frontmatter(text), text);
    }

    #[test]
    fn an_empty_block_strips_to_nothing() {
        assert_eq!(strip_frontmatter("---\n---\n\n"), "");
    }

    #[test]
    fn the_closing_fence_must_stand_alone() {
        let text = "---\na: 1\n--- extra\nbody\n";
        assert_eq!(strip_frontmatter(text), text);
    }

    #[test]
    fn content_is_byte_preserved() {
        assert_eq!(strip_frontmatter("a\nb"), "a\nb");
        assert_eq!(strip_frontmatter("a\nb\n"), "a\nb\n");
    }

    // ------------------------------------------------------ render_markdown

    #[test]
    fn no_frontmatter_emits_the_body_alone() {
        assert_eq!(
            render_markdown("## Summary\n\nA.\n", &BTreeMap::new()),
            "## Summary\n\nA.\n"
        );
    }

    #[test]
    fn frontmatter_is_wrapped_and_sorted() {
        let rendered = render_markdown(
            "## Summary\n\nKeep secrets safe.\n",
            &map(json!({"team": "platform", "owner": "default"})),
        );
        assert_eq!(
            rendered,
            "---\nowner: default\nteam: platform\n---\n\n## Summary\n\nKeep secrets safe.\n"
        );
    }

    #[test]
    fn values_that_need_quoting_are_quoted() {
        let rendered = render_markdown("body\n", &map(json!({"description": "a: b"})));
        assert_eq!(rendered, "---\ndescription: 'a: b'\n---\n\nbody\n");
    }

    #[test]
    fn source_frontmatter_is_replaced_not_merged() {
        let rendered = render_markdown(
            "---\nsource: kept\n---\n\nbody\n",
            &map(json!({"owner": "default"})),
        );
        assert_eq!(rendered, "---\nowner: default\n---\n\nbody\n");
    }

    #[test]
    fn an_empty_map_still_strips_source_frontmatter() {
        assert_eq!(
            render_markdown("---\nsource: x\n---\n\nbody\n", &BTreeMap::new()),
            "body\n"
        );
    }

    /// A full render exercising every quoting rule at once.
    ///
    /// The contract is that these bytes are stable. The emitter is its own
    /// implementation, pinned here, so no dependency upgrade can move what a
    /// consumer sees.
    #[test]
    fn every_quoting_rule_at_once() {
        let frontmatter = map(json!({
            "team": "platform",
            "owner": "import-team",
            "description": "a: b",
            "flag": "-x",
            "count": 3,
            "enabled": true,
            "none": null,
            "empty-map": {},
            "empty-list": [],
            "nested": {"z": 1, "y": 2},
            "list": ["a", "b"],
            "multi": "a\nb"
        }));
        let rendered = render_markdown("## Summary\n\nKeep secrets safe.\n", &frontmatter);
        assert_eq!(
            rendered,
            r#"---
count: 3
description: 'a: b'
empty-list: []
empty-map: {}
enabled: true
flag: '-x'
list:
  - a
  - b
multi: "a\nb"
nested:
  'y': 2
  z: 1
none: null
owner: import-team
team: platform
---

## Summary

Keep secrets safe.
"#
        );
    }

    // ---------------------------------------------------------- render_asset

    #[test]
    fn a_file_asset_renders_its_one_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("security.md");
        std::fs::write(&path, "## Summary\n\nA.\n").unwrap();
        let asset = Asset::File(FileAsset { path: path.clone() });
        match render_asset(&asset, &map(json!({"owner": "x"}))).unwrap() {
            Rendered::File(bytes) => {
                assert_eq!(bytes, b"---\nowner: x\n---\n\n## Summary\n\nA.\n");
            }
            other => panic!("expected a file, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_asset_styles_the_entry_and_copies_artifacts_verbatim() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().join("tdd");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("SKILL.md"),
            "---\nold: 1\n---\n\n## Description\n",
        )
        .unwrap();
        std::fs::write(root.join("checklist.md"), "- plan\n").unwrap();
        std::fs::write(root.join("raw.bin"), [0u8, 1, 2, 3]).unwrap();

        let asset = Asset::Dir(DirAsset {
            path: root,
            entry: PathBuf::from("SKILL.md"),
            files: vec![
                PathBuf::from("SKILL.md"),
                PathBuf::from("checklist.md"),
                PathBuf::from("raw.bin"),
            ],
        });

        match render_asset(&asset, &map(json!({"scope": "skills"}))).unwrap() {
            Rendered::Dir(files) => {
                assert_eq!(files.len(), 3);
                assert_eq!(files[0].0, PathBuf::from("SKILL.md"));
                assert_eq!(
                    files[0].1,
                    b"---\nscope: skills\n---\n\n## Description\n".to_vec()
                );
                assert_eq!(files[1].0, PathBuf::from("checklist.md"));
                assert_eq!(files[1].1, b"- plan\n".to_vec());
                assert_eq!(files[2].0, PathBuf::from("raw.bin"));
                assert_eq!(files[2].1, vec![0u8, 1, 2, 3]);
            }
            other => panic!("expected a directory, got {other:?}"),
        }
    }
}
