# Shared rules catalog

These rules are provider-neutral. Consumers import them and adapt them for their agent.

Each rule can be referenced in a config by its file name without the `.md` extension.

| Rule | Description | Suggested frontmatter |
|---|---|---|
| [`security.md`](security.md) | Prevent committing secrets and handle credentials safely. | `owner`, `severity` |
| [`code-style.md`](code-style.md) | General code consistency and review habits. | `team`, `language` |

Add new rules here and document them in this file.
