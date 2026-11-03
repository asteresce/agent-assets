# Example: wiring assets into OpenCode

OpenCode reads project rules from `.opencode/`, agent prompts from
`.opencode/agents/`, and its own settings from `opencode.json`. This example
places shared assets into exactly that layout and teaches `opencode.json` to
load the rules.

## The flake

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

      agentAssets = {
        enable = true;
        config = {
          rules = {
            root = "./.opencode/rules";
            imports = [
              {
                name = "conventional-commits";
                rename = "commits";
                destination = "conventions";
                injections.frontmatter.owner = "platform";
              }
            ];
            injections = {
              frontmatter = { team = "platform"; };
              json = [{
                file = "./opencode.json";
                content.instructions = [ ".opencode/rules/**/*.md" ];
              }];
            };
          };

          skills = {
            root = "./.opencode/skills";
            imports = [ "tdd" ];
          };

          agents = {
            root = "./.opencode/agents";
            imports = [ "code-reviewer" ];
          };
        };
      };
    };
}
```

## What `nix run .#sync` produces

```
.opencode/
  rules/
    conventions/commits.md
  skills/
    tdd/
      SKILL.md
      checklist.md
  agents/
    code-reviewer.md
opencode.json      # merged, not overwritten
agent-assets.lock  # one row per imported asset, with hashes
```

`conventions/commits.md` shows the frontmatter layering — the per-import
`owner` is joined by the category `team`:

```markdown
---
owner: platform
team: platform
---

## Summary

Commit messages follow the Conventional Commits specification.
...
```

`tdd/checklist.md` is a skill artifact: copied verbatim, no frontmatter. Only
`tdd/SKILL.md`, the entry file, receives it. `opencode.json` is merged, so
existing keys survive:

```json
{
  "user_key": true,
  "instructions": [ ".opencode/rules/**/*.md" ]
}
```

That `instructions` glob is what makes OpenCode load the rules; without the
JSON injection you would add it by hand.

## Your own assets live beside ours

The category root is a shared namespace. `nix run .#sync` never touches
anything there that no import claims:

```
.opencode/skills/
  tdd/                # ours — one row in agent-assets.lock
  my-own-skill/       # yours — never touched
```

`my-own-skill/` survives syncs and survives dropping the `tdd` import. Inside
`tdd/`, though, the asset is immutable after injection: a stray file added
there is reported as `EXTRA` by `check` and removed by `sync`.

## From there

```bash
nix run .#check   # clean now; exits 1 if anything drifts
```

`opencode.json` is deliberately not owned — the manifest never lists it, so
`check` cannot delete it and edits to unrelated keys are never flagged. Only
the absence of the injected keys is reported, as `INJECT`.

To drop an asset later, remove it from `imports` and run `nix run .#sync`: its
path is deleted as a whole (a file, or a directory asset's entire directory).

## If somebody edits a managed file

Emitted files are outputs, so a hand edit is not merged with the render. The
manifest remembers what the last sync intended to find on disk, and that is
what separates the two cases:

- the file matches what we last wrote and no longer matches the render → the
  registry moved on; `sync` rewrites it.
- the file matches nothing we wrote → somebody edited it. `sync` reports
  `CONFLICT` and leaves it alone.

```bash
nix run .#check          # CONFLICT .opencode/rules/conventions/commits.md (from rules/conventional-commits)
nix run .#sync           # reports CONFLICT and exits 1; the edit survives
nix run .#sync -- --force   # now it is an UPDATE, and the render wins
```

Decide deliberately: either keep the edit by dropping the import (the file
then becomes an orphan and is deleted — so move it somewhere unmanaged
first), or let the render win with `--force`.
