"""Compute the diff between on-disk state and the lock file.

`drift` is the building block for both `sync` (which applies the diff)
and `check` (which reports it). The function returns typed deltas
instead of action strings, so both consumers share the same walk.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Literal

from . import hashing

Kind = Literal["file", "dir"]

# Categories whose entries are directories (mirrors
# `lib/schemas/formats.nix`).
DIR_CATEGORIES = frozenset({"skills"})


def _kind_for(entry: dict) -> Kind:
    return "dir" if entry.get("category") in DIR_CATEGORIES else "file"


@dataclass
class Delta:
    """One actionable difference between on-disk state and the new lock.

    `path` is project-relative. `kind` is `file` or `dir`. `status`
    indicates the relationship:

      - `unchanged`: on-disk matches lock entry (skip).
      - `drift`: on-disk exists but hash differs (overwrite / recopy).
      - `missing`: on-disk does not exist (copy).
      - `orphan`: in previous lock only (remove on sync, report on check).
    """

    path: str
    kind: Kind
    status: str
    expected: str | None = None
    actual: str | None = None
    entry: dict | None = field(default=None, repr=False)


def _classify_file(project_root: str, entry: dict) -> Delta:
    target = os.path.join(project_root, entry["path"])
    if not os.path.exists(target):
        return Delta(path=entry["path"], kind="file", status="missing", entry=entry)
    current = hashing.format_hash(hashing.sha256_file(target))
    if current == entry["hash"]:
        return Delta(
            path=entry["path"], kind="file", status="unchanged",
            expected=entry["hash"], actual=current, entry=entry,
        )
    return Delta(
        path=entry["path"], kind="file", status="drift",
        expected=entry["hash"], actual=current, entry=entry,
    )


def _classify_dir(project_root: str, emission_root: str, entry: dict) -> Delta:
    target = os.path.join(project_root, entry["path"])
    if not os.path.exists(target):
        return Delta(path=entry["path"], kind="dir", status="missing", entry=entry)
    emission_target = os.path.join(emission_root, entry["path"])
    try:
        registry_files = hashing.list_registry_files(emission_target)
    except FileNotFoundError:
        return Delta(path=entry["path"], kind="dir", status="missing", entry=entry)
    current = hashing.dir_hash(target, registry_files)
    if current is None:
        return Delta(
            path=entry["path"], kind="dir", status="drift",
            expected=entry["hash"], actual=None, entry=entry,
        )
    if current == entry["hash"]:
        return Delta(
            path=entry["path"], kind="dir", status="unchanged",
            expected=entry["hash"], actual=current, entry=entry,
        )
    return Delta(
        path=entry["path"], kind="dir", status="drift",
        expected=entry["hash"], actual=current, entry=entry,
    )


def diff_owned(
    project_root: str,
    emission_root: str,
    new_entries: list[dict],
    prev_entries: list[dict] | None,
) -> list[Delta]:
    """Compute the diff between on-disk state and `new_entries`,
    using `prev_entries` to surface orphans."""
    deltas: list[Delta] = []
    new_by_path = {e["path"]: e for e in new_entries}

    for entry in new_entries:
        kind = _kind_for(entry)
        if kind == "dir":
            deltas.append(_classify_dir(project_root, emission_root, entry))
        else:
            deltas.append(_classify_file(project_root, entry))

    if prev_entries:
        for prev in prev_entries:
            if prev["path"] in new_by_path:
                continue
            deltas.append(
                Delta(
                    path=prev["path"],
                    kind=_kind_for(prev),
                    status="orphan",
                    entry=prev,
                )
            )

    return deltas