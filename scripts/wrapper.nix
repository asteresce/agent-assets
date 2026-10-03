{ pkgs }:

{ name, script, config, manifest ? "./agent-assets.lock" }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
  mkAgentAssets = import ../lib/mkAgentAssets.nix { inherit pkgs; };
  result = mkAgentAssets config;
  emission = result.package;
  configJson = pkgs.writeText "agent-assets-config.json" (builtins.toJSON config);
in
pkgs.writeShellScriptBin "agent-assets-${name}" ''
  set -euo pipefail
  PROJECT="''${PROJECT_ROOT:-$PWD}"
  ${pythonWithYaml}/bin/python3 ${../scripts}/${script}.py \
    --project "$PROJECT" \
    --emission ${emission} \
    --config ${configJson} \
    --manifest '${manifest}'
''