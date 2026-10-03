{ pkgs }:

let
  pythonWithYaml = pkgs.python3.withPackages (ps: [ ps.pyyaml ]);
  inherit (pkgs) stdenvNoCC;
in
stdenvNoCC.mkDerivation {
  name = "agent-assets-nix-roundtrip";
  src = ../.;

  nativeBuildInputs = [ pythonWithYaml ];

  buildPhase = ''
    runHook preBuild
    export PYTHONPATH=$PWD/scripts
    export LANG=C.UTF-8
    tmp=$(mktemp -d)
    trap "rm -rf $tmp" EXIT

    cat > $tmp/config.json <<'EOF'
    ${builtins.toJSON {
      rules = {
        root = "./.opencode/rules";
        imports = [ "security" "code-style" ];
      };
      skills = {
        root = "./.opencode/skills";
        imports = [ "migration" ];
      };
      agents = {
        root = "./.opencode/agents";
        imports = [ "build" ];
      };
    }}
    EOF

    python3 scripts/build-overlay.py \
      --config $tmp/config.json \
      --src $PWD \
      --out $tmp/emission

    mkdir -p $tmp/project
    python3 scripts/sync.py \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock

    test -f $tmp/emission/agent-assets.lock || { echo "missing emission lock"; exit 1; }
    test -f $tmp/project/agent-assets.lock || { echo "missing project lock"; exit 1; }
    test -f $tmp/project/.opencode/rules/security.md || { echo "missing security.md"; exit 1; }
    test -f $tmp/project/.opencode/skills/migration/SKILLS.md || { echo "missing SKILLS.md"; exit 1; }
    test -f $tmp/project/.opencode/agents/build.md || { echo "missing build.md"; exit 1; }

    python3 scripts/sync.py \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock

    python3 scripts/check.py \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock

    runHook postBuild
  '';

  installPhase = ''
    mkdir -p $out
    touch $out/success
  '';
}