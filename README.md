# Shared Agent Assets

A reproducible, provider-agnostic Nix flake module for importing, adapting, and sharing agent rules, skills, and prompts across projects.

The module reads a single declarative configuration and emits a project-tree overlay. Client projects apply that overlay and wire it into their agent of choice. OpenCode is shown as an example consumer.

## What this project provides

- Shared, provider-neutral assets under `rules/`, `skills/`, and `agents/`.
- A Nix flake module with one configuration entry point: `agentAssets.config`.
- Injections that transform imported assets:
  - `frontmatter` — inject YAML frontmatter.
  - `json` — inject static content into JSON files.
- Automatic provenance comments on imported files.
- Flake apps for syncing and checking imported assets.

## Quick start

```nix
{
  inputs = {
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
          };
        };
      };
    };
}
```

## Sync to the project

```bash
nix run .#agentAssets.sync
```

This copies the emitted tree into the project, overwriting only files that carry the provenance marker.

## Check for drift

```bash
nix run .#agentAssets.check
```

This compares imported files in the project against the generated tree and reports differences.

## Moving the configuration to a separate file

`agentAssets.config` can also be a path:

```nix
agentAssets.config = ./agent-assets.nix;
```

Because the config is pure data, it can later be converted to JSON (`./agent-assets.json`) without changing the schema.

## Documentation

- [`docs/concepts.md`](docs/concepts.md) — core abstractions.
- [`docs/api.md`](docs/api.md) — Nix flake module API and config schema.
- [`docs/injections/frontmatter.md`](docs/injections/frontmatter.md) — frontmatter injection.
- [`docs/injections/json.md`](docs/injections/json.md) — JSON injection.
- [`docs/examples/opencode.md`](docs/examples/opencode.md) — OpenCode example.
- [`rules/README.md`](rules/README.md), [`skills/README.md`](skills/README.md), [`agents/README.md`](agents/README.md) — asset catalogs.

## Project layout

```
flake.nix          # flake outputs (flake-parts module, lib helpers)
rules/             # provider-neutral shared rules
skills/            # provider-neutral shared skills
agents/            # provider-neutral shared agent prompts
docs/              # human-readable documentation
opencode.json      # OpenCode config used to maintain this repo
AGENTS.md          # contributor guidance for this repo
```

## Status

This documentation describes the intended public API. The Nix module implementation is being developed alongside these docs.
