"""Sync the emission tree to the consumer's project.

Concerns are split into:
  - `apply_json_injections`: write our JSON chunks into the project.
  - `reconcile`: walk `Diff`s produced by `diff.diff_owned` and apply
    them (copy, skip, overwrite, recopy, remove).
  - `sync`: orchestrate the above and refresh the lock.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

from . import diff as diff_mod
from . import fsutil, json_merge as jmerge, lock
from .constants import (
    DEFAULT_MANIFEST,
    EMISSION_LOCK_NAME,
)
from .enums import Action
from .fsutil import copy_file, copy_tree, dump_json_file, remove


@dataclass
class JsonInjectionReport:
    file: str
    merged: dict


def _load_json_safely(path: str) -> dict:
    """Backward-compat helper: returns `{}` for missing/invalid JSON
    files (used by `apply_json_injections`)."""
    data, status = fsutil.load_json_target(path)
    return data if status == "ok" and isinstance(data, dict) else {}


def apply_json_injections(project_root: str, config: dict) -> list[JsonInjectionReport]:
    grouped = jmerge.collect(config)
    reports: list[JsonInjectionReport] = []
    for fp, content in grouped.items():
        target = os.path.join(project_root, fp)
        existing = _load_json_safely(target)
        merged = jmerge.deep_merge(existing, content)
        dump_json_file(target, merged)
        reports.append(JsonInjectionReport(file=fp, merged=merged))
    return reports


def _action_for(delta: diff_mod.Delta) -> Action:
    if delta.status == "orphan":
        return Action.REMOVE_DIR if delta.kind == "dir" else Action.REMOVE_FILE
    if delta.kind == "dir":
        return Action.SKIP_DIR if delta.status == "unchanged" else Action.RECOPY_DIR
    if delta.status == "missing":
        return Action.COPY
    if delta.status == "unchanged":
        return Action.SKIP
    return Action.OVERWRITE


def reconcile(
    project_root: str,
    emission_root: str,
    deltas: list[diff_mod.Delta],
) -> list[dict]:
    """Apply each delta to disk, returning a list of action records."""
    actions: list[dict] = []
    for d in deltas:
        action = _action_for(d)
        target = os.path.join(project_root, d.path)
        if action == Action.COPY:
            copy_file(os.path.join(emission_root, d.path), target)
        elif action == Action.OVERWRITE:
            copy_file(os.path.join(emission_root, d.path), target)
        elif action == Action.RECOPY_DIR:
            copy_tree(os.path.join(emission_root, d.path), target)
        elif action == Action.REMOVE_FILE:
            if os.path.exists(target):
                os.remove(target)
        elif action == Action.REMOVE_DIR:
            if os.path.exists(target):
                import shutil
                shutil.rmtree(target)
        # SKIP / SKIP_DIR: nothing to do.
        actions.append({"path": d.path, "action": action.value})
    return actions


def _resolve_manifest(project_root: str, manifest: str) -> str:
    return os.path.normpath(os.path.join(project_root, manifest))


def sync(
    project_root: str,
    emission_root: str,
    config: dict,
    manifest: str = DEFAULT_MANIFEST,
) -> dict:
    """Apply JSON injections, reconcile owned files, and refresh the
    lock."""
    json_reports = apply_json_injections(project_root, config)

    emission_lock_path = os.path.join(emission_root, EMISSION_LOCK_NAME)
    new_entries = lock.read_lock(emission_lock_path) or []

    manifest_path = _resolve_manifest(project_root, manifest)
    prev_entries = lock.read_lock(manifest_path)

    deltas = diff_mod.diff_owned(project_root, emission_root, new_entries, prev_entries)
    actions = reconcile(project_root, emission_root, deltas)

    lock.write_lock(manifest_path, new_entries)

    return {
        "json_injections": [{"file": r.file, "merged": r.merged} for r in json_reports],
        "actions": actions,
        "manifest_path": manifest_path,
        "had_prev_lock": prev_entries is not None,
    }