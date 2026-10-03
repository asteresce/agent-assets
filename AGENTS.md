# Agent contributor guide

This repository is a Nix flake module that shares provider-neutral agent assets (rules, skills, and agent prompts) and lets consumers adapt them for their chosen agent. It does not implement any agent-specific glue.

## Terminology

- **Asset category** — one of `rules`, `skills`, or `agents`.
- **Asset source** — a Markdown file containing guidance, a skill, or a prompt.
- **Import declaration** — the data set a consumer writes to import an asset (`{ name, rename, destination, injections }`).
- **Injection** — a data-driven transformation applied to an asset before emission.
- **Emission target** — the project-tree overlay the consumer applies.
- **Lock file** — `./agent-assets.lock` (path is configurable via
  `agentAssets.manifest`). Records the emitted state for idempotent sync
  and orphan removal.

## Where shared assets live

- `rules/` for provider-neutral rules.
- `skills/` for provider-neutral skills.
- `agents/` for provider-neutral agent prompts.

Each asset should describe *what* the agent should do, not *how* a specific agent loads it.

## Asset section schema

Every shared asset must conform to the section schema for its category. The schemas are defined in `lib/schemas/headings.nix` and documented in [`docs/writing-assets.md`](docs/writing-assets.md).

- No `# Title` heading. The file starts with the first schema section.
- Required sections must be present and non-empty.
- Optional sections, if used, must be non-empty.
- All headings must match the schema exactly; unknown headings are not allowed.
- Sections must appear in schema order.

Run `nix flake check` to validate repository assets. Client check (`nix run .#check`) validates imported assets as well.

## How to add a shared asset

1. Create a new Markdown file in `rules/` or `agents/`, or a new directory with a `SKILLS.md` file in `skills/`.
2. Copy the template from `docs/templates/<category>.md` (for skills, copy `docs/templates/skills/SKILLS.md` and any artifacts into the new directory).
3. Fill in the required sections and any optional sections you need.
4. Keep content provider-neutral.
5. Add it to the category `README.md`.
6. Run `nix flake check` to validate.
7. Add a usage snippet to `docs/examples/` if it helps.

## How to add an injection type

1. Accept pure data in the configuration schema.
2. Implement the transformation in `scripts/agent_assets/<name>.py`. It is
   invoked from `scripts/agent_assets/overlay.py` (during build) or
   `scripts/agent_assets/sync.py` (during sync). Do not expose it as a Nix
   helper — the configuration is the public interface.
3. Document it in `docs/injections/<name>.md`.
4. Keep the core docs provider-agnostic.

## How to document agent integration

This module must not know about any agent. If you want to show how to use the emitted assets with a concrete agent, add a client-side example under `docs/examples/`:

1. Do not add agent-specific options to the module or to `docs/api.md`.
2. Keep agent-specific details out of `docs/concepts.md`.
3. Frame the example as "the emitted tree can be consumed like this".

## Note on `opencode.json`

The `opencode.json` file in this repository is only for maintaining this project. It is not part of the public API and should not be referenced by consumers.

## Configuration constraints

- Client configuration is declared in one place (`agentAssets.config`).
- The configuration is pure data: no functions, no Nix variable expansion. This lets the same config be moved to JSON later.
- Injections support both global (category-level) and per-import configuration.
- Imports select only from the shared registry (`name`).
- Provenance is tracked by a JSON lock file in the consumer project (default `./agent-assets.lock`), not by markers embedded in emitted files.

## Documentation style

- Keep `docs/concepts.md` and `docs/api.md` provider-agnostic.
- Put agent-specific examples in `docs/examples/`.
- Use concrete Nix snippets in examples, not hand-wavy descriptions.
