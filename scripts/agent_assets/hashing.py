"""Content hashing utilities.

Hashes are emitted in Nix-style format: `sha256-<unpadded-base32>`.
This format is used both for emission-time integrity and for the lock
file. `parse_hash` is the inverse of `format_hash`; tests should use it
instead of hand-rolling the inverse.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os

from .constants import HASH_PREFIX

CHUNK_SIZE = 65536


def sha256_bytes(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def sha256_file(path: str) -> bytes:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(CHUNK_SIZE), b""):
            h.update(chunk)
    return h.digest()


def format_hash(digest: bytes) -> str:
    """Encode `digest` as `sha256-<unpadded-base32>`."""
    b32 = base64.b32encode(digest).decode("ascii").rstrip("=")
    return f"{HASH_PREFIX}{b32}"


def parse_hash(value: str) -> bytes | None:
    """Decode a hash produced by `format_hash`. Returns `None` on
    malformed input (wrong prefix or non-base32 alphabet)."""
    if not value.startswith(HASH_PREFIX):
        return None
    body = value[len(HASH_PREFIX):]
    pad = "=" * ((-len(body)) % 8)
    try:
        return base64.b32decode(body + pad)
    except (ValueError, TypeError):
        return None


def dir_hash(root: str, registry_files: list[str]) -> str | None:
    """Compute a Merkle-style hash over the named files under `root`.

    Returns `None` if any expected file is missing (signals to callers
    that the directory is in an unexpected state)."""
    entries = []
    for rel_path in sorted(registry_files):
        full = os.path.join(root, rel_path)
        if not os.path.exists(full):
            return None
        digest = sha256_file(full)
        entries.append({"path": rel_path, "hash": format_hash(digest)})
    canonical = json.dumps(
        entries, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return format_hash(sha256_bytes(canonical))


def list_registry_files(skill_source: str) -> list[str]:
    """Return a sorted list of file paths under `skill_source`, relative
    to it. Used for hash inputs (so a hash of two trees is comparable
    iff the same file set is present in both)."""
    result = []
    for dirpath, _dirnames, filenames in os.walk(skill_source):
        for name in sorted(filenames):
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, skill_source)
            result.append(rel)
    return sorted(result)