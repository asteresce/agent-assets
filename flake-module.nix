# The consumer-facing flake-parts module.
#
# Imported as `import ./flake-module.nix self`, so the program it emits runs
# the package this flake builds. That is the whole point: the consumer's
# `flake.lock` pins the revision, so the emitted bytes are a function of the
# lock and nothing else. The engine is one self-contained binary; the wrapper
# below only injects the subcommand and the resolved paths.
self:
{ lib, config, ... }:

let
  # One declaration for every default and label; see spec.json.
  spec = builtins.fromJSON (builtins.readFile ./spec.json);

  cfg = config.agentAssets;
  isActive = cfg.enable && cfg.config != { };

  makeProgram = pkgs: name:
    let
      configJson =
        if builtins.isPath cfg.config
        then "${cfg.config}"
        else "${pkgs.writeText (builtins.baseNameOf spec.defaults.config) (builtins.toJSON cfg.config)}";
      package = self.packages.${pkgs.system}.default;
    in
    "${pkgs.writeShellScript "${spec.name}-${name}" ''
      exec ${package}/bin/${spec.name} ${name} \
        --config ${lib.escapeShellArg configJson} \
        --manifest ${lib.escapeShellArg cfg.manifest} \
        --project "''${PROJECT_ROOT:-$PWD}" \
        "$@"
    ''}";
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
