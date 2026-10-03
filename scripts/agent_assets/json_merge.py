"""JSON injection: collect and merge static JSON chunks across categories.

Unlike frontmatter, JSON lists are concatenated on merge (we don't own
the file; we just want our keys present). The merged file is not
tracked in the lock.
"""

from __future__ import annotations

from .merge import deep_merge


def chunk_present(target, chunk) -> bool:
    """Return True iff `chunk` is fully contained in `target`.

    - Scalars: equality.
    - Lists: every item of `chunk` is in `target`.
    - Dicts: recursive containment.
    """
    if not isinstance(chunk, dict):
        return target == chunk
    if not isinstance(target, dict):
        return False
    for k, v in chunk.items():
        if k not in target:
            return False
        if isinstance(v, dict):
            if not chunk_present(target[k], v):
                return False
        elif isinstance(v, list):
            tv = target[k]
            if not isinstance(tv, list):
                return False
            for item in v:
                if item not in tv:
                    return False
        else:
            if target[k] != v:
                return False
    return True


def collect(config: dict) -> dict:
    """Walk every category's `injections.json` and deep-merge by `file`
    path. Returns a dict mapping target file path -> merged content."""
    grouped: dict = {}
    for category in ("rules", "skills", "agents"):
        cat_cfg = (config or {}).get(category) or {}
        for entry in (cat_cfg.get("injections") or {}).get("json") or []:
            fp = entry["file"]
            content = entry.get("content") or {}
            if fp in grouped:
                grouped[fp] = deep_merge(grouped[fp], content)
            else:
                grouped[fp] = content
    return grouped