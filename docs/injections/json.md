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

JSON injection does not take ownership of the file. The target file is not
listed in `./agent-assets.lock`, not hashed, and not deleted on orphan
removal. Sync deep-merges the `content` into the existing file (or creates a
new one with our keys if missing), preserving user-added keys. Re-running
with the same config is idempotent — our keys remain, user keys are preserved.

The target file lives wherever the config places it, relative to the project root.

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
