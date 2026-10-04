"""Registry conformance: assets follow the section schema in spec.json.

Repo-side only. Consumer `check` deliberately does not validate headings.
"""

import json
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SPEC = json.loads((REPO / "spec.json").read_text())
DEFAULTS = SPEC["defaults"]


def assets(category):
    """Every importable asset of a category: files, or a directory's entry file.

    Catalog indexes (`README.md`) are not assets, and artifacts inside a
    directory asset are free-form.
    """
    root = REPO / DEFAULTS["src"] / category
    for entry in sorted(root.iterdir()):
        if entry.is_dir():
            yield entry / DEFAULTS["skillEntry"]
        elif entry.name != "README.md":
            yield entry


def split_sections(text):
    """Return [(heading, body)] for each `##` section, in order."""
    parts = re.split(r"^## (.+)$", text, flags=re.MULTILINE)
    return [(parts[i], parts[i + 1]) for i in range(1, len(parts), 2)]


def has_content(body):
    for line in body.splitlines():
        if line.strip() and not line.lstrip().startswith("#"):
            return True
    return False


class RegistryTest(unittest.TestCase):
    def test_assets_match_the_section_schema(self):
        for category, expected in SPEC["headings"].items():
            for asset in assets(category):
                with self.subTest(category=category, asset=asset.name):
                    self.assertTrue(
                        asset.is_file(),
                        "%s: missing entry file %s" % (category, asset),
                    )
                    text = asset.read_text()
                    self.assertEqual(
                        re.findall(r"^# (.+)$", text, flags=re.MULTILINE),
                        [],
                        "%s must not carry an h1 title" % asset,
                    )
                    sections = split_sections(text)
                    self.assertEqual(
                        [h for h, _ in sections],
                        expected,
                        "%s: sections must match spec.json in order" % asset,
                    )
                    for heading, body in sections:
                        self.assertTrue(
                            has_content(body),
                            "%s: section %r is empty" % (asset, heading),
                        )

    def test_every_schema_category_exists(self):
        for category in SPEC["headings"]:
            self.assertTrue(
                (REPO / DEFAULTS["src"] / category).is_dir(), category
            )


if __name__ == "__main__":
    unittest.main()
