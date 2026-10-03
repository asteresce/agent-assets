# Writing assets

Shared assets in `rules/` and `agents/` are Markdown files. Skills in `skills/` are directories containing a `SKILLS.md` file plus any artifacts. Each asset must follow the section schema for its category.

## General principles

- Keep content provider-neutral: describe *what* the agent should do, not *how* a specific agent loads it.
- Use concrete, actionable language.
- Do not add a `# Title` heading. The asset name is the file or directory name, and the first heading must be the first schema section.
- All headings must match the schema exactly. Unknown headings are not allowed.
- Required sections must be present and non-empty.
- Optional sections, if used, must also be non-empty.
- Sections must appear in the order defined by the schema.

## Validation

The schema is enforced by `nix flake check`:

```bash
nix flake check
```

It is also validated by the client check command:

```bash
nix run .#check
```

The `check` command reports drift on owned files, JSON injection drift,
and orphans. It also validates section headings for imported assets
when the schema blob is supplied (the flake wires this up
automatically).

## Schemas

The per-category heading schemas are defined in `lib/schemas/headings.nix`
and exposed as `lib.schemas.headings`. The same source of truth is
consumed by both `nix flake check` (which validates shared assets) and
`nix run .#check` (which validates imported assets in the consumer's
project).

The tables below are generated from that schema by the
`docs:tables` derivation (see `lib/gen-schema-tables.nix`).

<!-- The tables below are checked-equal against `lib.schemas.headings`
     by the `nix flake check` `schema-tables` derivation. Edit
     `lib/schemas/headings.nix`, not this file. -->

### Rules

| Section | Required | Level |
|---|---|---|
| Summary | yes | 2 |
| Guidelines | yes | 2 |
| Examples | no | 2 |
| Exceptions | no | 2 |
| Rationale | no | 2 |

### Skills

Skills are directory-based. Each skill lives in its own directory under `skills/` and contains a `SKILLS.md` file plus any artifacts it needs.

| Section | Required | Level |
|---|---|---|
| Description | yes | 2 |
| Steps | yes | 2 |
| Prerequisites | no | 2 |
| Examples | no | 2 |
| Notes | no | 2 |

### Agents

| Section | Required | Level |
|---|---|---|
| Role | yes | 2 |
| Instructions | yes | 2 |
| Constraints | no | 2 |
| Examples | no | 2 |
| Tools | no | 2 |

## Templates

Copy a template from [`docs/templates/`](../docs/templates/) when adding a new asset.