import json
import os


LOCK_VERSION = 5


def entry_for_file(category, name, path, hash_value):
    return {
        "category": category,
        "name": name,
        "path": path,
        "hash": hash_value,
    }


def entry_for_directory(category, name, path, hash_value):
    return {
        "category": category,
        "name": name,
        "path": path,
        "hash": hash_value,
    }


def read_lock(path):
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


def write_lock(path, entries):
    payload = {"version": LOCK_VERSION, "entries": entries}
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, sort_keys=True, ensure_ascii=False)
        f.write("\n")


def paths_from(entries):
    return [e["path"] for e in entries if "path" in e]


def lock_diff(prev, new):
    prev_paths = set(paths_from(prev or []))
    new_paths = set(paths_from(new))
    orphans = sorted(prev_paths - new_paths)
    added = sorted(new_paths - prev_paths)
    common = sorted(prev_paths & new_paths)
    return {"orphans": orphans, "added": added, "common": common}


def lookup_entry(entries, path):
    for e in entries:
        if e.get("path") == path:
            return e
    return None