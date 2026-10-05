# Contributing to agent-assets

This repository ships provider-neutral agent assets and a small engine that
emits them into consumer projects. It contains no agent-specific glue: what an
agent is, and how it discovers files, is the consumer's business.

## Terminology

- **Assets** — `assets/`, the shared asset sources; one directory per category.
- **Category** — one directory under the assets root (and one top-level config
  key).
- **Import** — one config entry selecting an asset to emit.
- **Injection** — a data-driven transform applied at emit time
  (`frontmatter`, `json`).
- **Manifest** — `defaults.manifest`; the paths a sync owns.

## Declared values

`spec.json` is the single source of truth for the package name, the defaults
and the per-category section schema. The engine, the flake module, the tests
and the docs generator all read it — never restate a value from it in code or
in behaviour-bearing prose (option tables, flag defaults, CLI text); refer to
the key instead (`defaults.manifest`, `defaults.skillEntry`, …). A snapshot of
this repository's layout may name the directories; the reference stays the
authority for the values.

Generated documentation is only ever a whole file under `docs/`, named
`<something>.generated.md`, produced by `scripts/gen-docs.py` from `spec.json`.
Generated content is never embedded in a hand-written page, and generated files
stay light; hand-written pages link to them. Regenerate with
`python3 scripts/gen-docs.py`, and `nix flake check` fails if a `*.generated.md`
file is missing, stale or stray.

## Adding an asset

1. Drop it in the registry (`defaults.assets`): `rules/<name>.md`,
   `agents/<name>.md`, or `skills/<name>/` with its entry file
   (`defaults.skillEntry`) plus artifacts.
2. Keep the content provider-neutral: no frontmatter, no agent names, no
   references to where files land. All of that is injected by consumers.
3. Keep the sections in the order `spec.json` declares for the category —
   `nix flake check` enforces it.

The category catalog regenerates itself (`python3 scripts/gen-docs.py`), so
there is no listing to maintain.

Adding a **category** is a new registry directory plus the matching config key
in consumers — the engine infers file-vs-directory from the source layout and
has no category list to edit.

## Changing the engine

The engine is `bin/agent-assets` (bash + jq + yq). New injections take pure
data in the config schema and are implemented there; keep the core
provider-agnostic and document the shape in `docs/api.md` with a worked
example.

Behavioural rules live in one place — `README.md`'s "How it behaves". Update
that section rather than restating behaviour in other docs.

## Constraints

- Config is pure data (JSON-serialisable): no functions, no Nix evaluation.
- Only manifest-listed paths may be overwritten or deleted.
- `check` must never modify the project.
- Emitted files carry no provenance markers; pinning lives in the consumer's
  `flake.lock`.
- `opencode.json` at the repo root is tooling for maintaining this repository.
  It is not part of the public API and must not be referenced by consumers.

## Verifying

```bash
nix flake check                              # integration suite + shellcheck
python3 -m unittest discover -s tests -v     # same suite, needs jq and yq on PATH
```

`tests/test_engine.py` drives the real script and is the executable
specification for the behaviour section. Extend it whenever behaviour changes.
