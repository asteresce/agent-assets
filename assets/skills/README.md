# Shared skills catalog

These skills are provider-neutral. Consumers import them and adapt them for
their agent.

Each skill is a directory containing an entry file (`SKILL.md`) and any
artifacts it needs, referenced in a config by its directory name.

- [`tdd/`](tdd/) — the red-green-refactor loop, its preconditions and its
  pitfalls. Bundles [`checklist.md`](tdd/checklist.md).

The sections a skill's entry file may carry are declared in
[`spec.json`](../../spec.json) under `headings.skills`. Artifacts are
free-form.

Add new skills here; this listing is just the directory.
