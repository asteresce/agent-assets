{
  description = "Reproducible, provider-agnostic shared agent assets (see docs/)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      headings = import ./lib/schemas/headings.nix;
      formats = import ./lib/schemas/formats.nix;
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      lib.schemas.headings = headings;
      lib.schemas.formats = formats;

      # Intended public API (documented in docs/api.md):
      #   - flakeModules.default          — flake-parts module
      #   - lib.mkAgentAssets             — pure helper for non-flake-parts users
      #   - lib.assets.<category>.<name>  — registry of shared asset source paths
      #   - lib.injections.*              — internal injection implementations
      #   - lib.schemas.headings          — section heading schemas
      #   - lib.schemas.formats           — asset format per category (file or directory)
      #   - apps.agentAssets.sync         — apply emitted tree to project
      #   - apps.agentAssets.check        — report drift and validate headings

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
        }
      );
    };
}
