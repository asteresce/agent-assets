//! Report what is wrong without touching the project.
//!
//! `check` is `plan` plus printing. It cannot mutate anything, because this
//! module never opens a file for writing. `status` is the same plan viewed
//! without the filter: every path, then a tally.

use std::collections::BTreeSet;
use std::path::Path;

use crate::plan::Plan;
use crate::report::{self, Line, Status};

/// The lines `check` prints, in the order it prints them. Empty means clean.
pub fn run(plan: &Plan) -> Vec<Line> {
    plan.problems
        .iter()
        .map(|problem| Line::new(problem.status, problem.path.clone(), problem.origin.clone()))
        .collect()
}

/// The lines `status` prints: every emitted file with its state, then the
/// paths that are not emitted files — a blocked asset root, a stray inside a
/// managed directory, an orphan, a pending injection.
pub fn status(plan: &Plan) -> Vec<Line> {
    let mut lines: Vec<Line> = plan
        .states
        .iter()
        .map(|state| Line::new(state.status, state.path.clone(), Some(state.origin.clone())))
        .collect();

    let emitted: BTreeSet<&Path> = plan
        .states
        .iter()
        .map(|state| state.path.as_path())
        .collect();
    for problem in &plan.problems {
        if emitted.contains(problem.path.as_path()) {
            continue;
        }
        lines.push(Line::new(
            problem.status,
            problem.path.clone(),
            problem.origin.clone(),
        ));
    }
    lines
}

/// The tally `status` closes with.
///
/// An import gets one state — the worst of its files, or of an obstacle
/// sitting at the asset's own path — so "matching" and "conflicted" count
/// imports rather than files. Orphans and injections are not states of an
/// import (the import is gone, or there is no file to be in a state), so
/// they are counted as the path-level facts they are.
pub fn summary(plan: &Plan) -> String {
    let mut parts = Vec::new();

    let per_import = import_states(plan);
    parts.push(report::counted(per_import.len(), "import", "imports"));
    for status in [
        Status::Clean,
        Status::Extra,
        Status::Drift,
        Status::Missing,
        Status::Conflict,
    ] {
        let n = per_import.iter().filter(|state| **state == status).count();
        if n > 0 || status == Status::Clean {
            parts.push(format!("{n} {}", status.word().to_lowercase()));
        }
    }

    // Orphans and injections belong to no import: one is gone, the other is
    // not a file that could be in a state.
    for (status, one, many) in [
        (Status::Orphan, "orphan", "orphans"),
        (Status::Inject, "injection", "injections"),
    ] {
        let n = plan
            .problems
            .iter()
            .filter(|problem| problem.status == status)
            .count();
        if n > 0 {
            parts.push(report::counted(n, one, many));
        }
    }

    parts.push(report::counted(
        plan.custom.len(),
        "custom file",
        "custom files",
    ));
    parts.join(" · ")
}

/// One state per import, in manifest order.
fn import_states(plan: &Plan) -> Vec<Status> {
    plan.manifest
        .0
        .iter()
        .map(|row| {
            let mut worst = Status::Clean;
            let mut raise = |status: Status| {
                if status.severity() > worst.severity() {
                    worst = status;
                }
            };
            for file in &row.files {
                if let Some(state) = plan.states.iter().find(|state| state.path == file.path) {
                    raise(state.status);
                }
            }
            // An obstacle at the asset's own path outranks whatever sits
            // under it: there is nothing under it yet.
            for problem in &plan.problems {
                if problem.path == row.path {
                    raise(problem.status);
                }
            }
            worst
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::manifest::{Asset, FileEntry, Manifest};
    use crate::plan::{FileState, Problem};
    use crate::report::Origin;

    fn origin() -> Origin {
        Origin {
            category: "rules".to_string(),
            name: "security".to_string(),
        }
    }

    fn state(path: &str, status: Status) -> FileState {
        FileState {
            path: PathBuf::from(path),
            origin: origin(),
            status,
        }
    }

    fn problem(path: &str, status: Status) -> Problem {
        Problem {
            status,
            path: PathBuf::from(path),
            origin: None,
        }
    }

    /// A manifest row per `(asset path, emitted file paths)`.
    fn manifest(rows: &[(&str, &[&str])]) -> Manifest {
        Manifest(
            rows.iter()
                .map(|(path, files)| Asset {
                    path: PathBuf::from(*path),
                    files: files
                        .iter()
                        .map(|file| FileEntry {
                            path: PathBuf::from(*file),
                            hash: "sha256:0".to_string(),
                        })
                        .collect(),
                })
                .collect(),
        )
    }

    fn plan(manifest: Manifest, states: Vec<FileState>, problems: Vec<Problem>) -> Plan {
        Plan {
            tree: Vec::new(),
            manifest,
            actions: Vec::new(),
            problems,
            states,
            custom: Vec::new(),
        }
    }

    #[test]
    fn check_prints_nothing_when_the_plan_has_no_problems() {
        assert!(run(&plan(manifest(&[]), vec![], vec![])).is_empty());
    }

    #[test]
    fn check_prints_one_line_per_problem_in_order() {
        let plan = plan(
            manifest(&[]),
            vec![],
            vec![
                problem("a.md", Status::Missing),
                problem("b.md", Status::Drift),
            ],
        );

        let lines = run(&plan);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0].status, Status::Missing);
        assert_eq!(lines[0].path, PathBuf::from("a.md"));
        assert_eq!(lines[1].status, Status::Drift);
    }

    /// `status` is the states, then only the problems that have no state --
    /// a file that is both emitted and wrong is one line, not two.
    #[test]
    fn status_prints_the_emitted_files_then_the_paths_that_have_no_state() {
        let plan = plan(
            manifest(&[]),
            vec![
                state("emitted.md", Status::Clean),
                state("other.md", Status::Drift),
            ],
            vec![
                problem("emitted.md", Status::Missing),
                problem("gone.md", Status::Orphan),
            ],
        );

        let lines = status(&plan);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0].status, Status::Clean);
        assert_eq!(lines[1].status, Status::Drift);
        // The problem on emitted.md is already covered by its state; the
        // orphan has no state of its own, so it is printed.
        assert_eq!(lines[2].status, Status::Orphan);
        assert_eq!(lines[2].path, PathBuf::from("gone.md"));
    }

    #[test]
    fn the_summary_counts_imports_not_files() {
        let plan = plan(
            manifest(&[("skill", &["skill/a.md", "skill/b.md"][..])]),
            vec![
                state("skill/a.md", Status::Clean),
                state("skill/b.md", Status::Drift),
            ],
            vec![],
        );

        assert_eq!(
            summary(&plan),
            "1 import · 0 clean · 1 drift · 0 custom files"
        );
    }

    #[test]
    fn an_import_takes_the_worst_of_its_files() {
        let plan = plan(
            manifest(&[("skill", &["skill/a.md", "skill/b.md"][..])]),
            vec![
                state("skill/a.md", Status::Clean),
                state("skill/b.md", Status::Conflict),
            ],
            vec![],
        );

        assert_eq!(
            summary(&plan),
            "1 import · 0 clean · 1 conflict · 0 custom files"
        );
    }

    /// A file nobody can see under the asset yet does not make it clean: the
    /// obstacle at the asset's own path is the state.
    #[test]
    fn an_obstacle_at_the_asset_path_outranks_its_files() {
        let plan = plan(
            manifest(&[("skill", &["skill/a.md"][..])]),
            vec![state("skill/a.md", Status::Clean)],
            vec![problem("skill", Status::Missing)],
        );

        assert_eq!(
            summary(&plan),
            "1 import · 0 clean · 1 missing · 0 custom files"
        );
    }

    #[test]
    fn orphans_and_injections_are_path_facts_not_import_states() {
        let plan = plan(
            manifest(&[]),
            vec![],
            vec![
                problem("gone.md", Status::Orphan),
                problem("opencode.json", Status::Inject),
            ],
        );

        assert_eq!(
            summary(&plan),
            "0 imports · 0 clean · 1 orphan · 1 injection · 0 custom files"
        );
    }

    #[test]
    fn custom_files_are_counted_separately_from_imports() {
        let mut plan = plan(
            manifest(&[("skill", &["skill/a.md"][..])]),
            vec![state("skill/a.md", Status::Clean)],
            vec![],
        );
        plan.custom.push(PathBuf::from("skill/mine.md"));

        assert_eq!(summary(&plan), "1 import · 1 clean · 1 custom file");
    }
}
