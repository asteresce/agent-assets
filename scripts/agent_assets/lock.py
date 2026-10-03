"""Lock-file read/write.

The lock file is the source of truth for which files are owned by
agent-assets and what their on-disk state should look like. The format
is intentionally simple so it can be diffed and reviewed.

See `constants.LOCK_VERSION` for the current schema version.
"""

from __future__ import annotations

import json
import os

from .constants import LOCK_VERSION
from .fsutil import write_text


def entry(*, category: str, name: str, path: str, hash: str) -> dict:
    return {
        "category": category,
        "name": name,
        "path": path,
        "hash": hash,
    }


def read_lock(path: str) -> list | None:
    """Read the lock at `path`. Returns `None` if the file is missing,
    unreadable, the wrong version, or malformed. Returning `None` is
    the signal callers use to "start fresh" — see `sync.sync`."""
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (json.JSONDecodeError, OSError):
        return None
    if not isinstance(data, dict):
        return None
    if data.get("version") != LOCK_VERSION:
        return None
    entries = data.get("entries")
    if not isinstance(entries, list):
        return None
    return entries


def write_lock(path: str, entries: list) -> None:
    payload = {"version": LOCK_VERSION, "entries": entries}
    write_text(path, json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n")