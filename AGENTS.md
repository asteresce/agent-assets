# Contributing to agent-assets

This repository ships provider-neutral agent assets and a small engine that
emits them into consumer projects. It contains no agent-specific glue: what
an agent is, and how it discovers files, is the consumer's business.

## Terminology

- **Assets** — `assets/`, the shared asset sources: its `spec.json` plus one
  directory per category.
- **Category** — one directory under the assets root (declared in the
  registry's `spec.json`, and one top-level config key).
- **Import** — one config entry selecting an asset to emit.
- **Injection** — a data-driven transform applied at emit time
  (`frontmatter`, `json`).
- **Manifest** — `defaults.manifest`; the paths a sync owns.

## Declared values

Two declarations, each read and never restated:

- `defaults.json` — the defaults; the file *is* the object, one level deep.
- `assets/spec.json` — the registry's categories: their shape (`kind`, plus
  `extension` for a file category or `entryFile` for a directory one) and
  their section schema.

The package's identity — name, description, version — is `Cargo.toml`'s
(`env!` in the engine, `builtins.fromTOML` in the flakes): no declaration
here duplicates it.

The engine, `build.rs`, the flake module and the tests all read them — never
restate a value from them in code or in behaviour-bearing prose (option
tables, flag defaults, CLI text); refer to the key instead
(`defaults.manifest`, `assets/spec.json`'s `skills.entryFile`, …). A snapshot
of this repository's layout may name the directories; the references stay the
authority for the values.

Documentation links to the declarations rather than restating values from
them. The behavioural spec is one place — the "How it behaves" section of
`README.md` — and the tests are its executable form.

## Adding an asset

1. Drop it in the registry, in the shape its category declares:
   `rules/<name>.md`, `agents/<name>.md`, or `skills/<name>/` with
   its entry file (`assets/spec.json`'s `skills.entryFile`) plus artifacts.
2. Keep the content provider-neutral: no frontmatter, no agent names, no
   references to where files land. All of that is injected by consumers.
3. Use only the `##` sections `assets/spec.json` declares for the category,
   in the order it declares them. Required sections must be present and
   non-empty; optional ones may be omitted. `cargo test` (and therefore
   `nix flake check`) enforces all of this and rejects, for the asset
   (entry file, for a directory asset):
   - an unknown, repeated or out-of-order `##` section;
   - an h1 title, frontmatter, or any text before the first `##`;
   - an empty section (a subheading alone is not content).

   `agent-assets validate` reports the same departures for one consumer's
   config, alongside the config problems that would make a run undefined.

   Subheadings (`###` and below) are free. Fenced code blocks are opaque, so
   an example may contain headings of its own. Artifacts in a directory asset
   are free-form.

The category `README.md`s describe what a category is for and how its assets
are referenced. The listing itself is `agent-assets list`'s job, so there is
no catalog to keep in step.

Changing the section vocabulary is an `assets/spec.json` change: keep a
heading's name and meaning the same across categories, never rename one in
place (existing assets would break — add the new name, migrate, then remove
the old). Then regenerate nothing: the catalog is a listing. Headings that
only one agent understands (tool lists, model names, permissions) are
provider-specific and belong in consumer frontmatter, not in an asset.

Adding a **category** is a new registry directory, an entry in the
registry's `spec.json` giving its shape and section schema, and the matching
config key in consumers — the declaration, not the layout, decides what
shape the category's assets have.

## Changing the engine

The engine is `src/` — a Rust crate with a thin binary. New behaviour takes
pure data in the config schema and is implemented in `plan`/`apply`; keep the
core provider-agnostic and document the shape in `docs/api.md` with a worked
example.

Behavioural rules live in one place — `README.md`'s "How it behaves". Update
that section rather than restating behaviour in other docs, and change the
tests in `tests/integration.rs` in the same commit: that suite is the
behaviour's executable specification.

## Constraints

- Config is pure data (JSON-serialisable): no functions, no Nix evaluation.
- Only manifest-listed paths may be overwritten or deleted.
- `check` must never modify the project.
- Emitted files carry no provenance markers; pinning lives in the consumer's
  `flake.lock`.
- `opencode.json` at the repo root is tooling for maintaining this repository.
  It is not part of the public API and must not be referenced by consumers.

## Verifying

Run these inside `nix develop` first: it provides the Rust toolchain and the
C linker that `cargo` needs to link a binary, which a bare `cargo` on NixOS
does not have.

```bash
cargo fmt --check                        # formatting, as CI runs it
cargo clippy --all-targets -- -D warnings # lints, as CI runs them
cargo test                              # behaviour suite + registry conformance
nix flake check                         # the package build, which runs cargo test
```

`tests/integration.rs` drives the built binary and is the executable
specification for the behaviour section. Extend it whenever behaviour changes.

## Releasing

A release is `Cargo.toml` → commit → tag `vX.Y.Z`, in that order: set
`version`, commit, tag that commit `v<version>`. A tag that is not the
version in `Cargo.toml` fails the release build rather than shipping a
mismatched binary.

Pushing the tag runs `.github/workflows/release.yml`; see it for the targets
and the archive layout. It also publishes to crates.io, last, once the
version has matched the tag and four platforms have built. That step needs
`CARGO_REGISTRY_TOKEN` under Settings → Secrets and variables → Actions, and
it is the one irreversible step in the process.

Release notes are generated from `git log`, listing the `feat:`, `fix:` and
`docs:` subjects between the tags. The subject is the public changelog line,
so write one for somebody who did not read the diff.
