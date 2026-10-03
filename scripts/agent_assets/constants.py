"""Constants shared across the agent-assets Python package.

Centralises filename extensions, special filenames, default paths and
the lock-file version. Anything that used to be a literal string repeated
across multiple modules lives here.
"""

from __future__ import annotations

# Markdown file extension used by asset sources and templates.
MD_EXT = ".md"

# Category README files (catalog pages) are excluded from the asset registry.
README_NAME = "README.md"

# Skill directories must contain this file at their root.
SKILLS_ENTRY = "SKILLS.md"

# Lock-file names.
EMISSION_LOCK_NAME = "agent-assets.lock"  # written by overlay, lives in the emission tree.
DEFAULT_MANIFEST = "./agent-assets.lock"  # project-side manifest, configurable.

# Default `root` per category, when the consumer omits it.
DEFAULT_ROOT_TEMPLATE = "./.agent-assets/{category}"

# Hash wire format produced by `hashing.format_hash` / `parse_hash`.
HASH_PREFIX = "sha256-"

# Lock-file schema version. Incompatible with prior versions on read.
LOCK_VERSION = 5

# JSON file output options shared by `dump_json_file`.
JSON_INDENT = 2