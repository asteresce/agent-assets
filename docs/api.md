# Reference

Behavioural semantics (ownership, orphans, drift, idempotence) are described
once, in [README — How it behaves](../README.md#how-it-behaves). This page is
the data and interface reference.

## Config

A single JSON object. Each top-level key is an asset **category**; its value
describes where that category's assets are emitted.

```json
{
  "rules": {
    "root": "./.opencode/rules",
    "imports": [
      "code-style",
      {
        "name": "security",
        "rename": "secrets",
        "destination": "base",
        "injections": { "frontmatter": { "owner": "sec" } }
      }
    ],
    "injections": {
      "frontmatter": { "team": "platform" },
      "json": [
        {
          "file": "./opencode.json",
          "content": { "instructions": [ ".opencode/rules/**/*.md" ] }
        }
      ]
    }
  }
}
```

Category names are not a fixed set — see [Format inference](#format-inference).

### Category

| Key | Type | Default | Meaning |
|---|---|---|---|
| `root` | string | `./.agent-assets/<category>` | output directory, project-relative |
| `imports` | array | `[]` | assets to emit |
| `injections` | object | `{}` | category-wide transformations |

### Import

Either a string (shorthand for `{ "name": "<string>" }`) or an object:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | registry entry to import |
| `rename` | string | `name` | output basename: without `.md` for file assets, the directory name for directory assets |
| `destination` | string | `""` | subdirectory of `root`; `..` is rejected |
| `injections` | object | `{}` | per-import transformations |

### Format inference

For each import the engine looks for `--src/<category>/<name>.md` and then for
`--src/<category>/<name>/`:

- `<name>.md` found — a **file asset**, emitted to
  `<root>/<destination>/<rename>.md`.
- `<name>/` found — a **directory asset** (a skill), emitted to
  `<root>/<destination>/<rename>/`. Every file in the source is copied;
  `SKILL.md` is the entry file and is the one that receives frontmatter, the
  rest are artifacts and are copied verbatim.
- neither — the run fails naming both paths it tried.

A source that is neither is an error, so typos in `name` fail loudly instead
of emitting nothing. The directory is a skill directory; `SKILL.md` is the
one stated format convention of the registry.

## Injections

```json
"injections": {
  "frontmatter": { "key": "value" },
  "json": [ { "file": "<path>", "content": { } } ]
}
```

Both are optional at category and at import level.

### `frontmatter`

Adds a YAML frontmatter block to emitted Markdown. It *replaces* any
frontmatter the source carries — sources are provider-neutral and should not
carry any.

The category map and the per-import map are merged shallowly with the
**per-import key winning**, then rendered as YAML with sorted keys (so the
emitted bytes do not depend on key order in the config):

```markdown
---
owner: sec
team: platform
---

## Summary
...
```

from `category = { "team": "platform", "owner": "default" }` and
`import = { "owner": "sec" }`. Applied to file assets and to a directory
asset's `SKILL.md`; other
files in a directory asset are copied verbatim.

### `json`

Ensures static content is present in a JSON file you own (typically the agent's
own config). Entries are `{ "file": <project-relative path>, "content": <object> }`.
Chunks from every category that target the same file are combined first, then
merged into the target:

- **objects** — key-wise merge; keys only the target has are kept.
- **arrays** — union: the target's items and order are kept, chunk items not
  already present are appended. Re-running is therefore idempotent.
- **scalars** — the chunk value is enforced.

Merging `{ "instructions": [".opencode/rules/**/*.md"], "nested": {"a": 1} }`
into `{ "user_key": true, "nested": {"b": 2} }` yields:

```json
{
  "user_key": true,
  "instructions": [ ".opencode/rules/**/*.md" ],
  "nested": { "a": 1, "b": 2 }
}
```

The target is created if missing and must be a JSON object if present
(invalid JSON fails the run). Targets are **not** recorded in the manifest and
are never deleted; `check` reports `INJECT` whenever merging would still
change the file.

## Manifest

`./agent-assets.lock` (path configurable). Written by `sync`, read by `check`.
**One entry per imported asset** — a file asset's emitted file, a directory
asset's emitted directory:

```json
[
  { "category": "rules",  "name": "security",  "path": ".opencode/rules/security.md" },
  { "category": "skills", "name": "migration", "path": ".opencode/skills/migration" },
  { "category": "agents", "name": "build",     "path": ".opencode/agents/build.md" }
]
```

No hashes: drift is computed by re-rendering and comparing bytes. The manifest
is what makes an asset ours — those paths may be overwritten or deleted, and
anything inside a listed directory is ours to keep exactly as rendered.
Anything else under the category `root` is untouched.

## CLI

```
agent-assets render --out DIR
agent-assets sync
agent-assets check
```

| Flag | Default | Meaning |
|---|---|---|
| `--config FILE` | `./agent-assets.json` | config path |
| `--src DIR` | the checkout containing the script | registry root |
| `--project DIR` | `.` | project root to reconcile |
| `--manifest PATH` | `./agent-assets.lock` | manifest, relative to `--project` |
| `--out DIR` | — | destination tree (`render` only) |

Output tokens: `check` prints `MISSING`, `DRIFT`, `EXTRA`, `ORPHAN`, `INJECT`
(`EXTRA` is a file inside a managed asset directory that was not rendered);
`sync` prints `WRITE`, `UPDATE`, `REMOVE`, `MERGE`. Both are silent when there
is nothing to say. `render` writes the expected tree and touches nothing else.

## Nix module

```nix
{
  imports = [ agent-assets.flakeModules.default ];

  agentAssets = {
    enable = true;
    config = { rules = { root = "./.opencode/rules"; imports = [ "security" ]; }; };
    manifest = "./agent-assets.lock";
  };
}
```

Options live at the **module root** (sibling of `perSystem`), not inside
`perSystem`:

| Option | Type | Default | Meaning |
|---|---|---|---|
| `enable` | bool | `false` | emit the apps; when false nothing is emitted |
| `config` | attrs or path | `{}` | attrs are serialized to JSON; a path must point at a JSON file |
| `manifest` | str | `"./agent-assets.lock"` | passed through as `--manifest` |

With `enable = true` and a non-empty `config`, the flake gains `apps`:

```bash
nix run .#sync
nix run .#check
```

Both run with `--src` pinned to this flake's store path (your `flake.lock`
revision) and `--project` set to `$PROJECT_ROOT`, falling back to `$PWD`. Extra
arguments after `--` are forwarded to the engine.
