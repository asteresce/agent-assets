{ pkgs ? import <nixpkgs> {} }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
in
{ source, out, injections }:

pkgs.runCommand "agent-assets-frontmatter" { inherit source; } ''
  cat > $TMPDIR/injections.json <<'EOF'
  ${builtins.toJSON injections}
  EOF
  ${pythonWithYaml}/bin/python3 ${../scripts/apply-frontmatter.py} \
    "$source" "$out" "$TMPDIR/injections.json"
''