# Shared Agent Assets

Provider-neutral rules, skills and agent prompts, plus a small engine that
puts them into a project tree: declarative, idempotent, and checkable in CI.

## What you get

- `agent-assets` — one self-contained binary with `sync`, `check`, `render`,
  `status`, `list` and `show`. The registry of shared assets is compiled
  into it, so it runs anywhere with nothing installed beside it.
- `assets/` — that registry's source, one directory per category (`rules/`,
  `skills/`, `agents/`).
- A flake-parts module wiring `nix run .#sync`, `nix run .#check` and
  `nix run .#status`.

The engine targets Unix (Linux and macOS): its path-safety rules are Unix
symlink semantics.

## Quick start

Get the binary. From the
[releases](https://github.com/asteresce/agent-assets/releases), pick
`agent-assets-<version>-<target>.tar.gz` for your platform, check it against
`SHA256SUMS`, and put `agent-assets` on your `PATH`:

```bash
version=0.1.0 target=x86_64-unknown-linux-musl
base=https://github.com/asteresce/agent-assets/releases/download/v$version
curl -fsSLO "$base/agent-assets-$version-$target.tar.gz"
curl -fsSLO "$base/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar xzf "agent-assets-$version-$target.tar.gz"
```

Or install it from source, which is one command and needs nothing else —
the registry is in the binary:

```bash
cargo install agent-assets
```

Then write a config that says which assets you import and where they go:

```bash
cat > agent-assets.json <<'EOF'
{
  "rules":  { "root": "./.opencode/rules",  "imports": ["conventional-commits"] },
  "skills": { "root": "./.opencode/skills", "imports": ["tdd"] },
  "agents": { "root": "./.opencode/agents", "imports": ["code-reviewer"] }
}
EOF
```

And run the engine:

```bash
agent-assets list     # what the registry holds
agent-assets sync     # write the assets into the project
agent-assets check    # exit 1 if anything drifted (use it in CI)
```

`--assets` swaps in a different registry — your own, or a pinned checkout —
without changing the binary. Everything else defaults from
[`spec.json`](spec.json); see [the reference](docs/api.md).

## Quick start (Nix)

For a Nix consumer the flake module wires the same engine in, pinned by your
`flake.lock`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    agent-assets.url = "github:asteresce/agent-assets";
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
            imports = [ "conventional-commits" ];
            injections.json = [{
              file = "./opencode.json";
              content.instructions = [ ".opencode/rules/**/*.md" ];
            }];
          };
          skills = { root = "./.opencode/skills"; imports = [ "tdd" ]; };
          agents = { root = "./.opencode/agents"; imports = [ "code-reviewer" ]; };
        };
      };
    };
}
```

```bash
nix run .#sync     # write the assets into the project
nix run .#check    # exit 1 if anything drifted (use it in CI)
nix run .#status   # every path, matching or not; always exits 0
```

## How it behaves

`sync` renders the configured assets from `--assets` and reconciles them with
the project. `check` performs the same comparison and only reports. This is
the canonical description of both.

### Ownership

`sync` records each imported asset in the manifest (`defaults.manifest`): one
row per import, with the SHA-256 of every file it rendered. A file asset is
its emitted file; a directory asset is its emitted directory. Only those
paths are ever overwritten or deleted. A category's `README.md` is its
catalog index, not an asset: it cannot be imported, so it is never owned.

The hash is what lets the engine tell a **source change** from a **hand edit**.
It is the memory of what the last run intended to find on disk: comparing the
disk against it is the whole basis for `UPDATE` versus `CONFLICT` below.

### Shared roots

The category `root` is a shared namespace. Anything under it that no import
claims — a custom rule, a whole custom skill directory — is yours and is
never touched, including after neighbouring imports change.

### Immutable assets

After injection, an asset's contents are exactly the render. A file inside a
managed asset directory that we did not render is removed and reported as
`EXTRA`; the category root stays shared.

### Orphans

An asset in the manifest that the config no longer produces is deleted whole
(file or directory), and directories left empty by that are pruned. If the
manifest is missing or unreadable, `sync` warns and skips orphan removal
instead of guessing.

### Drift and conflicts

A rendered file that differs on disk is either a source change or a hand
edit, and the manifest hash decides which:

- the disk matches what the last run intended → the registry moved on;
  `sync` rewrites it (`UPDATE`), `check` reports `DRIFT`;
- the disk matches nothing we wrote → somebody edited it. `sync` leaves it
  alone and reports `CONFLICT`; `check` reports `CONFLICT`. `--force`
  overwrites it, as an `UPDATE`.

With no manifest there is no memory of what we wrote, so any difference is a
`CONFLICT`. `--force` is the way through.

Managed files are outputs, not shared state: there is no three-way merge.

### Wrong kinds of thing

A path holding the wrong kind of thing — a symlink, a directory where a file
belongs — is `DRIFT`, **whether or not we own it**: `check` has nothing to
refuse and everything to report. Only `sync` gets to decide whether to
replace it, and it replaces it only when the manifest says the path is ours.
Otherwise the whole run refuses before writing anything.

`sync` also refuses to write through a symlinked directory, or a file where a
directory belongs, anywhere between the project root and the leaf. Writes are
atomic: a reader sees either the old content or the new.

When a directory asset's own path is the wrong kind, that asset's `DRIFT` is
reported once. The files behind it get no separate problem — the asset's own
line already says what is wrong.

### Frontmatter

`injections.frontmatter` *replaces* any frontmatter the source carries. The
category map and the per-import map are combined with the per-import key
winning, then emitted as YAML with sorted keys.

### JSON injections

`injections.json` *ensures presence* in a file you own: object keys merge,
arrays union (your items are kept), scalars are enforced. Targets are never
listed in the manifest and never deleted; `check` reports `INJECT` whenever
merging would still change the file.

### What gets printed

`check` prints nothing and exits 0 when clean. `sync` is silent when nothing
changed. Otherwise each line is `STATUS path`, with `(from <category>/<name>)`
when a particular import produced the file.

| Word | Command | Meaning |
|---|---|---|
| `WRITE` | `sync` | a file that was not there is now written |
| `UPDATE` | `sync` | a file we wrote before is rewritten |
| `REMOVE` | `sync` | an owned path is deleted |
| `MERGE` | `sync` | a JSON injection target is merged |
| `MISSING` | `check`, `status` | a rendered file is not on disk |
| `DRIFT` | `check`, `status` | the file on disk differs from the render, or is the wrong kind of thing |
| `CONFLICT` | both | the file differs and the manifest cannot vouch for it; `sync` skips it unless `--force` |
| `EXTRA` | `check`, `status` | a file inside a managed asset directory that we did not render |
| `ORPHAN` | `check`, `status` | an owned path the config no longer produces |
| `INJECT` | `check`, `status` | a JSON injection target is not yet merged |
| `CLEAN` | `status` | the file matches the render and the manifest vouches for it |

`check` never modifies the project, by construction: it plans and reports,
and only `sync` writes. `check` exits 1 when it reported anything. `sync`
exits 1 only on the `CONFLICT` lines it left alone; everything else it
prints is work it did.

### `status`

`status` is `check` without the filter: every path the run knows about,
`CLEAN` included, then a tally. It always exits 0 — the state of the tree is
information, not an error, the way `git status` is.

```
$ agent-assets status
CLEAN .opencode/agents/build.md (from agents/build)
CONFLICT .opencode/rules/security.md (from rules/security)
...
4 imports · 3 clean · 1 conflict · 2 custom files
```

An import counts once, as the worst of its files, so "clean" and "conflict"
add up to the import count. The custom files are the ones under a category
root that nobody owns — yours, and never touched.

### `list` and `show`

`list` names every importable asset in the registry with its kind, size and
summary, and closes with totals per category. `show <category>/<name>`
prints one asset — the source by default, or `--rendered` for what `sync`
would write. Both read the registry alone and need no project, so they work
wherever the binary lands.

## The registry

| Category | Source | Emitted |
|---|---|---|
| `rules/` | `assets/rules/<name>.md` | one Markdown file |
| `skills/` | `assets/skills/<name>/` (entry file + artifacts) | a directory |
| `agents/` | `assets/agents/<name>.md` | one Markdown file |

Sources are provider-neutral: no frontmatter, no agent-specific framing. All
frontmatter comes from the consumer's config. In a directory asset, one file
is the entry file (`defaults.skillEntry`) and receives frontmatter; every
other file is an artifact and is copied verbatim. Each category fixes the
sections its assets carry — the schemas live in
[`spec.json`](spec.json) under `headings`.

This registry is compiled into the binary, so `list` and `show` work with
nothing on disk. `--assets` reads the same layout from a directory instead,
which is how a consumer brings their own.

Adding an asset is a Markdown file plus a line in the category `README.md`.
Adding a category is a new directory in the registry plus the matching config
key and section schema: the engine infers file-vs-directory from whether the
source is `<name>.md` or `<name>/`, so nothing else has to change.

## Pinning and updates

The engine never contacts the network and has no notion of upstream HEAD.
The registry is compiled into the binary, so what lands in your tree is a
function of the binary and nothing else.

As a Nix consumer, that binary is the `agent-assets` flake input **as pinned
by your `flake.lock`**. Upstream releases cannot change your tree, and
`check` stays green across upstream evolution. Standalone users get the same
property by pinning the release they install — a version number, or a
checksum-verified download.

Updating is therefore a deliberate, reviewable act: move the pin
(`nix flake update agent-assets`, or install a newer release), run `sync`,
commit the diff. Emitted files carry no provenance markers — the pin lives
in `flake.lock` or in the version you installed, so they stay plain Markdown.

`--assets` swaps in a different registry — your own, or a pinned checkout —
without changing the binary.

## Versions

`Cargo.toml` holds the version, and a release is that number carried through
three steps:

1. set `version` in `Cargo.toml`;
2. commit;
3. tag that commit `v<version>`.

A tag that is not the version in `Cargo.toml` fails the release build rather
than shipping a mismatched binary. So the rule is short: **`Cargo.toml` →
commit → tag `vX.Y.Z`**.

## Documentation

- [`docs/api.md`](docs/api.md) — config schema, injections, manifest, CLI and
  the Nix module.
- [`docs/examples/opencode.md`](docs/examples/opencode.md) — end-to-end
  example wiring assets into OpenCode.
- [`assets/rules/README.md`](assets/rules/README.md),
  [`assets/skills/README.md`](assets/skills/README.md),
  [`assets/agents/README.md`](assets/agents/README.md) — what is in the
  registry.
- [`AGENTS.md`](AGENTS.md) — contributing to this repository.

## Project layout

```
build.rs           # compiles assets/ into the binary
src/               # the engine: config, registry, render, manifest, plan, apply
tests/             # the behaviour suite, and registry conformance
spec.json          # declared defaults and section schemas
assets/            # the shared registry: rules/, skills/, agents/
docs/              # reference and examples
flake.nix          # packages, apps, checks, flakeModules.default
flake-module.nix   # the consumer-facing module
.github/workflows/ # ci, and the release pipeline
opencode.json      # tooling config for maintaining this repo (not public API)
```
