# OpenCode example

This page shows how an OpenCode project can import shared rules, skills, and agent prompts, then apply the emitted tree.

The module itself has no OpenCode-specific options; the wiring below is entirely on the client side.

## Client `flake.nix`

```nix
{
  description = "My project using shared agent assets";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    agent-assets.url = "github:your-org/agent-assets";
  };

  outputs = inputs@{ self, flake-parts, agent-assets, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ agent-assets.flakeModules.default ];

      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

      perSystem = { config, pkgs, ... }: {
        agentAssets = {
          enable = true;
          config = {
            rules = {
              root = "./.opencode/rules";
              imports = [
                "code-style"
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
              ];
              injections = {
                frontmatter = {
                  team = "platform";
                };
                json = [
                  {
                    file = "./opencode.json";
                    content = {
                      instructions = [ ".opencode/rules/**/*.md" ];
                    };
                  }
                ];
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
    };
}
```

## Apply to the project

```bash
nix run .#sync
```

This:

- Places the imported assets under their category roots:
  - `.opencode/rules/code-style.md`
  - `.opencode/rules/conventions/commit-convention.md`
  - `.opencode/skills/migration/SKILLS.md`
  - `.opencode/skills/migration/rollback-checklist.md`
  - `.opencode/agents/build.md`
- Merges the JSON injection into `./opencode.json` (existing keys preserved).
- Skips owned files whose on-disk hash already matches `./agent-assets.lock`.
- Refreshes `./agent-assets.lock`.

Custom local files under managed roots (not in any lock) are preserved. Files listed in a previous lock but absent from the current emission are deleted.

Skills are directory-based: the imported skill directory is copied as-is, and `SKILLS.md` is validated and may receive frontmatter injections. The skill's directory hash captures the contents of all files we manage.

## Check for drift

```bash
nix run .#check
```

Reports any owned file or skill dir whose hash differs from `./agent-assets.lock`, any JSON injection target missing our chunk's keys, and any orphans under managed roots. Exits non-zero on any report.

## Result

OpenCode loads the rules via `opencode.json` and auto-imports skills and agents from `.opencode/skills` and `.opencode/agents`.
