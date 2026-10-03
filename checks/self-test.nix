{ pkgs }:

(pkgs.callPackage ./mkCheck.nix { }) {
  name = "agent-assets-self-test";
  src = ../.;
  buildPhase = ''
    python3 -m agent_assets version
    python3 checks/run.py
  '';
}