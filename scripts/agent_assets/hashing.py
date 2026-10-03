import base64
import hashlib
import json
import os


NIX_HASH_PREFIX = "sha256-"


def sha256_bytes(data):
    return hashlib.sha256(data).digest()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.digest()


def to_nix_hash(digest):
    b32 = base64.b32encode(digest).decode("ascii").rstrip("=")
    return f"{NIX_HASH_PREFIX}{b32}"


def dir_hash(root, registry_files):
    entries = []
    for rel_path in sorted(registry_files):
        full = os.path.join(root, rel_path)
        if not os.path.exists(full):
            return None
        digest = sha256_file(full)
        entries.append({
            "path": rel_path,
            "hash": to_nix_hash(digest),
        })
    canonical = json.dumps(entries, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return to_nix_hash(sha256_bytes(canonical))


def list_registry_files(skill_source):
    result = []
    for dirpath, _dirnames, filenames in os.walk(skill_source):
        for name in sorted(filenames):
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, skill_source)
            result.append(rel)
    return sorted(result)