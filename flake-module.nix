{ lib, config, ... }:

let
  cfg = config.agentAssets;
  mkAgentAssets = import ./mkAgentAssets.nix;
  resolvedConfig =
    if lib.isPath cfg.config then import cfg.config
    else cfg.config;
  output =
    if cfg.enable && resolvedConfig != {} then
      mkAgentAssets resolvedConfig
    else
      { package = null; path = null; };
in
{
  options.agentAssets = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable the agent-assets module.";
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
        Path to the lock file. Required and non-nullable; sync always creates
        or refreshes it.
      '';
    };
    output = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = { package = null; path = null; };
      description = "Read-only output: { package, path }.";
    };
  };

  config = {
    agentAssets.output = output;
  };
}