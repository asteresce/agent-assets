# Shared Agent Assets

Provider-neutral rules, skills and agent prompts, plus a small engine that
puts them into a project tree: declarative, idempotent, and checkable in CI.

## What you get

- `rules/`, `skills/`, `agents/` — a registry of shared Markdown assets.
- `bin/agent-assets` — a bash engine with `render`, `sync` and `check`.
- A flake-parts module wiring `nix run .#sync` and `nix run .#check`.

The engine is a plain script: it runs outside Nix against any checkout too.

## Quick start (Nix)

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    agent-assets.url = "github:your-org/agent-assets";
  };

  outputs = inputs@{ flake-parts, agent-assets, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      imports = [ agent-assets.flakeModules.default ];

      # Declared at the module root, not inside `perSystem`.
      agentAssets = {
        enable = true;
        config = {
          rules = {
            root = "./.opencode/rules";
            imports = [ "code-style" "security" ];
            injections.json = [{
              file = "./opencode.json";
              content.instructions = [ ".opencode/rules/**/*.md" ];
            }];
          };
        };
      };
    };
}
```

```bash
nix run .#sync     # write the assets into the project
nix run .#check    # exit 1 if anything drifted (use it in CI)
```

## Quick start (no Nix)

```bash
cat > agent-assets.json <<'EOF'
{ "rules": { "root": "./.opencode/rules", "imports": ["security"] } }
EOF
path/to/agent-assets/bin/agent-assets sync
```

Requires `bash`, `jq` (>= 1.6), `yq` (mikefarah, v4), `awk` and standard
coreutils.
`--src` defaults to the checkout the script lives in; the remaining defaults
are in [the reference](docs/reference.generated.md).

## How it behaves

`sync` renders the configured assets from `--src` and reconciles them with the
project. `check` performs the same comparison and only reports. This is the
canonical description of both.

- **Ownership.** `sync` records each imported asset in the manifest
  (`defaults.manifest`; JSON, one row per import, paths only — no hashes): a
  file asset is its emitted file, a directory asset its emitted directory.
  Only those paths are ever overwritten or deleted.
- **Shared roots.** The category `root` is a shared namespace. Anything under
  it that no import claims — a custom rule, a whole custom skill directory —
  is yours and is never touched, including after neighbouring imports change.
- **Immutable assets.** After injection, an asset's contents are exactly the
  render. A file inside a managed asset directory that we did not render is
  removed and reported as `EXTRA`; the category root stays shared.
- **Orphans.** An asset in the manifest that the config no longer produces is
  deleted whole (file or directory), and directories left empty by that are
  pruned. If the manifest is missing or unreadable, sync skips orphan removal
  instead of guessing.
- **Drift.** A rendered file that differs on disk is rewritten by `sync` and
  reported as `DRIFT` by `check`. Managed files are outputs, not shared state —
  there is no three-way merge. A path holding the wrong kind of thing (a
  symlink, a directory where a file belongs) is `DRIFT` as well; `sync`
  replaces it only when the manifest says the path is ours, and otherwise
  refuses the run before writing anything.
- **Frontmatter** (`injections.frontmatter`) *replaces* any frontmatter the
  source carries. The category map and the per-import map are combined with
  the per-import key winning, then emitted as YAML with sorted keys.
- **JSON injections** (`injections.json`) *ensure presence* in a file you own:
  object keys merge, arrays union (your items are kept), scalars are
  enforced. Targets are never listed in the manifest and never deleted;
  `check` reports `INJECT` whenever merging would still change the file.
- `check` prints nothing and exits 0 when clean; it exits 1 on `MISSING`,
  `DRIFT`, `EXTRA`, `ORPHAN` or `INJECT`. `sync` prints one line per change
  (`WRITE`, `UPDATE`, `REMOVE`, `MERGE`) and is silent when nothing changed.

## The registry

| Category | Source | Emitted |
|---|---|---|
| `rules/` | `rules/<name>.md` | one Markdown file |
| `skills/` | `skills/<name>/` (entry file + artifacts) | a directory |
| `agents/` | `agents/<name>.md` | one Markdown file |

Sources are provider-neutral: no frontmatter, no agent-specific framing. All
frontmatter comes from the consumer's config. In a directory asset, one file is
the entry file (`defaults.skillEntry`) and receives frontmatter; every other
file is an artifact and is copied verbatim. Each category fixes the sections
its assets carry — see [the reference](docs/reference.generated.md).

Adding an asset is a Markdown file plus a line in the category `README.md`.
Adding a category is a new directory in the registry plus the matching config
key: the engine infers file-vs-directory from whether the source is
`<name>.md` or `<name>/`, so nothing else has to change.

## Pinning and updates

The engine only ever reads `--src`; it never contacts the network and has no
notion of upstream HEAD.

As a Nix consumer, `--src` is the store path of the `agent-assets` flake input
**as pinned by your `flake.lock`**. Upstream releases cannot change your tree,
and `check` stays green across upstream evolution. Standalone users get the
same property by pointing `--src` at a pinned checkout (git submodule, vendored
copy, or release tag).

Updating the assets is therefore a deliberate, reviewable act: move the pin
(`nix flake update agent-assets`), run `nix run .#sync`, commit the diff.
Emitted files carry no provenance markers — the pin lives in `flake.lock`, so
they stay plain Markdown.

## Documentation

- [`docs/api.md`](docs/api.md) — config schema, injections, manifest, CLI and
  the Nix module.
- [`docs/examples/opencode.md`](docs/examples/opencode.md) — end-to-end
  example wiring assets into OpenCode.
- [`rules/README.md`](rules/README.md), [`skills/README.md`](skills/README.md),
  [`agents/README.md`](agents/README.md) — asset catalogs.
- [`AGENTS.md`](AGENTS.md) — contributing to this repository.

## Project layout

```
bin/agent-assets     # the engine (bash + jq + yq)
rules/               # provider-neutral shared rules
skills/              # provider-neutral shared skills
agents/              # provider-neutral shared agent prompts
tests/               # integration tests for the engine
docs/                # reference and examples
flake.nix            # flakeModules.default + checks
flake-module.nix     # the consumer-facing module
opencode.json        # tooling config for maintaining this repo (not public API)
```
