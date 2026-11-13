//! Registry conformance: the shipped assets follow the declaration in
//! `assets/spec.json`.
//!
//! The linter itself lives in [`agent_assets::sections`]; this file is its
//! executable specification plus the repo-side checks around it — the
//! "adding an asset" rules in `AGENTS.md`. A consumer's `check` deliberately
//! does not validate headings; that is the registry's job.
//!
//! The declaration gives each category its shape — `kind`, plus `extension`
//! for a file category or `entryFile` for a directory one — and its section
//! schema: an ordered list of `{name, required}` entries. An asset conforms
//! when its `##` sections are a subsequence of that list, in that order,
//! with every required entry present. Fenced code blocks are opaque, so an
//! example may contain headings of its own.

use std::path::{Path, PathBuf};

use agent_assets::sections;
use agent_assets::spec::{AssetKind, AssetSpec, AssetSpecs, Heading};

const REPO: &str = env!("CARGO_MANIFEST_DIR");

/// Section names are `UpperCamel`-ish: an uppercase letter, then letters and
/// spaces, ending on a letter.
fn valid_section_name(name: &str) -> bool {
    let mut chars = name.chars();
    let (Some(first), Some(last)) = (chars.next(), name.chars().next_back()) else {
        return false;
    };
    if name.chars().count() < 2 {
        return false;
    }
    first.is_ascii_uppercase()
        && last.is_ascii_alphabetic()
        && name.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
}

/// The shipped declaration, read the way a registry directory reads its own.
fn specs() -> AssetSpecs {
    let path = Path::new(REPO).join("assets/spec.json");
    let text = std::fs::read_to_string(&path).unwrap();
    AssetSpecs::parse(&text, "assets/spec.json").unwrap()
}

/// The name a category reserves for its catalog index, which is documentation
/// about the category rather than an asset. Same call as `registry.rs`.
fn is_catalog_index(name: &str) -> bool {
    name == "README" || name == "README.md"
}

/// Every importable asset of a category, as the entry file whose sections
/// are the schema's business. Catalog indexes are not assets, and artifacts
/// inside a directory asset are free-form.
fn assets(spec: &AssetSpec, category: &str) -> Vec<PathBuf> {
    let root = Path::new(REPO).join("assets").join(category);
    let mut out = Vec::new();
    for entry in
        std::fs::read_dir(&root).unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if is_catalog_index(&entry.file_name().to_string_lossy()) {
            continue;
        }
        match spec.kind {
            AssetKind::File => out.push(path),
            AssetKind::Dir => {
                let entry_file = spec.entry_file.as_deref().expect("declaration");
                out.push(path.join(entry_file));
            }
        }
    }
    out.sort();
    out
}

// ------------------------------------------------------------- the schema

#[test]
fn every_schema_category_exists_in_the_registry() {
    let specs = specs();
    for (category, _) in specs.iter() {
        let dir = Path::new(REPO).join("assets").join(category);
        assert!(dir.is_dir(), "{category}: missing registry directory");
    }
}

#[test]
fn every_registry_category_has_a_schema() {
    let specs = specs();
    let root = Path::new(REPO).join("assets");
    for entry in std::fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            assert!(
                specs.get(&name).is_some(),
                "{name} has no declaration in assets/spec.json"
            );
        }
    }
}

#[test]
fn schema_entries_are_well_formed() {
    // Shape invariants (`entryFile` iff `dir`, `extension` iff `file`, a
    // first section that is required) are `AssetSpecs::parse`'s to enforce —
    // `specs()` would have failed. What stays here is the naming rule the
    // linter's callers rely on.
    let specs = specs();
    for (category, spec) in specs.iter() {
        let mut names = std::collections::BTreeSet::new();
        for entry in &spec.headings {
            assert!(
                names.insert(entry.name.as_str()),
                "{category}: duplicate section name {:?}",
                entry.name
            );
            assert!(
                valid_section_name(&entry.name),
                "{category}: malformed section name {:?}",
                entry.name
            );
        }
    }
}

#[test]
fn the_declaration_on_disk_matches_the_embedded_one() {
    // Guards against shipping a binary built from a stale declaration.
    assert_eq!(specs(), AssetSpecs::embedded().unwrap());
}

// ---------------------------------------------------------- the registry

#[test]
fn every_asset_matches_its_declared_shape() {
    let specs = specs();
    for (category, spec) in specs.iter() {
        let root = Path::new(REPO).join("assets").join(category);
        for entry in std::fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_catalog_index(&name) {
                continue;
            }
            let path = entry.path();
            match spec.kind {
                AssetKind::File => {
                    let extension = spec.extension.as_deref().expect("declaration");
                    assert!(
                        path.is_file(),
                        "{category}/{name}: a file category holds .{extension} files"
                    );
                    assert!(
                        name.ends_with(&format!(".{extension}")),
                        "{category}/{name}: not a .{extension} file"
                    );
                }
                AssetKind::Dir => {
                    assert!(
                        path.is_dir(),
                        "{category}/{name}: a directory category holds directories"
                    );
                    let entry_file = spec.entry_file.as_deref().expect("declaration");
                    assert!(
                        path.join(entry_file).is_file(),
                        "{category}/{name}: missing entry file {entry_file}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_shipped_asset_matches_the_section_schema() {
    let specs = specs();
    for (category, spec) in specs.iter() {
        for asset in assets(spec, category) {
            assert!(
                asset.is_file(),
                "{category}: missing entry file {}",
                asset.display()
            );
            let text = std::fs::read_to_string(&asset).unwrap();
            assert_eq!(
                sections::problems(&text, &spec.headings),
                Vec::<String>::new(),
                "{}: {}",
                category,
                asset.display()
            );
        }
    }
}

// ------------------------------------------------- the linter's own tests

fn schema(entries: &[(&str, bool)]) -> Vec<Heading> {
    entries
        .iter()
        .map(|(name, required)| Heading {
            name: (*name).to_string(),
            required: *required,
        })
        .collect()
}

fn check(text: &str) -> Vec<String> {
    sections::problems(
        text,
        &schema(&[
            ("Summary", true),
            ("Scope", false),
            ("Guidelines", true),
            ("Examples", false),
        ]),
    )
}

#[test]
fn a_minimal_asset_conforms() {
    assert_eq!(
        check("## Summary\n\nA.\n\n## Guidelines\n\n- B.\n"),
        Vec::<String>::new()
    );
}

#[test]
fn optional_sections_may_appear_in_order() {
    let text = "## Summary\nA\n## Scope\nB\n## Guidelines\nC\n## Examples\nD\n";
    assert_eq!(check(text), Vec::<String>::new());
}

#[test]
fn a_missing_required_section_is_reported() {
    assert_eq!(
        check("## Summary\nA\n"),
        vec!["missing required section 'Guidelines'"]
    );
}

#[test]
fn an_unknown_section_is_reported() {
    assert_eq!(
        check("## Summary\nA\n## Guidelines\nB\n## Notes\nC\n"),
        vec!["unknown section 'Notes'"]
    );
}

#[test]
fn out_of_order_sections_are_reported() {
    assert_eq!(
        check("## Guidelines\nB\n## Summary\nA\n"),
        vec!["sections out of order: Guidelines, Summary"]
    );
}

#[test]
fn an_optional_section_out_of_order_is_reported() {
    let text = "## Summary\nA\n## Examples\nD\n## Scope\nB\n## Guidelines\nC\n";
    let found = check(text);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("out of order"), "{found:?}");
}

#[test]
fn a_duplicate_section_is_reported() {
    assert_eq!(
        check("## Summary\nA\n## Summary\nB\n## Guidelines\nC\n"),
        vec!["duplicate section 'Summary'"]
    );
}

#[test]
fn heading_names_are_case_sensitive() {
    let found = check("## summary\nA\n## Guidelines\nB\n");
    assert!(found.contains(&"unknown section 'summary'".to_string()));
    assert!(found.contains(&"missing required section 'Summary'".to_string()));
}

#[test]
fn extra_space_after_the_marker_is_tolerated() {
    assert_eq!(
        check("##  Summary\nA\n##   Guidelines\nB\n"),
        Vec::<String>::new()
    );
}

#[test]
fn a_double_spaced_h1_is_still_an_h1() {
    let found = check("#  Title\n## Summary\nA\n## Guidelines\nB\n");
    assert!(found.contains(&"h1 title '#  Title' (assets carry no title)".to_string()));
}

#[test]
fn a_fenced_info_string_does_not_close_a_block() {
    let text = "## Summary\nA\n## Guidelines\n```\n```bash\n## Fake\n```\n";
    assert_eq!(check(text), Vec::<String>::new());
}

#[test]
fn an_empty_section_is_reported() {
    assert_eq!(
        check("## Summary\n\n## Guidelines\n- B\n"),
        vec!["empty section 'Summary'"]
    );
}

#[test]
fn subheadings_do_not_count_as_content() {
    assert_eq!(
        check("## Summary\n### Only a subheading\n## Guidelines\n- B\n"),
        vec!["empty section 'Summary'"]
    );
}

#[test]
fn subheadings_are_free_inside_a_section() {
    let text = "## Summary\nA\n## Guidelines\n### Naming\n- B\n### Layout\n- C\n";
    assert_eq!(check(text), Vec::<String>::new());
}

#[test]
fn an_h1_title_is_rejected() {
    let found = check("# Title\n## Summary\nA\n## Guidelines\nB\n");
    let h1s: Vec<String> = found.iter().filter(|f| f.contains("h1")).cloned().collect();
    assert_eq!(h1s, vec!["h1 title '# Title' (assets carry no title)"]);
}

#[test]
fn content_before_the_first_section_is_rejected() {
    let found = check("---\nname: x\n---\n## Summary\nA\n## Guidelines\nB\n");
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("before the first"), "{found:?}");
}

#[test]
fn headings_inside_fences_are_content() {
    let text = "## Summary\nA\n\
                ## Guidelines\n\
                Write headings like this:\n\n\
                ```markdown\n# Title\n## Notes\n```\n\
                ## Examples\n\
                ~~~\n## Another\n~~~\n";
    assert_eq!(check(text), Vec::<String>::new());
}

#[test]
fn a_longer_fence_is_not_closed_by_a_shorter_one() {
    let text = "## Summary\nA\n## Guidelines\n````\n```\n## Notes\n````\n";
    assert_eq!(check(text), Vec::<String>::new());
}

#[test]
fn an_unclosed_fence_swallows_the_rest() {
    let found = check("## Summary\nA\n## Guidelines\n```\n## Examples\nD\n");
    assert_eq!(found, Vec::<String>::new());
}
