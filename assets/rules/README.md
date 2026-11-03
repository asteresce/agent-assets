# Shared rules catalog

These rules are provider-neutral. Consumers import them and adapt them for
their agent.

Each rule is a Markdown file, referenced in a config by its name without the
`.md` extension.

- [`conventional-commits.md`](conventional-commits.md) — commit message
  format and the reasoning behind it.

The sections a rule may carry are declared in [`spec.json`](../../spec.json)
under `headings.rules`.

Add new rules here; this listing is just the directory.
