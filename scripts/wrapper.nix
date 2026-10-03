# Wrapper that invokes `python -m agent_assets <subcommand>`.
#
# `name` is the subcommand name (e.g. `sync`, `check`) and the binary
# suffix. The Python entry point reads `--project`, `--emission`,
# `--config` and `--manifest` directly from argv.
{ pkgs }:

{ name, config, manifest ? "./agent-assets.lock" }:

let
  pythonWithYaml = import ../lib/python-env.nix { inherit pkgs; };
  mkAgentAssets = import ../lib/mkAgentAssets.nix { inherit pkgs; };
  emission = (mkAgentAssets config).package;
  configJson = pkgs.writeText "agent-assets-config.json" (builtins.toJSON config);
in

pkgs.writeShellScriptBin "agent-assets-${name}" ''
  set -euo pipefail
  PROJECT="''${PROJECT_ROOT:-$PWD}"
  export PYTHONPATH=${../scripts}
  ${pythonWithYaml}/bin/python3 -m agent_assets ${name} \
    --project "$PROJECT" \
    --emission ${emission} \
    --config ${configJson} \
    --manifest '${manifest}'
''