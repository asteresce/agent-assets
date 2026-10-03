import json
import os


LOCK_VERSION = 5


def entry(category, name, path, hash_value):
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