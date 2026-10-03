#!/usr/bin/env python3
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_assets import json_merge


def main():
    if len(sys.argv) != 3:
        print("usage: apply-json.py <entries.json> <out_dir>", file=sys.stderr)
        sys.exit(2)
    entries_path = sys.argv[1]
    out_dir = sys.argv[2]

    with open(entries_path, "r", encoding="utf-8") as f:
        entries = json.load(f)

    grouped = {}
    for entry in entries:
        file_path = entry["file"]
        content = entry.get("content") or {}
        if file_path in grouped:
            grouped[file_path] = json_merge.deep_merge(grouped[file_path], content)
        else:
            grouped[file_path] = content

    os.makedirs(out_dir, exist_ok=True)
    for file_path, content in grouped.items():
        target = os.path.join(out_dir, file_path)
        os.makedirs(os.path.dirname(target) or ".", exist_ok=True)
        with open(target, "w", encoding="utf-8") as f:
            json.dump(content, f, indent=2, sort_keys=True, ensure_ascii=False)
            f.write("\n")


if __name__ == "__main__":
    main()