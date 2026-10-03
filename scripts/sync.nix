{ pkgs }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
  mkAgentAssets = import ../lib/mkAgentAssets.nix { inherit pkgs; };
in
{ config }:

let
  result = mkAgentAssets config;
  emission = result.package;
  configJson = pkgs.writeText "agent-assets-config.json" (builtins.toJSON config);
in
pkgs.writeShellScriptBin "agent-assets-sync" ''
  set -euo pipefail
  PROJECT="''${PROJECT_ROOT:-$PWD}"
  ${pythonWithYaml}/bin/python3 ${../scripts/sync.py} \
    --project "$PROJECT" \
    --emission ${emission} \
    --config ${configJson}
''