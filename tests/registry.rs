//! Registry conformance: the shipped assets follow the section schema in
//! `spec.json`.
//!
//! Repo-side only. A consumer's `check` deliberately does not validate
//! headings — that is the registry's job, and this test is the executable
//! version of the "adding an asset" rules in `AGENTS.md`.
//!
//! The schema for a category is an ordered list of `{name, required}` entries.
//! An asset conforms when its `##` sections are a subsequence of that list, in
//! that order, with every required entry present. Fenced code blocks are
//! opaque, so an example may contain headings of its own.

use std::path::{Path, PathBuf};

use agent_assets::spec::{Heading, Spec};

const REPO: &str = env!("CARGO_MANIFEST_DIR");

// ------------------------------------------------------- section scanning

#[derive(Debug, PartialEq)]
enum LineKind {
    H1,
    H2(String),
    Other,
}

/// Leading spaces of a line (a fence may be indented up to three).
fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// A run of three or more backticks or tildes, with its character and length.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let rest = &line[leading_spaces(line)..];
    let first = rest.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = rest.chars().take_while(|c| *c == first).count();
    if count < 3 {
        return None;
    }
    Some((first, count))
}

/// A closing fence carries no info string (CommonMark); an opener may.
fn is_fence_close(line: &str, open: (char, usize)) -> bool {
    let Some((character, length)) = fence_marker(line) else {
        return false;
    };
    if character != open.0 || length < open.1 {
        return false;
    }
    let rest = &line[leading_spaces(line)..];
    rest[length..].chars().all(|c| c == ' ' || c == '\t')
}

fn h2_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("## ")?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.trim().to_string())
}

fn is_h1(line: &str) -> bool {
    line.strip_prefix('#')
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Classify every line, treating fenced code blocks as opaque content.
fn scan(text: &str) -> Vec<LineKind> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        if let Some(open) = fence {
            if is_fence_close(line, open) {
                fence = None;
            }
            out.push(LineKind::Other);
            continue;
        }
        if let Some(marker) = fence_marker(line) {
            fence = Some(marker);
            out.push(LineKind::Other);
        } else if let Some(name) = h2_name(line) {
            out.push(LineKind::H2(name));
        } else if is_h1(line) {
            out.push(LineKind::H1);
        } else {
            out.push(LineKind::Other);
        }
    }
    out
}

/// Split into (preamble, sections, h1 lines).
fn parse_sections(text: &str) -> (String, Vec<(String, String)>, Vec<String>) {
    let mut preamble: Vec<&str> = Vec::new();
    let mut sections: Vec<(String, Vec<&str>)> = Vec::new();
    let mut h1s: Vec<String> = Vec::new();

    for (line, kind) in text.lines().zip(scan(text)) {
        match kind {
            LineKind::H2(name) => {
                sections.push((name, Vec::new()));
                continue;
            }
            LineKind::H1 => h1s.push(line.to_string()),
            LineKind::Other => {}
        }
        match sections.last_mut() {
            Some((_, body)) => body.push(line),
            None => preamble.push(line),
        }
    }

    (
        preamble.join("\n"),
        sections
            .into_iter()
            .map(|(name, body)| (name, body.join("\n")))
            .collect(),
        h1s,
    )
}

/// A section carries content when it has a line that is neither blank nor a
/// heading. A subheading alone is not content.
fn has_content(body: &str) -> bool {
    body.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !line.trim_start().starts_with('#')
    })
}

/// Every way `text` departs from `schema`; empty when it conforms. The order
/// of the messages is part of the contract — the linter's own tests assert it.
fn problems(text: &str, schema: &[Heading]) -> Vec<String> {
    let canon: Vec<&str> = schema.iter().map(|h| h.name.as_str()).collect();
    let required: Vec<&str> = schema
        .iter()
        .filter(|h| h.required)
        .map(|h| h.name.as_str())
        .collect();
    let (preamble, sections, h1s) = parse_sections(text);
    let names: Vec<&str> = sections.iter().map(|(n, _)| n.as_str()).collect();
    let mut found = Vec::new();

    if !preamble.trim().is_empty() {
        found.push(
            "content before the first `##` section \
             (frontmatter and titles are injected by consumers)"
                .to_string(),
        );
    }
    for line in &h1s {
        found.push(format!("h1 title '{line}' (assets carry no title)"));
    }
    let mut seen: Vec<&str> = Vec::new();
    for name in &names {
        if seen.contains(name) {
            continue;
        }
        seen.push(name);
        if !canon.contains(name) {
            found.push(format!("unknown section '{name}'"));
        }
        if names.iter().filter(|n| *n == name).count() > 1 {
            found.push(format!("duplicate section '{name}'"));
        }
    }
    for name in &required {
        if !names.contains(name) {
            found.push(format!("missing required section '{name}'"));
        }
    }

    let mut present: Vec<&str> = Vec::new();
    for name in &names {
        if canon.contains(name) && !present.contains(name) {
            present.push(name);
        }
    }
    let in_canon_order: Vec<&str> = canon
        .iter()
        .copied()
        .filter(|name| present.contains(name))
        .collect();
    if present != in_canon_order {
        found.push(format!("sections out of order: {}", present.join(", ")));
    }

    for (heading, body) in &sections {
        if !has_content(body) {
            found.push(format!("empty section '{heading}'"));
        }
    }
    found
}

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

/// Every importable asset of a category: files, or a directory's entry file.
/// Catalog indexes (`README.md`) are not assets, and artifacts inside a
/// directory asset are free-form.
fn assets(assets_dir: &str, skill_entry: &str, category: &str) -> Vec<PathBuf> {
    let root = Path::new(REPO).join(assets_dir).join(category);
    let mut out = Vec::new();
    for entry in
        std::fs::read_dir(&root).unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            out.push(path.join(skill_entry));
        } else if entry.file_name() != "README.md" {
            out.push(path);
        }
    }
    out.sort();
    out
}

// ------------------------------------------------------------- the schema

#[test]
fn every_schema_category_exists_in_the_registry() {
    let spec = Spec::parse().unwrap();
    for category in spec.headings.keys() {
        let dir = Path::new(REPO).join(&spec.defaults.assets).join(category);
        assert!(dir.is_dir(), "{category}: missing registry directory");
    }
}

#[test]
fn every_registry_category_has_a_schema() {
    let spec = Spec::parse().unwrap();
    let root = Path::new(REPO).join(&spec.defaults.assets);
    for entry in std::fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            assert!(
                spec.headings.contains_key(&name),
                "{name} has no section schema in spec.json"
            );
        }
    }
}

#[test]
fn schema_entries_are_well_formed() {
    let spec = Spec::parse().unwrap();
    for (category, schema) in &spec.headings {
        let mut names = std::collections::BTreeSet::new();
        for entry in schema {
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
        assert!(
            schema[0].required,
            "{category}: the first section must be required"
        );
    }
}

// ---------------------------------------------------------- the registry

#[test]
fn every_shipped_asset_matches_the_section_schema() {
    let spec = Spec::parse().unwrap();
    for (category, schema) in &spec.headings {
        for asset in assets(&spec.defaults.assets, &spec.defaults.skill_entry, category) {
            assert!(
                asset.is_file(),
                "{category}: missing entry file {}",
                asset.display()
            );
            let text = std::fs::read_to_string(&asset).unwrap();
            assert_eq!(
                problems(&text, schema),
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
    problems(
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
