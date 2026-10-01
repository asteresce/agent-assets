# API reference

## Flake outputs

The flake exposes:

| Output | Description |
|---|---|
| `flakeModules.default` | The flake-parts module. |
| `lib.mkAgentAssets` | Pure function for building assets from a config attrset. |
| `lib.assets.<category>.<name>` | Registry of shared asset sources. Rules and agents are files; skills are directories. |
| `lib.schemas.headings` | Section heading schemas for rules, skills, and agents. |
| `lib.schemas.formats` | Asset format per category (`file` or `directory`). |
| `apps.agentAssets.sync` | Apply the emitted tree to the project. |
| `apps.agentAssets.check` | Report drift and validate section headings for imported assets. |

## flake-parts module options

Imported via:

```nix
imports = [ agent-assets.flakeModules.default ];
```

Options live under `perSystem.agentAssets`.

### `agentAssets.enable`

- Type: `bool`
- Default: `false`

Enable the module for this system.

### `agentAssets.config`

- Type: `attrs or path`
- Default: `{}`

The single source of truth for the client's configuration.

When a path, it is imported and must evaluate to the same attribute set.

Top-level schema:

```nix
{
  rules = { ... };
  skills = { ... };
  agents = { ... };
}
```

Each category uses the schema below.

### `agentAssets.output`

Read-only attribute set describing the generated output. Exposed as `config.agentAssets.output`.

| Attribute | Description |
|---|---|
| `package` | The derivation containing the project-tree overlay. |
| `path` | Path to the overlay root. |

## Category config schema

```nix
{
  root ? "./.agent-assets/<category>" :: string;
  imports = [ ... ];
  injections ? {} :: attrs;
}
```

Rules and agents are imported as single Markdown files. Skills are imported as directories containing a `SKILLS.md` file.

### `imports`

List of import declarations or string shorthands.

Import-declaration schema:

```nix
{
  name :: string;                # shared asset name
  rename ? name :: string;       # output basename without extension
  destination ? "" :: string;    # directory relative to root
  injections ? {} :: attrs;      # per-import injections
}
```

### `injections`

Attribute set of category-level injections applied to every asset in the category.

```nix
{
  frontmatter ? {} :: attrs;
  json ? [] :: list of { file :: string; content :: attrs; };
}
```

## Pure helper: `lib.mkAgentAssets`

For projects not using flake-parts:

```nix
agent-assets.lib.mkAgentAssets {
  rules = { ... };
  skills = { ... };
  agents = { ... };
}
```

Returns an attribute set with `package` and `path`.

## Flake apps

### `nix run .#agentAssets.sync`

Copies the emitted tree into the project. Only files with the provenance marker are overwritten; custom local files are left untouched.

### `nix run .#agentAssets.check`

Compares imported files in the project against the emitted tree and reports differences. Custom local files are ignored.
