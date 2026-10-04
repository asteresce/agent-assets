{
  description = "Provider-neutral shared agent assets with a small sync/check engine";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      # Consumer entry point: see flake-module.nix and docs/api.md.
      flakeModules.default = ./flake-module.nix;

      checks = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          # Integration suite for bin/agent-assets.
          selfTest = pkgs.runCommand "agent-assets-self-test"
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

          shellcheck = pkgs.runCommand "agent-assets-shellcheck"
            {
              nativeBuildInputs = [ pkgs.shellcheck ];
            } ''
              shellcheck ${./bin/agent-assets}
              mkdir -p $out
            '';
        });
    };
}
