{ lib, config, ... }:

let
  cfg = config.agentAssets;
  resolvedConfig =
    if lib.isPath cfg.config then import cfg.config
    else cfg.config;
  isActive = cfg.enable && resolvedConfig != {};
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
        Declarative configuration. Can be an attrset or a path that evaluates
        to the same shape.
      '';
    };
    manifest = lib.mkOption {
      type = lib.types.str;
      default = "./agent-assets.lock";
      description = ''
        Path to the lock file, relative to the project root. Sync writes this
        file and check reads it.
      '';
    };
  };

  # Apps are emitted as `apps.<system>.sync` and `apps.<system>.check` so
  # consumers can run `nix run .#sync` and `nix run .#check`.
  config = {
    perSystem = { pkgs, ... }: let
      makeProgram = params:
        let drv = (import ./scripts/wrapper.nix { inherit pkgs; }) params;
        in "${drv}/bin/agent-assets-${params.name}";
    in {
      apps.sync = lib.optionalAttrs isActive {
        type = "app";
        program = makeProgram {
          name = "sync";
          script = "sync";
          config = resolvedConfig;
          manifest = cfg.manifest;
        };
      };
      apps.check = lib.optionalAttrs isActive {
        type = "app";
        program = makeProgram {
          name = "check";
          script = "check";
          config = resolvedConfig;
          manifest = cfg.manifest;
        };
      };
    };
  };
}