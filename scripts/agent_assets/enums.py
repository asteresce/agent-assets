"""Enum-like constants for action and report kinds.

Replaces the magic strings previously emitted by `sync` / `check`.
Inheriting from `str` keeps the values JSON-serialisable while making
the set of valid kinds explicit and typo-proof.
"""

from __future__ import annotations
from enum import Enum


class FormatKind(str, Enum):
    """Per-category asset format. Mirrors `lib/schemas/formats.nix`."""

    FILE = "file"
    DIRECTORY = "directory"


class Action(str, Enum):
    """Per-entry actions emitted by `sync.reconcile`."""

    COPY = "copy"
    SKIP = "skip"
    OVERWRITE = "overwrite"
    SKIP_DIR = "skip-dir"
    RECOPY_DIR = "recopy-dir"
    REMOVE_FILE = "remove-file"
    REMOVE_DIR = "remove-dir"


class ReportKind(str, Enum):
    """Drift / orphan / json reports emitted by `check.check`."""

    DRIFT = "drift"
    DRIFT_MISSING = "drift-missing"
    DRIFT_DIR = "drift-dir"
    DRIFT_DIR_MISSING = "drift-dir-missing"
    ORPHAN = "orphan"
    JSON_MISSING = "json-missing"
    JSON_INVALID = "json-invalid"
    JSON_CHUNK_MISSING = "json-chunk-missing"
    HEADING = "heading"