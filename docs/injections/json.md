# JSON injection

The JSON injection writes static content into JSON files in the emitted tree. It is data-driven and agent-agnostic: the module does not interpret the content.

## Enabling the injection

Add a `json` list under a category's `injections`:

```nix
{
  rules = {
    injections = {
      json = [
        {
          file = "./opencode.json";
          content = {
            instructions = [ ".opencode/rules/**/*.md" ];
          };
        }
      ];
    };
  };
}
```

## Merging

`injections.json` is a list of `{ file, content }` objects. If multiple entries target the same `file` (possibly across categories), their `content` is deep-merged.

## Output

The resulting JSON file is placed in the emitted tree at the specified `file` path, relative to the overlay root. The client applies the overlay to their project.

## Example

Two categories contributing to the same file:

```nix
{
  rules.injections.json = [
    { file = "./opencode.json"; content = { instructions = [ ".opencode/rules/**/*.md" ]; }; }
  ];
  agents.injections.json = [
    { file = "./opencode.json"; content = { agent = { build = { description = "Build agent"; }; }; }; }
  ];
}
```

The emitted `./opencode.json` will contain both `instructions` and `agent`.
