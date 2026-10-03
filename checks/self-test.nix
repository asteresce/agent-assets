{ pkgs }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
  inherit (pkgs) stdenvNoCC;
in
stdenvNoCC.mkDerivation {
  name = "agent-assets-self-test";
  src = ../.;

  nativeBuildInputs = [ pythonWithYaml ];

  buildPhase = ''
    runHook preBuild
    export PYTHONPATH=$PWD/scripts
    python3 checks/run.py
    runHook postBuild
  '';

  installPhase = ''
    mkdir -p $out
    touch $out/success
  '';
}