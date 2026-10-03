#!/usr/bin/env python3
"""Compare generated schema tables against the doc tables.

Run by the `schemaTables` flake check. The Nix side renders the
expected tables into a temp file via `lib/gen-schema-tables.nix`; this
script extracts the doc-side tables and diffs them.
"""

from __future__ import annotations

import re
import sys


def extract_tables(text: str) -> dict[str, str]:
    """Return `{title: body}` for each `### Title` block in `text`.

    Stops at any `## Title` heading that closes a category block.
    Bodies are normalised so trailing whitespace and inconsistent
    blank lines don't cause false-positive drift."""
    out: dict[str, str] = {}
    # Split on either `### ` (new table) or `## ` (next-level heading).
    parts = re.split(r"^(?=### |## )", text, flags=re.MULTILINE)
    for part in parts:
        if not part.startswith("### "):
            continue
        title, _, body = part.partition("\n")
        # Trim any later `## ` heading from the body.
        body = re.split(r"^## ", body, flags=re.MULTILINE)[0]
        # Collapse runs of blank lines to a single blank line and strip
        # trailing whitespace.
        body = re.sub(r"\n{3,}", "\n\n", body).strip()
        out[title.strip()] = body
    return out


def main() -> int:
    expected_path, actual_path, doc_path = sys.argv[1], sys.argv[2], sys.argv[3]
    expected = extract_tables(open(expected_path).read())
    doc = extract_tables(open(doc_path).read())
    diffs: list[str] = []
    for title, body in expected.items():
        if title not in doc:
            diffs.append(f"missing in doc: {title}")
            continue
        if doc[title] != body:
            diffs.append(f"drift in: {title}")
            print(f"--- expected for {title} ---", file=sys.stderr)
            print(repr(body), file=sys.stderr)
            print(f"--- actual for {title} ---", file=sys.stderr)
            print(repr(doc[title]), file=sys.stderr)
    if diffs:
        for d in diffs:
            print(d, file=sys.stderr)
        return 1
    print("schema tables match.")
    return 0


if __name__ == "__main__":
    sys.exit(main())