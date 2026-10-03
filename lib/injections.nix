{ pkgs ? import <nixpkgs> {} }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
  frontmatterFn = import ./frontmatter.nix { inherit pkgs; };
in
{
  frontmatter = frontmatterFn;
}