import json
import os
import shutil

from . import hashing, json_merge as jmerge, lock


def copy_file(src, dst):
    os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
    shutil.copyfile(src, dst)


def copy_directory(src, dst):
    if os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def apply_json_injections(project_root, config):
    grouped = jmerge.collect(config)
    if not grouped:
        return []
    reports = []
    for fp, content in grouped.items():
        target = os.path.join(project_root, fp)
        os.makedirs(os.path.dirname(target) or ".", exist_ok=True)
        if os.path.exists(target):
            with open(target, "r", encoding="utf-8") as f:
                try:
                    existing = json.load(f)
                except json.JSONDecodeError:
                    existing = {}
        else:
            existing = {}
        merged = jmerge.deep_merge(existing, content)
        with open(target, "w", encoding="utf-8") as f:
            json.dump(merged, f, indent=2, sort_keys=True, ensure_ascii=False)
            f.write("\n")
        reports.append({"file": fp, "merged": merged})
    return reports


def reconcile_owned(project_root, emission_root, new_entries, prev_entries):
    actions = []
    prev_by_path = {e["path"]: e for e in (prev_entries or [])}
    new_by_path = {e["path"]: e for e in new_entries}

    for path, new_entry in new_by_path.items():
        target = os.path.join(project_root, path)
        source = os.path.join(emission_root, path)
        if not os.path.exists(source):
            continue
        if os.path.isdir(source):
            current_hash = hashing.dir_hash(target, hashing.list_registry_files(source))
            if current_hash == new_entry["hash"]:
                actions.append({"path": path, "action": "skip-dir"})
                continue
            copy_directory(source, target)
            actions.append({"path": path, "action": "recopy-dir"})
        else:
            if not os.path.exists(target):
                copy_file(source, target)
                actions.append({"path": path, "action": "copy"})
                continue
            current = hashing.sha256_file(target)
            if hashing.to_nix_hash(current) == new_entry["hash"]:
                actions.append({"path": path, "action": "skip"})
                continue
            copy_file(source, target)
            actions.append({"path": path, "action": "overwrite"})

    for path, prev_entry in prev_by_path.items():
        if path in new_by_path:
            continue
        target = os.path.join(project_root, path)
        if os.path.isdir(target):
            shutil.rmtree(target)
            actions.append({"path": path, "action": "remove-dir"})
        elif os.path.exists(target):
            os.remove(target)
            actions.append({"path": path, "action": "remove-file"})

    return actions


def sync(project_root, emission_root, config, manifest="./agent-assets.lock"):
    json_reports = apply_json_injections(project_root, config)

    emission_lock_path = os.path.join(emission_root, "agent-assets.lock")
    new_entries = lock.read_lock(emission_lock_path)
    if new_entries is None:
        new_entries = []

    manifest_path = os.path.normpath(os.path.join(project_root, manifest))
    prev_entries = lock.read_lock(manifest_path)

    actions = reconcile_owned(project_root, emission_root, new_entries, prev_entries)

    os.makedirs(os.path.dirname(manifest_path) or ".", exist_ok=True)
    lock.write_lock(manifest_path, new_entries)

    return {
        "json_injections": json_reports,
        "actions": actions,
        "manifest_path": manifest_path,
        "had_prev_lock": prev_entries is not None,
    }