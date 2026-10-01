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
nix run .#agentAssets.sync
```

This places:

- `.opencode/rules/code-style.md`
- `.opencode/rules/conventions/commit-convention.md`
- `.opencode/skills/migration/SKILLS.md`
- `.opencode/skills/migration/rollback-checklist.md`
- `.opencode/agents/build.md`
- `./opencode.json` (with the `instructions` entry)

Only files carrying the provenance marker are overwritten.

Skills are directory-based: the imported skill directory is copied as-is, and `SKILLS.md` is validated and may receive frontmatter injections.

## Check for drift

```bash
nix run .#agentAssets.check
```

## Result

OpenCode loads the rules via `opencode.json` and auto-imports skills and agents from `.opencode/skills` and `.opencode/agents`.
