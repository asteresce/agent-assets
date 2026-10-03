# API reference

## Flake outputs

The flake exposes:

| Output | Description |
|---|---|
| `flakeModules.default` | The flake-parts module. |
| `lib.mkAgentAssets` | Pure helper: given `{ pkgs, config }`, returns `{ package, path }`. |
| `lib.assets.<category>.<name>` | Registry of shared asset sources. Rules and agents are files; skills are directories. |
| `lib.schemas.headings` | Section heading schemas for rules, skills, and agents. |
| `lib.schemas.formats` | Asset format per category (`file` or `directory`). |
| `apps.<system>.sync` | Apply the emitted tree to the project. |
| `apps.<system>.check` | Report drift and validate section headings for imported assets. |

## flake-parts module options

Imported via:

```nix
imports = [ agent-assets.flakeModules.default ];
```

Options live under `perSystem.agentAssets`.

### `agentAssets.enable`

- Type: `bool`
- Default: `false`

Enable the module. Apps (`sync`, `check`) are only emitted when this is
`true` **and** `config` is non-empty.

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

### `agentAssets.manifest`

- Type: `string`
- Default: `"./agent-assets.lock"`

Path to the lock file, relative to the project root. Sync writes this file
and check reads it. The emission's internal `agent-assets.lock` (which
records the emitted tree) is separate and not configurable.

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
agent-assets.lib.mkAgentAssets pkgs {
  rules = { ... };
  skills = { ... };
  agents = { ... };
}
```

Returns `{ package, path }`. `pkgs` must be the nixpkgs the consumer wants
to use (no hidden `<nixpkgs>` import). The `manifest` option is not part of
this helper; if you also want sync/check, wrap a derivation yourself.

## Flake apps

Apps are only emitted when `agentAssets.enable = true` and `config` is
non-empty. Each app uses the `pkgs` of the system it was instantiated for.

### `nix run .#sync`

1. Applies JSON injections: deep-merges our keys into the target files.
2. Reconciles owned files (rules, agents, skills) using the lock:
   - Skips files whose on-disk hash matches the lock.
   - Overwrites files whose hash differs.
   - Recopies skill directories whose dir hash differs.
   - Deletes files in the previous lock but absent from the current emission.
3. Refreshes the lock.

Files under managed roots that are not in any lock are preserved (custom local files). If the lock is missing or unreadable, sync writes a fresh one and skips orphan deletion for that run.

### `nix run .#check`

Reports:

- Drift: owned file or skill dir whose hash differs from the lock.
- JSON injection drift: target files missing the chunk's keys.
- Orphans: paths under managed roots in the previous lock but absent from the current one.

Exits non-zero on any report. Custom local files (not in any lock) are never flagged.