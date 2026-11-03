//! Report what is wrong without touching the project.
//!
//! `check` is `plan` plus printing. It cannot mutate anything, because this
//! module never opens a file for writing.

use crate::plan::Plan;
use crate::report::Line;

/// The lines `check` prints, in the order it prints them. Empty means clean.
pub fn run(plan: &Plan) -> Vec<Line> {
    plan.problems
        .iter()
        .map(|problem| {
            Line::new(
                problem.status(),
                problem.path().to_path_buf(),
                problem.origin().cloned(),
            )
        })
        .collect()
}
