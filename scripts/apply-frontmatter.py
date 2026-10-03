#!/usr/bin/env python3
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_assets import frontmatter


def main():
    if len(sys.argv) != 4:
        print("usage: apply-frontmatter.py <source> <out> <injections.json>", file=sys.stderr)
        sys.exit(2)
    source_path = sys.argv[1]
    out_path = sys.argv[2]
    injections_path = sys.argv[3]

    with open(source_path, "r", encoding="utf-8") as f:
        source = f.read()

    category_frontmatter = {}
    import_frontmatter = {}
    if os.path.exists(injections_path):
        with open(injections_path, "r", encoding="utf-8") as f:
            injections = json.load(f)
        category_frontmatter = injections.get("category") or {}
        import_frontmatter = injections.get("import") or {}

    rendered = frontmatter.apply(source, category_frontmatter, import_frontmatter)

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(rendered)


if __name__ == "__main__":
    main()