# Reference

Behavioural semantics — ownership, orphans, drift, conflicts, idempotence —
are described once, in [README — How it behaves](../README.md#how-it-behaves).
This page is the data and interface reference: what the keys and flags *mean*.
The values are not stated here: [`spec.json`](../spec.json) is the declared
source for the name, the defaults and the per-category section schemas, and
this page refers to those keys instead of repeating them.

## Config

A single JSON object. Each top-level key is an asset **category**; its value
describes where that category's assets are emitted.

```json
{
  "rules": {
    "root": "./.opencode/rules",
    "imports": [
      {
        "name": "conventional-commits",
        "rename": "commits",
        "destination": "conventions",
        "injections": { "frontmatter": { "owner": "platform" } }
      }
    ],
    "injections": {
      "frontmatter": { "team": "platform" },
      "json": [
        {
          "file": "./opencode.json",
          "content": { "instructions": [".opencode/rules/**/*.md"] }
        }
      ]
    }
  },
  "skills": { "root": "./.opencode/skills", "imports": ["tdd"] },
  "agents": { "root": "./.opencode/agents", "imports": ["code-reviewer"] }
}
```

Category names are not a fixed set — see [Format inference](#format-inference).
Unknown keys are rejected anywhere in the document, and every error names the
JSON path it is about: `rules.imports[1].rename: …`.

### Category

| Key | Type | Meaning |
|---|---|---|
| `root` | string | output directory, project-relative; defaults to `defaults.rootPrefix` + the category |
| `imports` | array | assets to emit; defaults to none |
| `injections` | object | category-wide transformations; defaults to none |

### Import

Either a string (shorthand for `{ "name": "<string>" }`) or an object:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | registry entry to import |
| `rename` | string | `name` | output basename: without `.md` for file assets, the directory name for directory assets |
| `destination` | string | `""` | subdirectory of `root`; `..` is rejected |
| `injections` | object | `{}` | per-import transformations |

### Format inference

For each import the engine looks for `<category>/<name>.md` and then for
`<category>/<name>/` in `--assets`:

- `<name>.md` found — a **file asset**, emitted to
  `<root>/<destination>/<rename>.md`.
- `<name>/` found — a **directory asset** (a skill), emitted to
  `<root>/<destination>/<rename>/`. Every file in the source is copied;
  `defaults.skillEntry` is the entry file and is the one that receives
  frontmatter, the rest are artifacts and are copied verbatim.
- neither — the run fails naming both paths it tried.

A source that is both is an error, so a typo fails loudly instead of
silently picking one shape. The directory is a skill directory; its entry
file (`defaults.skillEntry`) is the one stated format convention of the
registry.

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
owner: platform
team: platform
---

## Summary

Commit messages follow the Conventional Commits specification.
...
```

from `category = { "team": "platform" }` and
`import = { "owner": "platform" }`. Applied to file assets and to a directory
asset's entry file; other files in a directory asset are copied verbatim.

An empty map emits no block at all — but it still strips whatever the source
carried, because frontmatter is the consumer's to decide.

### `json`

Ensures static content is present in a JSON file you own (typically the
agent's own config). Entries are
`{ "file": <project-relative path>, "content": <object> }`. Chunks from every
category that target the same file are combined first, then merged into the
target:

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
(invalid JSON fails the run). Targets are **not** recorded in the manifest
and are never deleted; `check` reports `INJECT` whenever merging would still
change the file. A target that collides with a rendered asset path fails the
run before anything is written.

## Manifest

The manifest lives at `defaults.manifest` (path configurable). Written by
`sync`, read by `check`. One entry per **imported asset** — a file asset's
emitted file, a directory asset's emitted directory — with the SHA-256 of
every file rendered inside it:

```json
[
  {
    "path": ".opencode/rules/conventional-commits.md",
    "files": [
      { "path": ".opencode/rules/conventional-commits.md", "hash": "sha256:…" }
    ]
  },
  {
    "path": ".opencode/skills/tdd",
    "files": [
      { "path": ".opencode/skills/tdd/SKILL.md", "hash": "sha256:…" },
      { "path": ".opencode/skills/tdd/checklist.md", "hash": "sha256:…" }
    ]
  }
]
```

The hash is of the **rendered** bytes, not of the source, so an injection
change shows up too. Rows and files are sorted by path, so the manifest's
bytes do not depend on the order imports happened to be written in.

The manifest is what makes a path ours — those paths may be overwritten or
deleted, and anything inside a listed directory is ours to keep exactly as
rendered. Anything else under the category `root` is untouched.

### What the hash decides

Comparing the disk against the manifest separates two very different
situations — see
[README — Drift and conflicts](../README.md#drift-and-conflicts):

| disk vs render | manifest hash vs disk | verdict |
|---|---|---|
| equal | anything | clean |
| differs | equal | `UPDATE` — the registry moved on |
| differs | differs (or absent) | `CONFLICT` — somebody edited it |

## CLI

```
agent-assets sync
agent-assets check
agent-assets status
agent-assets validate
agent-assets render --out DIR
agent-assets list
agent-assets show <category>/<name> [--rendered]
```

| Flag | Meaning |
|---|---|
| `--config FILE` | config path (default `defaults.config`) |
| `--assets DIR` | asset registry root (default: the registry compiled into the binary) |
| `--project DIR` | project root to reconcile (default `defaults.project`) |
| `--manifest PATH` | manifest, relative to `--project` (default `defaults.manifest`) |
| `--out DIR` | destination tree (`render` only) |
| `--force` | replace files the manifest cannot vouch for (`sync` only) |
| `--color WHEN` | `auto`, `always` or `never`; `NO_COLOR` is honoured |

`--project` may be omitted: the engine walks up from the current directory
looking for the declared config basename, the way `git` finds its repository
root.

`list` and `show` read the registry alone — no project, no config — so they
work wherever the binary lands. `show` prints the source; `--rendered`
prints every file `sync` would write for that asset, and is the only form
that needs a config. An asset imported twice is two answers: each file is
printed behind a `==> <output path> <==` header, so the preview and the tree
cannot disagree. An asset nobody imports errors, because `sync` would write
nothing. Output goes through `bat`, then `less`, then `cat` on a terminal,
and straight out otherwise so `show` still composes with a pipe.

`status` is `check` without the filter and always exits 0.

`validate` reports every problem with the config and the registry at once,
rather than the first one each command would hit: an import that does not
resolve, two imports fighting over one output path, an injection aimed at a
file an asset owns, and any asset whose `##` sections depart from the schema
its category declares under `headings` in `spec.json`. It prints one problem
per line, exits 1 when it found any, and modifies nothing. It needs a project,
because the config half needs one.

The config half is also a pre-flight: `sync`, `check`, `status`, `render` and
`show --rendered` refuse with every reason at once before writing anything.
The registry half is never a gate — a malformed asset still syncs, because it
is a quality problem in the registry rather than in this project's config.

Output tokens: `check` prints `MISSING`, `DRIFT`, `EXTRA`, `ORPHAN`,
`INJECT`, `CONFLICT`; `status` prints those plus `CLEAN`; `sync` prints
`WRITE`, `UPDATE`, `REMOVE`, `MERGE`, and `CONFLICT` for the ones it refused
to touch. `check` and `sync` are silent when there is nothing to say.
`validate` prints plain problem lines rather than status tokens. `render`
writes the expected tree and touches nothing else — no manifest, no JSON
injections, no project.

Status words are coloured on a terminal; paths stay plain so the output is
greppable, and the `(from <category>/<name>)` attribution is dimmed.

## Nix module

```nix
{
  imports = [ agent-assets.flakeModules.default ];

  agentAssets = {
    enable = true;
    config = { rules = { root = "./.opencode/rules"; imports = [ "conventional-commits" ]; }; };
  };
}
```

Options live at the **module root** (sibling of `perSystem`), not inside
`perSystem`:

| Option | Type | Meaning |
|---|---|---|
| `enable` | bool | emit the apps; when false nothing is emitted (default `false`) |
| `config` | attrs or path | attrs are serialized to JSON; a path must point at a JSON file (default `{}`) |
| `manifest` | str | passed through as `--manifest` (default `defaults.manifest`) |

With `enable = true` and a non-empty `config`, the flake gains `apps`:

```bash
nix run .#sync
nix run .#check
nix run .#status
```

All three run the package **this flake builds**, with `--project` set to
`$PROJECT_ROOT`, falling back to `$PWD`. Extra arguments after `--` are
forwarded to the engine. Because the program is this flake's own binary, and
the registry is compiled into it, emitted bytes depend on your `flake.lock`
and nothing else — the consumer's nixpkgs cannot drift them.

The flake also exposes `packages.default` (the engine), and
`apps.{sync,check,status,render,list,show}` for running it outside a module.
