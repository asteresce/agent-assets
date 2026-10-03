# Build helper for `nix flake check` derivations.
#
# `mkCheck { name, src, buildPhase }` produces a stdenvNoCC derivation
# that runs the given `buildPhase` with the agent-assets Python
# environment and PYTHONPATH wired in. Used by `self-test.nix` and
# `nix-roundtrip.nix`.
{ pkgs }:

{ name, src, buildPhase }:

let
  inherit (pkgs) stdenvNoCC;
  pythonWithYaml = import ../lib/python-env.nix { inherit pkgs; };
in

stdenvNoCC.mkDerivation {
  inherit name src;
  nativeBuildInputs = [ pythonWithYaml ];
  buildPhase = ''
    runHook preBuild
    export PYTHONPATH=$PWD/scripts
    export LANG=C.UTF-8
    ${buildPhase}
    runHook postBuild
  '';
  installPhase = ''
    mkdir -p $out
    touch $out/success
  '';
}