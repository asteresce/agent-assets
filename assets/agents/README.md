# Shared agents catalog

These agent prompts are provider-neutral. Consumers import them and adapt
them for their agent.

Each agent is a Markdown file, referenced in a config by its name without the
`.md` extension. `agent-assets list` names what is here.

The sections an agent prompt may carry are declared in
[`spec.json`](../spec.json) under `agents.headings`.

Add a file here and it is importable; nothing else needs updating.
