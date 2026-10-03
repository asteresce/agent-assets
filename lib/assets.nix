let
  formats = import ./schemas/formats.nix;
in
src:
let
  stripExt = name:
    let
      parts = builtins.split "\\.md$" name;
    in
    if builtins.length parts == 0 then name else builtins.head parts;

  endsWithMd = name:
    let
      parts = builtins.split "\\.md$" name;
    in
    builtins.length parts != 0;

  readFileCategory = category:
    let
      dir = src + "/${category}";
      entries = builtins.readDir dir;
      isMdFile = n: entries.${n} == "regular" && n != "README.md" && endsWithMd n;
      names = builtins.filter isMdFile (builtins.attrNames entries);
    in
    builtins.listToAttrs (map
      (n: { name = stripExt n; value = dir + "/${n}"; })
      names);

  readDirectoryCategory = category:
    let
      dir = src + "/${category}";
      entries = builtins.readDir dir;
      isSkill = n: entries.${n} == "directory";
      names = builtins.filter isSkill (builtins.attrNames entries);
    in
    builtins.listToAttrs (map
      (n: { name = n; value = dir + "/${n}"; })
      names);
in
{
  rules  = readFileCategory "rules";
  skills = readDirectoryCategory "skills";
  agents = readFileCategory "agents";
}