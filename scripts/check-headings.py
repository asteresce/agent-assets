#!/usr/bin/env python3
import argparse
import json
import os
import re
import sys


def parse_headings(path):
    headings = []
    with open(path, "r", encoding="utf-8") as f:
        for lineno, line in enumerate(f, 1):
            m = re.match(r"^(#{1,6})\s+(.+?)\s*$", line)
            if m:
                level = len(m.group(1))
                text = m.group(2).strip()
                headings.append((level, text, lineno))
    return headings


def section_has_content(lines, heading_lineno):
    # heading_lineno is 1-indexed; content starts on the next line
    for i in range(heading_lineno, len(lines)):
        line = lines[i]
        if re.match(r"^#{1,6}\s+", line):
            break
        if line.strip():
            return True
    return False


def validate_file(path, schema):
    errors = []
    headings = parse_headings(path)

    schema_entries = {
        (entry.get("level", 2), entry["heading"]): entry for entry in schema
    }
    required_keys = [
        (entry.get("level", 2), entry["heading"])
        for entry in schema
        if not entry.get("optional", False)
    ]
    ordered_keys = [(entry.get("level", 2), entry["heading"]) for entry in schema]

    # Every heading must be defined in the schema.
    for level, text, lineno in headings:
        key = (level, text)
        if key not in schema_entries:
            errors.append(f"{path}:{lineno}: unknown heading '{text}' (level {level})")

    present_keys = [(level, text) for level, text, _ in headings]

    # Headings must appear in schema order.
    expected_order = [k for k in ordered_keys if k in present_keys]
    if present_keys != expected_order:
        errors.append(f"{path}: headings are not in schema order")

    # Required headings must be present.
    for key in required_keys:
        if key not in present_keys:
            errors.append(f"{path}: missing required heading '{key[1]}'")

    # Every present section must have content.
    with open(path, "r", encoding="utf-8") as f:
        lines = f.readlines()
    for level, text, lineno in headings:
        if not section_has_content(lines, lineno):
            errors.append(f"{path}:{lineno}: section '{text}' is blank")

    return errors


def validate_category(category, category_dir, headings_schema, fmt):
    errors = []

    if fmt == "directory":
        for entry in os.listdir(category_dir):
            path = os.path.join(category_dir, entry)
            if entry.endswith(".md") and entry != "README.md":
                errors.append(f"{path}: stray Markdown file; directory-format categories must contain only skill directories")
                continue
            if not os.path.isdir(path):
                continue
            main_file = os.path.join(path, "SKILLS.md")
            if not os.path.isfile(main_file):
                errors.append(f"{path}: missing SKILLS.md")
                continue
            errors.extend(validate_file(main_file, headings_schema))
    else:
        for root, _, files in os.walk(category_dir):
            for fname in files:
                if not fname.endswith(".md") or fname == "README.md":
                    continue
                path = os.path.join(root, fname)
                errors.extend(validate_file(path, headings_schema))

    return errors


def main():
    parser = argparse.ArgumentParser(description="Validate Markdown section headings")
    parser.add_argument("--headings", required=True, help="JSON headings schema")
    parser.add_argument("--formats", required=True, help="JSON formats schema")
    parser.add_argument("--dirs", nargs="+", required=True, help="Category directories")
    args = parser.parse_args()

    headings = json.loads(args.headings)
    formats = json.loads(args.formats)
    errors = []

    for category in args.dirs:
        cat_schema = headings.get(category)
        cat_format = formats.get(category, "file")
        if not cat_schema:
            continue
        if not os.path.isdir(category):
            errors.append(f"{category}: directory not found")
            continue
        errors.extend(validate_category(category, category, cat_schema, cat_format))

    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        sys.exit(1)

    print("All headings are valid.")


if __name__ == "__main__":
    main()
