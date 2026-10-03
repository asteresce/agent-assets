{ lib, config, ... }:

let
  cfg = config.agentAssets;
  resolvedConfig =
    if lib.isPath cfg.config then import cfg.config
    else cfg.config;
  hasConfig = resolvedConfig != {};
  isActive = cfg.enable && hasConfig;

  # Apps are emitted as `apps.<system>.<name>` so consumers can run
  # `nix run .#sync` and `nix run .#check`. The list of apps is
  # configurable so consumers can opt out or add new subcommands.
  makeProgram = pkgs: name:
    let drv = (import ./scripts/wrapper.nix { inherit pkgs; }) {
      inherit name;
      config = resolvedConfig;
      manifest = cfg.manifest;
    };
    in "${drv}/bin/agent-assets-${name}";
in
{
  options.agentAssets = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable the agent-assets module. When false, no apps are emitted.
        `nix run .#sync` requires `enable = true` and a non-empty
        `config`.
      '';
    };
    config = lib.mkOption {
      type = lib.types.either lib.types.attrs (lib.types.path);
      default = {};
      description = ''
        Declarative configuration. Can be an attrset or a path that
        evaluates to the same shape.
      '';
    };
    manifest = lib.mkOption {
      type = lib.types.str;
      default = "./agent-assets.lock";
      description = ''
        Path to the lock file, relative to the project root. Sync writes
        this file and check reads it.
      '';
    };
    apps = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "sync" "check" ];
      description = ''
        Subcommand names to expose as flake apps. Each name must
        correspond to a `python -m agent_assets <name>` subcommand.
      '';
    };
  };

  config = {
    perSystem = { pkgs, ... }: {
      apps = lib.genAttrs cfg.apps (name:
        lib.optionalAttrs isActive {
          type = "app";
          program = makeProgram pkgs name;
        });
    };
  };
}