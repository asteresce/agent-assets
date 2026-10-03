{ pkgs }:

let
  src = ./..;
  scripts = src + "/scripts";
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
in
config:

let
  configJson = pkgs.writeText "agent-assets-config.json" (builtins.toJSON config);
  overlay = pkgs.runCommand "agent-assets-overlay" { } ''
    mkdir -p $out
    ${pythonWithYaml}/bin/python3 ${scripts}/build-overlay.py \
      --config ${configJson} \
      --src ${src} \
      --out $out
  '';
in
{
  package = overlay;
  path = "${overlay}";
}