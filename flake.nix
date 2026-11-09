{
  description = "Provider-neutral shared agent assets with a small sync/check engine";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane, ... }:
    let
      # One declaration for every default and label; see spec.json.
      spec = builtins.fromJSON (builtins.readFile ./spec.json);
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
          # `spec.json` is include_str!'d, `assets/` is walked by build.rs and
          # read again by the registry tests, and Cargo.toml names README.md.
          # crane's default filter keeps only Cargo and Rust files, so all
          # three are named here. Everything else — docs, CI, this flake — is
          # excluded, so editing it does not rebuild the engine.
          srcFilter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*/spec\\.json$" path != null)
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
