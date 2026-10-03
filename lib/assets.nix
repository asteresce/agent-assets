# Registry of shared asset sources under `src/{rules,skills,agents}/`.
#
# `pickers.<format>` selects how each category's directory is walked.
# The per-category format is derived from `lib/schemas/formats.nix`, so
# adding or renaming a category there automatically updates the registry.
let
  formats = import ./schemas/formats.nix;
  mdExt = "\\.md$";
  stripExt = name:
    let parts = builtins.split mdExt name;
    in if builtins.length parts == 0 then name else builtins.head parts;
  isAssetFile = name:
    let ext = stripExt name;
    in ext != name;

  walk = fmt: src: category:
    let
      dir = src + "/${category}";
      entries = builtins.readDir dir;
    in
    if fmt == "file" then
      let
        isMdFile = n: entries.${n} == "regular" && n != "README.md" && isAssetFile n;
        names = builtins.filter isMdFile (builtins.attrNames entries);
      in
      builtins.listToAttrs (map
        (n: { name = stripExt n; value = dir + "/${n}"; })
        names)
    else
      let
        isDir = n: entries.${n} == "directory";
        names = builtins.filter isDir (builtins.attrNames entries);
      in
      builtins.listToAttrs (map
        (n: { name = n; value = dir + "/${n}"; })
        names);
in
src: {
  rules  = walk formats.rules  src "rules";
  skills = walk formats.skills src "skills";
  agents = walk formats.agents src "agents";
}