"""Frontmatter parsing, merging and rendering.

Frontmatter is YAML at the top of a Markdown file, delimited by `---`.
Lists in frontmatter are *deduplicated* on merge (so authors can list
tags without producing duplicates when both source and injection name
the same one). This is intentionally different from JSON injection,
where lists are concatenated.
"""

from __future__ import annotations

import yaml

from .merge import deep_merge


def split_frontmatter(source: str) -> tuple[dict, str]:
    """Return `(frontmatter_dict, body)`. If no frontmatter is present,
    returns `({}, body)`."""
    if not source.startswith("---"):
        return {}, source
    rest = source[3:]
    end = rest.find("\n---")
    if end < 0:
        return {}, source
    head = rest[:end]
    body = rest[end + 4:]
    if body.startswith("\n"):
        body = body[1:]
    if body.startswith("\r\n"):
        body = body[2:]
    parsed = yaml.safe_load(head) if head.strip() else {}
    if parsed is None:
        parsed = {}
    if not isinstance(parsed, dict):
        parsed = {}
    return parsed, body


def render(frontmatter: dict, body: str) -> str:
    if not frontmatter:
        return body
    yaml_text = yaml.safe_dump(
        frontmatter,
        default_flow_style=False,
        allow_unicode=True,
        sort_keys=True,
        width=4096,
    )
    return f"---\n{yaml_text}---\n{body}"


def apply(
    source: str,
    category_frontmatter: dict | None,
    import_frontmatter: dict | None,
) -> str:
    """Apply category- and import-level frontmatter to `source`.

    Merge order (highest priority last):
      1. Frontmatter already in the source.
      2. Category-level `injections.frontmatter`.
      3. Per-import `injections.frontmatter`.
    """
    source_fm, body = split_frontmatter(source)
    merged = deep_merge(
        deep_merge({}, source_fm, list_policy="dedupe"),
        deep_merge(category_frontmatter or {}, import_frontmatter or {}, list_policy="dedupe"),
        list_policy="dedupe",
    )
    return render(merged, body)