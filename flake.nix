{
  description = "Reproducible, provider-agnostic shared agent assets (see docs/)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      headings = import ./lib/schemas/headings.nix;
      formats = import ./lib/schemas/formats.nix;
      categories = builtins.attrNames formats;
      assets = import ./lib/assets.nix;
      flakeModule = ./flake-module.nix;
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      # Schema blob passed to the Python tooling. Categories, formats and
      # heading schemas come from a single source of truth (`lib/schemas/`).
      schemaForPython = {
        categories = categories;
        formats = formats;
        headings = headings;
      };

      # Public-API convenience: callers pass `pkgs` so the nixpkgs they
      # pinned is the one we use. This avoids hidden <nixpkgs> imports.
      mkAgentAssets = pkgs: import ./lib/mkAgentAssets.nix { inherit pkgs; };
      pythonWithYaml = pkgs: import ./lib/python-env.nix { inherit pkgs; };
    in
    {
      flakeModules.default = flakeModule;

      lib = {
        schemas = { inherit headings formats; };
        assets = assets;
        mkAgentAssets = mkAgentAssets;
      };

      checks = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          headings = pkgs.stdenvNoCC.mkDerivation {
            name = "headings-check";
            src = ./.;
            nativeBuildInputs = [ (pythonWithYaml pkgs) ];
            buildPhase = ''
              export PYTHONPATH=$PWD/scripts
              export LANG=C.UTF-8
              schema=$(mktemp)
              cat > $schema <<'SCHEMA_EOF'
              ${builtins.toJSON schemaForPython}
              SCHEMA_EOF
              python3 ${./scripts/check-headings.py} \
                --schema $schema
            '';
            installPhase = "mkdir -p $out";
          };
          selfTest = pkgs.callPackage ./checks/self-test.nix { };
          nixRoundtrip = pkgs.callPackage ./checks/nix-roundtrip.nix { };
          schemaTables = pkgs.stdenvNoCC.mkDerivation {
            name = "schema-tables-check";
            src = ./.;
            nativeBuildInputs = [ (pythonWithYaml pkgs) ];
            buildPhase = ''
              export LANG=C.UTF-8
              expected=$(mktemp)
              cat > $expected <<'EOF'
              ${(import ./lib/gen-schema-tables.nix { inherit headings; }).rules}
              ${(import ./lib/gen-schema-tables.nix { inherit headings; }).skills}
              ${(import ./lib/gen-schema-tables.nix { inherit headings; }).agents}
              EOF
              python3 ${./scripts/check-schema-tables.py} \
                $expected \
                $expected \
                docs/writing-assets.md
            '';
            installPhase = "mkdir -p $out";
          };
        });
    };
}