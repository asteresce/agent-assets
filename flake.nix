{
  description = "Provider-neutral shared agent assets with a small sync/check engine";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      # One declaration for every default and label; see spec.json.
      spec = builtins.fromJSON (builtins.readFile ./spec.json);
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      # Consumer entry point: see flake-module.nix and docs/api.md.
      flakeModules.default = ./flake-module.nix;

      checks = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          # Integration suite for bin/agent-assets plus registry conformance.
          selfTest = pkgs.runCommand "${spec.name}-self-test"
            {
              nativeBuildInputs = [
                pkgs.bash
                pkgs.coreutils
                pkgs.diffutils
                pkgs.findutils
                pkgs.gawk
                pkgs.jq
                pkgs.python3
                pkgs.yq-go
              ];
            } ''
              export LANG=C.UTF-8
              cp -r ${./.} src
              chmod -R u+w src
              cd src
              python3 -m unittest discover -s tests -v
              mkdir -p $out
            '';

          shellcheck = pkgs.runCommand "${spec.name}-shellcheck"
            {
              nativeBuildInputs = [ pkgs.shellcheck ];
            } ''
              shellcheck ${./bin/agent-assets}
              mkdir -p $out
            '';

          # docs/reference.generated.md must match spec.json.
          reference = pkgs.runCommand "${spec.name}-reference"
            {
              nativeBuildInputs = [ pkgs.python3 ];
            } ''
              export LANG=C.UTF-8
              cp -r ${./.} src
              chmod -R u+w src
              cd src
              python3 scripts/gen-docs.py --check
              mkdir -p $out
            '';
        });
    };
}
