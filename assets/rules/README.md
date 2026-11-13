# Shared rules catalog

These rules are provider-neutral. Consumers import them and adapt them for
their agent.

Each rule is a Markdown file, referenced in a config by its name without the
`.md` extension. `agent-assets list` names what is here.

The sections a rule may carry are declared in [`spec.json`](../spec.json)
under `rules.headings`.

Add a file here and it is importable; nothing else needs updating.
