# Python environment used by agent-assets tooling.
#
# Single source of truth for `python3 + pyyaml`, imported from
# `lib/mkAgentAssets.nix`, `scripts/wrapper.nix`, `checks/self-test.nix`
# and `checks/nix-roundtrip.nix`.
{ pkgs }:

pkgs.python3.withPackages (ps: [ ps.pyyaml ])