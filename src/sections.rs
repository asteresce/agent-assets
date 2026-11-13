//! Reading `##` sections out of a Markdown asset.
//!
//! Fenced code blocks are opaque: an example may contain headings of its
//! own, and a fence marker is not prose. Every reader goes through this
//! module — registry conformance, `list`'s summary — so the rule has one
//! implementation rather than one per call site.
//!
//! The schema for a category is an ordered list of `{name, required}` entries
//! from the registry's `spec.json`. An asset conforms when its `##` sections
//! are a subsequence of that list, in that order, with every required entry
//! present.

use crate::spec::Heading;

#[derive(Debug, PartialEq)]
enum LineKind {
    H1,
    H2(String),
    /// Inside a fence, or the fence line itself. Opaque to every rule here.
    Fence,
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
            out.push(LineKind::Fence);
            continue;
        }
        if let Some(marker) = fence_marker(line) {
            fence = Some(marker);
            out.push(LineKind::Fence);
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
            LineKind::Fence | LineKind::Other => {}
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
        !trimmed.is_empty() && !trimmed.starts_with('#')
    })
}

/// Every way `text` departs from `schema`; empty when it conforms. The order
/// of the messages is part of the contract — the tests assert it.
pub fn problems(text: &str, schema: &[Heading]) -> Vec<String> {
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

/// The first non-blank line of the first `##` section: the summary `list`
/// shows. Assets carry no preamble and their first section is required, so
/// this is defined for a conforming asset. Code fences are skipped — a fence
/// marker is not prose, and an example's headings are not headings.
pub fn summary(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut in_section = false;
    for (index, kind) in scan(&text).into_iter().enumerate() {
        match kind {
            LineKind::H2(_) => in_section = true,
            LineKind::Fence => continue,
            LineKind::H1 | LineKind::Other => {}
        }
        if !in_section {
            continue;
        }
        let trimmed = lines[index].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return trimmed.to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_the_first_line_of_the_first_section() {
        assert_eq!(
            summary(b"## Summary\n\nKeep secrets safe.\n\n## Guidelines\n\n- A.\n"),
            "Keep secrets safe."
        );
        assert_eq!(
            summary(b"## Description\n### A subheading\n\nTest first.\n"),
            "Test first."
        );
        assert_eq!(summary(b"# A title\n"), "");
    }

    #[test]
    fn summary_treats_code_fences_as_opaque() {
        assert_eq!(
            summary(b"## Summary\n\n```bash\ngit commit\n```\n\nAfter the fence.\n"),
            "After the fence."
        );
        assert_eq!(
            summary(b"## Summary\n\n````\n## Fake\n````\n\nReal text.\n"),
            "Real text."
        );
        // A longer fence is not closed by a shorter one.
        assert_eq!(
            summary(b"## Summary\n\n````\n```\nStill code.\n````\n\nProse.\n"),
            "Prose."
        );
    }
}
