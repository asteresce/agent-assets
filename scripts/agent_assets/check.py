import json
import os

from . import hashing, json_merge as jmerge, lock


def check_owned(project_root, new_entries, prev_entries):
    reports = []
    prev_by_path = {e["path"]: e for e in (prev_entries or [])}
    for entry in new_entries:
        target = os.path.join(project_root, entry["path"])
        if not os.path.exists(target):
            reports.append({"kind": "drift-missing", "path": entry["path"]})
            continue
        if os.path.isdir(target):
            emission_target = target
            try:
                registry_files = hashing.list_registry_files(emission_target)
            except FileNotFoundError:
                reports.append({"kind": "drift-dir-missing", "path": entry["path"]})
                continue
            current = hashing.dir_hash(target, registry_files)
            if current != entry["hash"]:
                reports.append({
                    "kind": "drift-dir",
                    "path": entry["path"],
                    "expected": entry["hash"],
                    "actual": current,
                })
        else:
            current = hashing.to_nix_hash(hashing.sha256_file(target))
            if current != entry["hash"]:
                reports.append({
                    "kind": "drift",
                    "path": entry["path"],
                    "expected": entry["hash"],
                    "actual": current,
                })

    for path, prev in prev_by_path.items():
        if any(e["path"] == path for e in new_entries):
            continue
        target = os.path.join(project_root, path)
        if os.path.exists(target):
            reports.append({"kind": "orphan", "path": path, "prev_hash": prev.get("hash")})
    return reports


def check_json(project_root, grouped_injections):
    reports = []
    for fp, chunk in grouped_injections.items():
        target = os.path.join(project_root, fp)
        if not os.path.exists(target):
            reports.append({"kind": "json-missing", "file": fp})
            continue
        with open(target, "r", encoding="utf-8") as f:
            try:
                data = json.load(f)
            except json.JSONDecodeError:
                reports.append({"kind": "json-invalid", "file": fp})
                continue
        if not jmerge.chunk_present(data, chunk):
            reports.append({"kind": "json-chunk-missing", "file": fp})
    return reports


def check(project_root, emission_root, config, manifest="./agent-assets.lock"):
    grouped_json = jmerge.collect(config)
    json_reports = check_json(project_root, grouped_json)

    emission_lock = os.path.join(emission_root, "agent-assets.lock")
    new_entries = lock.read_lock(emission_lock) or []

    manifest_path = os.path.normpath(os.path.join(project_root, manifest))
    prev_entries = lock.read_lock(manifest_path)

    owned_reports = check_owned(project_root, new_entries, prev_entries)

    ok = not json_reports and not owned_reports
    return {
        "ok": ok,
        "json_reports": json_reports,
        "owned_reports": owned_reports,
    }