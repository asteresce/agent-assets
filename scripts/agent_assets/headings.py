"""Heading validation for shared assets.

The schema for each category lives in `lib/schemas/headings.nix`. This
module re-implements the parser and the rules (level default, ordering,
required, non-empty). It is consumed by:

  - `scripts/check-headings.py` (CLI used by `nix flake check`).
  - `scripts/agent_assets/check.py` (so `nix run .#check` validates
    imported-asset headings too).

Single source of truth on the Nix side, single parser on the Python
side. The Nix blob is the schema; the walk rule lives here.
"""

from __future__ import annotations

import os
import re

from .constants import MD_EXT, README_NAME, SKILLS_ENTRY

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
_DEFAULT_LEVEL = 2


def parse_headings(path: str) -> list[tuple[int, str, int]]:
    """Return `[(level, text, line_no_1_indexed), ...]` for `path`."""
    headings = []
    with open(path, "r", encoding="utf-8") as f:
        for lineno, line in enumerate(f, 1):
            m = _HEADING_RE.match(line)
            if m:
                headings.append((len(m.group(1)), m.group(2).strip(), lineno))
    return headings


def _section_has_content(lines: list[str], heading_lineno: int) -> bool:
    """`heading_lineno` is 1-indexed; content starts on the next line."""
    for i in range(heading_lineno, len(lines)):
        line = lines[i]
        if _HEADING_RE.match(line):
            break
        if line.strip():
            return True
    return False


def validate_file(path: str, schema: list[dict]) -> list[str]:
    """Return a list of error messages (empty == OK) for `path`
    against `schema`."""
    errors: list[str] = []
    headings = parse_headings(path)

    schema_entries = {
        (e.get("level", _DEFAULT_LEVEL), e["heading"]): e for e in schema
    }
    required_keys = [
        (e.get("level", _DEFAULT_LEVEL), e["heading"])
        for e in schema
        if not e.get("optional", False)
    ]
    ordered_keys = [
        (e.get("level", _DEFAULT_LEVEL), e["heading"]) for e in schema
    ]

    for level, text, lineno in headings:
        if (level, text) not in schema_entries:
            errors.append(
                f"{path}:{lineno}: unknown heading '{text}' (level {level})"
            )

    present_keys = [(level, text) for level, text, _ in headings]
    expected_order = [k for k in ordered_keys if k in present_keys]
    if present_keys != expected_order:
        errors.append(f"{path}: headings are not in schema order")

    for key in required_keys:
        if key not in present_keys:
            errors.append(
                f"{path}: missing required heading '{key[1]}'"
            )

    with open(path, "r", encoding="utf-8") as f:
        lines = f.readlines()
    for level, text, lineno in headings:
        if not _section_has_content(lines, lineno):
            errors.append(
                f"{path}:{lineno}: section '{text}' is blank"
            )

    return errors


def validate_category(
    category: str,
    category_dir: str,
    headings_schema: list[dict],
    fmt: str,
) -> list[str]:
    """Validate every asset in `category_dir` against `headings_schema`
    and the per-category `fmt` (`file` or `directory`)."""
    errors: list[str] = []
    if not os.path.isdir(category_dir):
        errors.append(f"{category}: directory not found")
        return errors

    if fmt == "directory":
        for entry in os.listdir(category_dir):
            path = os.path.join(category_dir, entry)
            if entry.endswith(MD_EXT) and entry != README_NAME:
                errors.append(
                    f"{path}: stray Markdown file; directory-format "
                    f"categories must contain only skill directories"
                )
                continue
            if not os.path.isdir(path):
                continue
            main_file = os.path.join(path, SKILLS_ENTRY)
            if not os.path.isfile(main_file):
                errors.append(f"{path}: missing {SKILLS_ENTRY}")
                continue
            errors.extend(validate_file(main_file, headings_schema))
    else:
        for root, _, files in os.walk(category_dir):
            for fname in files:
                if not fname.endswith(MD_EXT) or fname == README_NAME:
                    continue
                path = os.path.join(root, fname)
                errors.extend(validate_file(path, headings_schema))

    return errors