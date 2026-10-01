# Frontmatter injection

The frontmatter injection adds YAML frontmatter keys to imported Markdown assets.

## Enabling the injection

Add a `frontmatter` entry under a category's `injections`:

```nix
{
  rules = {
    injections = {
      frontmatter = {
        team = "platform";
      };
    };
  };
}
```

This frontmatter is applied to every asset imported in that category.

## Global vs. per-import frontmatter

Frontmatter is merged in this order, with later layers overriding earlier ones:

1. Frontmatter already present in the asset source.
2. Category-level `injections.frontmatter`.
3. Per-import `injections.frontmatter`.

## Example

Asset source:

```markdown
## Summary

Do not commit secrets.
```

Configuration:

```nix
{
  rules = {
    imports = [
      {
        name = "security";
        rename = "security";
        injections = {
          frontmatter = {
            owner = "security-team";
          };
        };
      }
    ];

    injections = {
      frontmatter = {
        team = "platform";
        owner = "default-team";
      };
    };
  };
}
```

Emitted file:

```markdown
---
team: platform
owner: security-team
---

## Summary

Do not commit secrets.
```

The provenance comment is inserted immediately after the frontmatter block.
