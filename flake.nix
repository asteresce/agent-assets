{
  description = "Reproducible, provider-agnostic shared agent assets (see docs/)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      headings = import ./lib/schemas/headings.nix;
      formats = import ./lib/schemas/formats.nix;
      assets = import ./lib/assets.nix;
      injections = import ./lib/injections.nix;
      mkAgentAssets = import ./lib/mkAgentAssets.nix;
      flakeModule = ./flake-module.nix;
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      flakeModules.default = flakeModule;

      lib = {
        inherit headings formats assets injections mkAgentAssets;
        scripts = {
          sync = import ./scripts/sync.nix;
          check = import ./scripts/check.nix;
        };
      };

      checks = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          headings = pkgs.stdenvNoCC.mkDerivation {
            name = "headings-check";
            src = ./.;
            nativeBuildInputs = [ pkgs.python3 ];
            buildPhase = ''
              python3 ${./scripts/check-headings.py} \
                --headings '${builtins.toJSON headings}' \
                --formats '${builtins.toJSON formats}' \
                --dirs rules skills agents
            '';
            installPhase = "mkdir -p $out";
          };
          selfTest = pkgs.callPackage ./checks/self-test.nix { };
        });
    };
}