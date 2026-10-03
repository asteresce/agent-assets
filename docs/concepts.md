# Core concepts

This module reads a declarative configuration and emits a project-tree overlay of adapted Markdown assets. The consumer applies that overlay and wires it into their agent.

## Asset category

An asset category groups similar assets. This project provides three:

- `rules` — single Markdown files.
- `skills` — directories containing a `SKILLS.md` file and optional artifacts.
- `agents` — single Markdown files.

Each category has its own `root`, `imports`, and `injections`. The format for each category is defined in `lib.schemas.formats`: rules and agents are single files; skills are directories. Adding a fourth category only requires editing `lib/schemas/headings.nix` and `lib/schemas/formats.nix`.

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

`root` is the base output directory for a category. It defaults to `./.agent-assets/<category>` — clients can override it, for example to `./.opencode/rules`.

## Injection

An injection transforms an asset before emission. Injections are configured with pure data, not functions.

Supported injections:

- [`frontmatter`](injections/frontmatter.md)
- [`json`](injections/json.md)

## Lock

Every sync writes a JSON lock file at `./agent-assets.lock` (the path is
configurable via the flake-parts module's `agentAssets.manifest` option).

The lock records, for each owned file or directory:

- `category` and `name` — registry reference.
- `path` — location relative to the project.
- `hash` — `sha256-` content hash of the emitted bytes. For directory-backed
  imports, a Merkle-style hash over the files we manage.

The hash captures the entire emitted state, including any frontmatter
injection. Frontmatter, JSON injections, and other transformations are
*implicit* in the hash — they are not recorded separately.

## Sync behaviour

The sync command is the canonical authority on what sync does. The
summary below appears in `README.md` and the OpenCode example; both
link back here.

Sync:

1. Applies JSON injections: deep-merges our keys into the target files.
2. Reconciles owned files (rules, agents, skills) using the lock:
   - Skips owned files whose on-disk hash matches the lock (idempotent).
   - Overwrites owned files whose hash differs.
   - Recopies skill directories whose dir hash differs.
   - Deletes files present in the previous lock but absent from the current one
     (stale orphans).
3. Refreshes the lock.

Files under managed roots that are not in any lock are preserved (custom
local files). If the lock is missing or unreadable on sync, a fresh one is
written and orphan deletion is skipped for that run.

JSON injections are applied as a side-effect during sync: our chunk's keys are
deep-merged (list concatenation) into the target file. JSON files are *not*
tracked in the lock, since we do not claim ownership of them.

## Check behaviour

The check command is the canonical authority on what check reports. Both
the README and the OpenCode example link back here.

Check reports:

- **Drift** — owned file or skill dir whose on-disk hash differs from
  the lock.
- **JSON injection drift** — target files missing the chunk's keys.
- **Orphans** — paths under managed roots in the previous lock but
  absent from the current emission.
- **Heading errors** — section-heading violations on imported assets
  (when a schema blob is supplied by the flake; the `nix run .#check`
  app does this automatically).

Exits non-zero on any report. Custom local files (not in any lock) are
never flagged.

## Emission target

The emission target is a project-tree overlay. It contains the imported assets
arranged under their category roots. JSON injection files live wherever the
config places them and are not part of the overlay.