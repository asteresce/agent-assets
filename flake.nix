{
  description = "Provider-neutral shared agent assets with a small sync/check engine";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane, ... }:
    let
      # One declaration for every default; see defaults.json. The package
      # identity — name and description — comes from Cargo.toml, which the
      # release process already treats as its authority, so no value is
      # declared twice.
      defaults = builtins.fromJSON (builtins.readFile ./defaults.json);
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      # Consumer entry point: see flake-module.nix and docs/api.md.
      flakeModules.default = import ./flake-module.nix self;

      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.mkLib pkgs;
          # `defaults.json` and `assets/spec.json` are include_str!'d —
          # build.rs reads the latter to know what to embed — `assets/` is
          # walked by build.rs and read again by the registry tests, and
          # Cargo.toml names README.md. crane's default filter keeps only
          # Cargo and Rust files, so those are named here. Everything else —
          # docs, CI, this flake — is excluded, so editing it does not
          # rebuild the engine.
          srcFilter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*/defaults\\.json$" path != null)
            || (builtins.match ".*/assets(/.*)?$" path != null)
            || (builtins.match ".*/README\\.md$" path != null);
        in
        {
          # Dependencies build once, keyed on Cargo.toml and Cargo.lock, and
          # are reused by every later build: a source change rebuilds only
          # this crate. `doCheck` defaults to true, so `cargo test` runs in
          # the check phase and `nix flake check` stays the behaviour suite's
          # gate. pname and version come from Cargo.toml.
          default = craneLib.buildPackage {
            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = srcFilter;
              name = "source";
            };
            meta = {
              description = cargoToml.package.description;
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
            program = "${pkgs.writeShellScript "${cargoToml.package.name}-${name}" ''
              exec ${package}/bin/${cargoToml.package.name} ${name} "$@"
            ''}";
          };
        in
        {
          sync = app "sync";
          check = app "check";
          status = app "status";
          validate = app "validate";
          render = app "render";
          list = app "list";
          show = app "show";
        });

      # A shell that can build, test and publish the crate. The engine is
      # pure Rust, but something has to link it, and a bare `cargo` on NixOS
      # has no C toolchain to reach for.
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [ cargo rustc rustfmt clippy stdenv.cc ];
          };
        });

      # The package build runs `cargo test`, so checking it covers the unit
      # tests, the behaviour suite, the registry conformance and the golden.
      checks = forAllSystems (system: {
        default = self.packages.${system}.default;
      });
    };
}
