# Renders schema tables for `docs/writing-assets.md` from
# `lib/schemas/headings.nix`. Used by the `schemaTables` flake check
# to ensure the docs and the schema agree.
{ headings }:

let
  levelOf = entry: entry.level or 2;
  concatMapStrings = sep: f: list:
    builtins.concatStringsSep sep (map f list);

  renderRow = entry:
    "| ${entry.heading} | ${if entry.optional or false then "no" else "yes"} | ${toString (levelOf entry)} |";

  renderTable = title: schema:
    let
      preamble = if title == "Skills" then
        "\n\nSkills are directory-based. Each skill lives in its own directory under `skills/` and contains a `SKILLS.md` file plus any artifacts it needs."
      else
        "";
    in ''
      ### ${title}${preamble}

      | Section | Required | Level |
      |---|---|---|
      ${concatMapStrings "\n" renderRow schema}
    '';
in
{
  rules  = renderTable "Rules"  headings.rules;
  skills = renderTable "Skills" headings.skills;
  agents = renderTable "Agents" headings.agents;
}