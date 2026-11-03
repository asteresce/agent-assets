{
  description = "Provider-neutral shared agent assets with a small sync/check engine";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      # One declaration for every default and label; see spec.json.
      spec = builtins.fromJSON (builtins.readFile ./spec.json);
      manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      # Consumer entry point: see flake-module.nix and docs/api.md.
      flakeModules.default = import ./flake-module.nix self;

      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          # The engine, with `spec.json` compiled in. The registry ships
          # beside it so a release archive is self-contained; `--assets`
          # substitutes another registry.
          default = pkgs.rustPlatform.buildRustPackage {
            pname = spec.name;
            version = manifest.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
            postInstall = ''
              cp -r ${./assets} $out/${spec.defaults.assets}
            '';
            meta = {
              description = spec.name;
            };
          };
        });

      apps = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          package = self.packages.${system}.default;
          # A one-line wrapper so `nix run .#sync` forwards to `agent-assets
          # sync`. The engine itself has no runtime tools.
          app = name: {
            type = "app";
            program = "${pkgs.writeShellScript "${spec.name}-${name}" ''
              exec ${package}/bin/${spec.name} ${name} "$@"
            ''}";
          };
        in
        {
          sync = app "sync";
          check = app "check";
          render = app "render";
        });

      # The package build runs `cargo test`, so checking it covers the unit
      # tests, the behaviour suite, the registry conformance and the golden.
      checks = forAllSystems (system: {
        default = self.packages.${system}.default;
      });
    };
}
