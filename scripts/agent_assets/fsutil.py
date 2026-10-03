"""Filesystem helpers shared by overlay/sync/check.

Wraps `shutil` with idempotent semantics (mkdir-before-write, replace
existing trees). All functions raise `OSError` on failure — callers
should narrow to the specific exception they care about.
"""

from __future__ import annotations

import json
import os
import shutil

from .constants import JSON_INDENT


def write_text(path: str, content: str) -> None:
    """Write `content` to `path`, creating parent directories as needed.

    If `path` already exists but is read-only (e.g. it was copied from
    a read-only source like /nix/store), it's made writable before
    being overwritten."""
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    if os.path.exists(path):
        os.chmod(path, 0o644)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)


def write_bytes(path: str, data: bytes) -> None:
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def copy_file(src: str, dst: str) -> None:
    parent = os.path.dirname(dst)
    if parent:
        os.makedirs(parent, exist_ok=True)
    shutil.copyfile(src, dst)


def copy_tree(src: str, dst: str) -> None:
    if os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def remove(path: str) -> str:
    """Remove `path` (file or dir). Returns the kind that was removed."""
    if os.path.isdir(path):
        shutil.rmtree(path)
        return "dir"
    if os.path.exists(path):
        os.remove(path)
        return "file"
    return "absent"


def load_json_target(path: str) -> tuple[dict | None, str]:
    """Read a JSON file, returning `(data, status)` where status ∈
    `{"ok", "missing", "invalid"}`."""
    if not os.path.exists(path):
        return None, "missing"
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f), "ok"
    except json.JSONDecodeError:
        return None, "invalid"


def dump_json_file(path: str, data) -> None:
    """Write `data` to `path` as stable JSON (sorted keys, ASCII=False,
    trailing newline)."""
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(
            data,
            f,
            indent=JSON_INDENT,
            sort_keys=True,
            ensure_ascii=False,
        )
        f.write("\n")