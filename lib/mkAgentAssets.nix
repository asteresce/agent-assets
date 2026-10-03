# Build helper that produces an emission-tree derivation from a config.
#
# `mkAgentAssets pkgs config` returns `{ package, path }`. `package` is
# the emission tree; `path` is the same path as a string.
{ pkgs }:

let
  src = ./..;
  scripts = src + "/scripts";
  pythonWithYaml = import ./python-env.nix { inherit pkgs; };
in

config:

let
  configJson = pkgs.writeText "agent-assets-config.json" (builtins.toJSON config);
  overlay = pkgs.runCommand "agent-assets-overlay" { } ''
    mkdir -p $out
    export PYTHONPATH=${scripts}
    ${pythonWithYaml}/bin/python3 -m agent_assets build \
      --config ${configJson} \
      --src ${src} \
      --out $out
  '';
in
{
  package = overlay;
  path = "${overlay}";
}