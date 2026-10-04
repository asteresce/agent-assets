{ lib, config, ... }:

let
  # One declaration for every default and label; the engine and the docs
  # generator read the same file. See spec.json.
  spec = builtins.fromJSON (builtins.readFile ./spec.json);

  cfg = config.agentAssets;
  isActive = cfg.enable && cfg.config != { };

  # The registry this flake ships. The standalone engine defaults --src to
  # its own checkout; the apps pin it to this store path, which is the
  # consumer's flake.lock-pinned revision of this flake. The engine is run
  # from here too, so its spec.json lookup resolves inside the store.
  registry = ./.;

  makeProgram = pkgs: name:
    let
      configJson =
        if builtins.isPath cfg.config
        then "${cfg.config}"
        else "${pkgs.writeText (builtins.baseNameOf spec.defaults.config) (builtins.toJSON cfg.config)}";
      app = pkgs.writeShellApplication {
        name = "${spec.name}-${name}";
        runtimeInputs = [
          pkgs.bash
          pkgs.coreutils
          pkgs.diffutils
          pkgs.findutils
          pkgs.gawk
          pkgs.jq
          pkgs.yq-go
        ];
        text = ''
          exec ${pkgs.bash}/bin/bash ${registry}/bin/agent-assets ${lib.escapeShellArg name} \
            --config ${lib.escapeShellArg configJson} \
            --src ${lib.escapeShellArg "${registry}"} \
            --manifest ${lib.escapeShellArg cfg.manifest} \
            --project "''${PROJECT_ROOT:-$PWD}" \
            "$@"
        '';
      };
    in
    "${app}/bin/${spec.name}-${name}";
in
{
  options.agentAssets = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable the agent-assets module. When false, no apps are emitted.
      '';
    };

    config = lib.mkOption {
      type = lib.types.either lib.types.attrs lib.types.path;
      default = { };
      description = ''
        Declarative configuration: an attribute set (serialized to JSON) or
        a path to a JSON file with the same shape. See docs/api.md.
      '';
    };

    manifest = lib.mkOption {
      type = lib.types.str;
      default = spec.defaults.manifest;
      description = ''
        Path to the ownership manifest, relative to the project root.
        Sync writes it, check reads it. Defaults to
        `${spec.defaults.manifest}`, the engine's own default.
      '';
    };
  };

  config = {
    perSystem = { pkgs, ... }: {
      apps = lib.optionalAttrs isActive (
        lib.genAttrs [ "sync" "check" ] (name: {
          type = "app";
          program = makeProgram pkgs name;
        })
      );
    };
  };
}
