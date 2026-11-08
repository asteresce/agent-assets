//! Report what is wrong without touching the project.
//!
//! `check` is `plan` plus printing. It cannot mutate anything, because this
//! module never opens a file for writing. `status` is the same plan viewed
//! without the filter: every path, then a tally.

use std::collections::BTreeSet;
use std::path::Path;

use crate::plan::Plan;
use crate::report::{Line, Status};

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
    parts.push(counted(per_import.len(), "import", "imports"));
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

    for (status, word) in [(Status::Orphan, "orphan"), (Status::Inject, "injection")] {
        let n = plan
            .problems
            .iter()
            .filter(|problem| problem.status == status)
            .count();
        if n > 0 {
            parts.push(counted(n, word, &format!("{word}s")));
        }
    }

    let custom = plan.custom.len();
    parts.push(format!(
        "{custom} {}",
        if custom == 1 {
            "custom file"
        } else {
            "custom files"
        }
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

fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
