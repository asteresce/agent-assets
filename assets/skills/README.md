# Shared skills catalog

These skills are provider-neutral. Consumers import them and adapt them for
their agent.

Each skill is a directory containing an entry file (`SKILL.md`) and any
artifacts it needs, referenced in a config by its directory name.
`agent-assets list` names what is here.

The sections a skill's entry file may carry are declared in
[`spec.json`](../../spec.json) under `headings.skills`. Artifacts are
free-form.

Add a directory here and it is importable; nothing else needs updating.
