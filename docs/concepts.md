# Core concepts

This module reads a declarative configuration and emits a project-tree overlay of adapted Markdown assets. The consumer applies that overlay and wires it into their agent.

## Asset category

An asset category groups similar assets. This project provides three:

- `rules` — single Markdown files.
- `skills` — directories containing a `SKILLS.md` file and optional artifacts.
- `agents` — single Markdown files.

Each category has its own `root`, `imports`, and `injections`. The format for each category is defined in `lib.schemas.formats`: rules and agents are single files; skills are directories.

## Import declaration

A consumer declares an asset to import with a data attribute set:

```nix
{
  name = "conventional-commits";
  rename = "commit-convention";
  destination = "conventions";
  injections = {
    frontmatter = {
      path = "./frontend";
    };
  };
}
```

- `name` — shared asset to import from the registry.
- `rename` — output file basename without extension; defaults to `name`.
- `destination` — directory relative to the category `root`; defaults to the `root`.
- `injections` — optional per-import injections.

`imports` also accepts a plain string as shorthand for `{ name = "..."; }`.

## Root

`root` is the base output directory for a category. It defaults to `./.agent-assets/<category>`. Clients can override it, for example to `./.opencode/rules`.

## Injection

An injection transforms an asset before emission. Injections are configured with pure data, not functions.

Supported injections:

- [`frontmatter`](injections/frontmatter.md)
- [`json`](injections/json.md)

## Provenance marker

Every imported asset receives a machine-readable HTML comment after its frontmatter:

```markdown
<!-- agent-assets: rules/security -->
```

This lets sync/check commands distinguish imported assets from custom local files.

## Emission target

The emission target is a project-tree overlay. It contains the imported assets arranged under their category roots, plus any JSON files created by JSON injections.
