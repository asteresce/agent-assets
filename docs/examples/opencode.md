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
              "code-style"
              {
                name = "security";
                rename = "secrets";
                destination = "base";
                injections.frontmatter.owner = "sec";
              }
            ];
            injections = {
              frontmatter = { team = "platform"; owner = "default"; };
              json = [{
                file = "./opencode.json";
                content.instructions = [ ".opencode/rules/**/*.md" ];
              }];
            };
          };

          skills = {
            root = "./.opencode/skills";
            imports = [ "migration" ];
          };

          agents = {
            root = "./.opencode/agents";
            imports = [ "build" ];
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
    code-style.md
    base/secrets.md
  skills/
    migration/
      SKILL.md
      rollback-checklist.md
  agents/
    build.md
opencode.json      # merged, not overwritten
agent-assets.lock  # one row per imported asset
```

`base/secrets.md` shows the frontmatter layering — the per-import `owner`
overrides the category `owner`, `team` comes from the category:

```markdown
---
owner: sec
team: platform
---

## Summary

Rules for handling secrets and credentials safely.
...
```

`migration/rollback-checklist.md` is a skill artifact: copied verbatim, no
frontmatter. `opencode.json` is merged, so existing keys survive:

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
  migration/          # ours -- one row in agent-assets.lock
  my-own-skill/       # yours -- never touched
```

`my-own-skill/` survives syncs and survives dropping the `migration` import.
Inside `migration/`, though, the asset is immutable after injection: a stray
file added there is reported as `EXTRA` by `check` and removed by `sync`.

## From there

```bash
nix run .#check   # clean now; exits 1 if anyone edits a managed file
```

`opencode.json` is deliberately not owned — the manifest never lists it, so
`check` cannot delete it and edits to unrelated keys are never flagged. Only
the absence of the injected keys is reported, as `INJECT`.

To drop an asset later, remove it from `imports` and run `nix run .#sync`: its
path is deleted as a whole (a file, or a directory asset's entire directory).
