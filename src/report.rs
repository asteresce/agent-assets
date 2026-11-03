//! Output formatting: the status words and the shape of one report line.
//!
//! Paths are printed plain so the output stays greppable; only the status
//! word and the attribution are coloured.

use std::path::PathBuf;

use crate::cli::ColorWhen;

const DIM: &str = "\u{1b}[2m";
const RESET: &str = "\u{1b}[0m";
const GREEN: &str = "\u{1b}[32m";
const RED: &str = "\u{1b}[31m";
const YELLOW: &str = "\u{1b}[33m";
const CYAN: &str = "\u{1b}[36m";

/// Where a problem came from: the import that produced the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub category: String,
    pub name: String,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(from {}/{})", self.category, self.name)
    }
}

/// Every word the engine can print.
///
/// `sync` prints the first four for each change it makes; `check` prints the
/// rest for each problem it finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Write,
    Update,
    Remove,
    Merge,
    Missing,
    Drift,
    Extra,
    Orphan,
    Inject,
    Conflict,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Write => "WRITE",
            Status::Update => "UPDATE",
            Status::Remove => "REMOVE",
            Status::Merge => "MERGE",
            Status::Missing => "MISSING",
            Status::Drift => "DRIFT",
            Status::Extra => "EXTRA",
            Status::Orphan => "ORPHAN",
            Status::Inject => "INJECT",
            Status::Conflict => "CONFLICT",
        }
    }

    /// Red is for missing, deleted and refused; yellow for changed and
    /// worth a look; green for created; cyan for merged.
    fn color(self) -> &'static str {
        match self {
            Status::Write => GREEN,
            Status::Merge => CYAN,
            Status::Update | Status::Drift | Status::Extra | Status::Inject => YELLOW,
            Status::Remove | Status::Missing | Status::Orphan | Status::Conflict => RED,
        }
    }
}

/// One thing to print. Commands build these; only the reporter formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub status: Status,
    pub path: PathBuf,
    pub origin: Option<Origin>,
}

impl Line {
    pub fn new(status: Status, path: impl Into<PathBuf>, origin: Option<Origin>) -> Line {
        Line {
            status,
            path: path.into(),
            origin,
        }
    }
}

/// Formats report lines.
#[derive(Debug, Clone, Copy)]
pub struct Reporter {
    color: bool,
}

impl Reporter {
    pub fn new(when: ColorWhen) -> Reporter {
        Reporter {
            color: should_color(when),
        }
    }

    /// Build a reporter that colours unconditionally, for tests.
    pub fn with_color(color: bool) -> Reporter {
        Reporter { color }
    }

    /// `STATUS path`, or `STATUS path (from category/name)` when the problem
    /// belongs to a particular import.
    pub fn format(&self, line: &Line) -> String {
        let mut out = String::new();
        if self.color {
            out.push_str(line.status.color());
            out.push_str(line.status.word());
            out.push_str(RESET);
        } else {
            out.push_str(line.status.word());
        }
        out.push(' ');
        out.push_str(&line.path.display().to_string());
        if let Some(origin) = &line.origin {
            out.push(' ');
            if self.color {
                out.push_str(DIM);
                out.push_str(&origin.to_string());
                out.push_str(RESET);
            } else {
                out.push_str(&origin.to_string());
            }
        }
        out.push('\n');
        out
    }

    pub fn print(&self, line: &Line) {
        print!("{}", self.format(line));
    }
}

fn should_color(when: ColorWhen) -> bool {
    match when {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => {
            if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
                return false;
            }
            match std::env::var("TERM") {
                Ok(term) if term != "dumb" => {}
                _ => return false,
            }
            std::io::IsTerminal::is_terminal(&std::io::stdout())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Origin {
        Origin {
            category: "rules".to_string(),
            name: "security".to_string(),
        }
    }

    #[test]
    fn a_plain_line_is_greppable() {
        // `Path` printing is not directly assertable, so assert on the
        // formatter's inputs instead of capturing stdout.
        let origin = origin();
        assert_eq!(origin.to_string(), "(from rules/security)");
    }

    #[test]
    fn every_status_has_a_distinct_word() {
        let words = [
            Status::Write,
            Status::Update,
            Status::Remove,
            Status::Merge,
            Status::Missing,
            Status::Drift,
            Status::Extra,
            Status::Orphan,
            Status::Inject,
            Status::Conflict,
        ]
        .map(Status::word);
        let unique: std::collections::BTreeSet<_> = words.iter().collect();
        assert_eq!(unique.len(), words.len());
    }

    #[test]
    fn a_reporter_can_be_built_for_each_colour_policy() {
        for when in [ColorWhen::Always, ColorWhen::Never, ColorWhen::Auto] {
            let _ = Reporter::new(when);
        }
        assert!(Reporter::with_color(true).color);
        assert!(!Reporter::with_color(false).color);
    }
}
