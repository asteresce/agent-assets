{ pkgs }:

(pkgs.callPackage ./mkCheck.nix { }) {
  name = "agent-assets-nix-roundtrip";
  src = ../.;

  buildPhase = ''
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

    python3 -m agent_assets build \
      --config $tmp/config.json \
      --src $PWD \
      --out $tmp/emission

    mkdir -p $tmp/project
    python3 -m agent_assets sync \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock

    test -f $tmp/emission/agent-assets.lock || { echo "missing emission lock"; exit 1; }
    test -f $tmp/project/agent-assets.lock || { echo "missing project lock"; exit 1; }
    test -f $tmp/project/.opencode/rules/security.md || { echo "missing security.md"; exit 1; }
    test -f $tmp/project/.opencode/skills/migration/SKILLS.md || { echo "missing SKILLS.md"; exit 1; }
    test -f $tmp/project/.opencode/agents/build.md || { echo "missing build.md"; exit 1; }

    python3 -m agent_assets sync \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock

    python3 -m agent_assets check \
      --project $tmp/project \
      --emission $tmp/emission \
      --config $tmp/config.json \
      --manifest ./agent-assets.lock
  '';
}