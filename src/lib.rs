//! agent-assets — render, sync and check shared agent assets.
//!
//! The crate is a library plus a thin binary, so the tests drive the same
//! types the binary uses. Reading order:
//!
//! * [`spec`] — the declared values, from `spec.json`.
//! * [`config`] — the consumer's JSON document.
//! * [`registry`] — where the shared assets live.
//! * [`sections`] — the `##` sections of a Markdown asset, read through code
//!   fences.
//! * [`render`] and [`yaml_emit`] — one asset plus its injections, as bytes.
//! * [`merge`] — what `injections.json` means.
//! * [`manifest`] — what a sync owns, and what it last wrote.
//! * [`plan`] — what the project needs.
//! * [`validate`] — whether a config, and the registry, are sound.
//! * [`check`] and [`apply`] — report it, or do it.
//! * [`files`] — how anything is read from, or written to, the tree.
//! * [`cli`] and [`report`] — the arguments, and the lines we print.

pub mod apply;
pub mod check;
pub mod cli;
pub mod config;
pub mod files;
pub mod manifest;
pub mod merge;
pub mod plan;
pub mod registry;
pub mod render;
pub mod report;
pub mod sections;
pub mod spec;
pub mod validate;
pub mod yaml_emit;
