# Shared agents catalog

These agent prompts are provider-neutral. Consumers import them and adapt
them for their agent.

Each agent is a Markdown file, referenced in a config by its name without the
`.md` extension.

- [`code-reviewer.md`](code-reviewer.md) — a sceptical review of a proposed
  change, ordered by severity.

The sections an agent prompt may carry are declared in
[`spec.json`](../../spec.json) under `headings.agents`.

Add new agents here; this listing is just the directory.
