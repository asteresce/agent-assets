# Contributing to agent-assets

This repository ships provider-neutral agent assets and a small engine that
emits them into consumer projects. It contains no agent-specific glue: what an
agent is, and how it discovers files, is the consumer's business.

## Terminology

- **Registry** — `rules/`, `skills/`, `agents/`; the shared asset sources.
- **Category** — one registry directory (and one top-level config key).
- **Import** — one config entry selecting a registry asset to emit.
- **Injection** — a data-driven transform applied at emit time
  (`frontmatter`, `json`).
- **Manifest** — `./agent-assets.lock`; the paths a sync owns.

## Adding an asset

1. Drop it in the registry: `rules/<name>.md`, `agents/<name>.md`, or
   `skills/<name>/` with a `SKILL.md` entry file plus artifacts.
2. Add a line to that category's `README.md`.
3. Keep the content provider-neutral: no frontmatter, no agent names, no
   references to where files land. All of that is injected by consumers.

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
