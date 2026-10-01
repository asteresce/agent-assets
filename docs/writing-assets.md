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
nix run .#agentAssets.check
```

## Rules schema

| Section | Required | Level |
|---|---|---|
| Summary | yes | 2 |
| Guidelines | yes | 2 |
| Examples | no | 2 |
| Exceptions | no | 2 |
| Rationale | no | 2 |

## Skills schema

Skills are directory-based. Each skill lives in its own directory under `skills/` and contains a `SKILLS.md` file plus any artifacts it needs.

| Section | Required | Level |
|---|---|---|
| Description | yes | 2 |
| Steps | yes | 2 |
| Prerequisites | no | 2 |
| Examples | no | 2 |
| Notes | no | 2 |

## Agents schema

| Section | Required | Level |
|---|---|---|
| Role | yes | 2 |
| Instructions | yes | 2 |
| Constraints | no | 2 |
| Examples | no | 2 |
| Tools | no | 2 |

## Templates

Copy a template from [`docs/templates/`](../docs/templates/) when adding a new asset.
