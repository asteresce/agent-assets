"""Registry conformance: assets follow the section schema in spec.json.

Repo-side only. Consumer `check` deliberately does not validate headings.

The schema for a category is an ordered list of `{"name", "required"}`
entries. An asset conforms when its `##` sections are a subsequence of that
list, in that order, with every required entry present. Fenced code blocks are
opaque, so examples may contain headings of their own.
"""

import json
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SPEC = json.loads((REPO / "spec.json").read_text())
DEFAULTS = SPEC["defaults"]

NAME = re.compile(r"^[A-Z][A-Za-z ]*[A-Za-z]$")
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})")
# A closing fence carries no info string (CommonMark); an opener may.
CLOSE = re.compile(r"^ {0,3}(`{3,}|~{3,})\s*$")
H1 = re.compile(r"^#( |$)")
H2 = re.compile(r"^## (.+?)\s*$")


def assets(category):
    """Every importable asset of a category: files, or a directory's entry file.

    Catalog indexes (`README.md`) are not assets, and artifacts inside a
    directory asset are free-form.
    """
    root = REPO / DEFAULTS["assets"] / category
    for entry in sorted(root.iterdir()):
        if entry.is_dir():
            yield entry / DEFAULTS["skillEntry"]
        elif entry.name != "README.md":
            yield entry


def scan(text):
    """Yield (kind, line) with kind in {"h1", "h2", "other"}.

    Anything inside a fenced code block is "other".
    """
    fence = None
    for line in text.splitlines():
        if fence is not None:
            close = CLOSE.match(line)
            if close and close.group(1)[0] == fence[0] and len(close.group(1)) >= fence[1]:
                fence = None
            yield "other", line
            continue
        open = FENCE.match(line)
        if open:
            marker = open.group(1)
            fence = (marker[0], len(marker))
            yield "other", line
        elif H2.match(line):
            yield "h2", line
        elif H1.match(line):
            yield "h1", line
        else:
            yield "other", line


def parse(text):
    """Return (preamble, [(heading, body)], [h1 lines])."""
    preamble, sections, h1s = [], [], []
    for kind, line in scan(text):
        if kind == "h2":
            sections.append((H2.match(line).group(1).strip(), []))
            continue
        if kind == "h1":
            h1s.append(line)
        (sections[-1][1] if sections else preamble).append(line)
    return (
        "\n".join(preamble),
        [(heading, "\n".join(body)) for heading, body in sections],
        h1s,
    )


def has_content(body):
    for line in body.splitlines():
        if line.strip() and not line.lstrip().startswith("#"):
            return True
    return False


def problems(text, schema):
    """Every way `text` departs from `schema`; empty when it conforms."""
    canon = [entry["name"] for entry in schema]
    required = [entry["name"] for entry in schema if entry.get("required")]
    preamble, sections, h1s = parse(text)
    names = [heading for heading, _ in sections]
    found = []

    if preamble.strip():
        found.append(
            "content before the first `##` section "
            "(frontmatter and titles are injected by consumers)"
        )
    for line in h1s:
        found.append("h1 title %r (assets carry no title)" % line)
    for name in dict.fromkeys(names):
        if name not in canon:
            found.append("unknown section %r" % name)
        if names.count(name) > 1:
            found.append("duplicate section %r" % name)
    for name in required:
        if name not in names:
            found.append("missing required section %r" % name)
    present = list(dict.fromkeys(name for name in names if name in canon))
    if present != [name for name in canon if name in present]:
        found.append("sections out of order: %s" % ", ".join(present))
    for heading, body in sections:
        if not has_content(body):
            found.append("empty section %r" % heading)
    return found


class SchemaTest(unittest.TestCase):
    """The schema in spec.json is itself well formed."""

    def test_every_schema_category_exists(self):
        for category in SPEC["headings"]:
            self.assertTrue(
                (REPO / DEFAULTS["assets"] / category).is_dir(), category
            )

    def test_every_registry_category_has_a_schema(self):
        for entry in sorted((REPO / DEFAULTS["assets"]).iterdir()):
            if entry.is_dir():
                self.assertIn(
                    entry.name,
                    SPEC["headings"],
                    "%s has no section schema in spec.json" % entry.name,
                )

    def test_schema_entries_are_well_formed(self):
        for category, schema in SPEC["headings"].items():
            with self.subTest(category=category):
                names = [entry["name"] for entry in schema]
                self.assertEqual(len(names), len(set(names)), "duplicate names")
                for entry in schema:
                    self.assertLessEqual(
                        set(entry), {"name", "required"}, "unknown key in %r" % entry
                    )
                    self.assertRegex(entry["name"], NAME)
                    self.assertIsInstance(entry.get("required", False), bool)
                self.assertTrue(
                    schema[0].get("required"),
                    "first section must be required: it feeds the catalog excerpt",
                )


class RegistryTest(unittest.TestCase):
    def test_assets_match_the_section_schema(self):
        for category, schema in SPEC["headings"].items():
            for asset in assets(category):
                with self.subTest(category=category, asset=asset.name):
                    self.assertTrue(
                        asset.is_file(),
                        "%s: missing entry file %s" % (category, asset),
                    )
                    self.assertEqual(problems(asset.read_text(), schema), [], asset)


class LinterTest(unittest.TestCase):
    """`problems` itself, against a small synthetic schema."""

    SCHEMA = [
        {"name": "Summary", "required": True},
        {"name": "Scope"},
        {"name": "Guidelines", "required": True},
        {"name": "Examples"},
    ]

    def check(self, text):
        return problems(text, self.SCHEMA)

    def test_minimal_asset_conforms(self):
        self.assertEqual(self.check("## Summary\n\nA.\n\n## Guidelines\n\n- B.\n"), [])

    def test_optional_sections_may_appear_in_order(self):
        text = "## Summary\nA\n## Scope\nB\n## Guidelines\nC\n## Examples\nD\n"
        self.assertEqual(self.check(text), [])

    def test_missing_required(self):
        self.assertEqual(
            self.check("## Summary\nA\n"),
            ["missing required section 'Guidelines'"],
        )

    def test_unknown_section(self):
        found = self.check("## Summary\nA\n## Guidelines\nB\n## Notes\nC\n")
        self.assertEqual(found, ["unknown section 'Notes'"])

    def test_out_of_order(self):
        found = self.check("## Guidelines\nB\n## Summary\nA\n")
        self.assertEqual(found, ["sections out of order: Guidelines, Summary"])

    def test_optional_out_of_order(self):
        found = self.check("## Summary\nA\n## Examples\nD\n## Scope\nB\n## Guidelines\nC\n")
        self.assertEqual(len(found), 1)
        self.assertIn("out of order", found[0])

    def test_duplicate_section(self):
        found = self.check("## Summary\nA\n## Summary\nB\n## Guidelines\nC\n")
        self.assertEqual(found, ["duplicate section 'Summary'"])

    def test_heading_names_are_case_sensitive(self):
        found = self.check("## summary\nA\n## Guidelines\nB\n")
        self.assertIn("unknown section 'summary'", found)
        self.assertIn("missing required section 'Summary'", found)

    def test_extra_space_after_the_marker_is_tolerated(self):
        self.assertEqual(self.check("##  Summary\nA\n##   Guidelines\nB\n"), [])

    def test_double_spaced_h1_is_still_an_h1(self):
        found = self.check("#  Title\n## Summary\nA\n## Guidelines\nB\n")
        self.assertIn("h1 title '#  Title' (assets carry no title)", found)

    def test_a_fenced_info_string_does_not_close_a_block(self):
        text = "## Summary\nA\n## Guidelines\n```\n```bash\n## Fake\n```\n"
        self.assertEqual(self.check(text), [])

    def test_empty_section(self):
        found = self.check("## Summary\n\n## Guidelines\n- B\n")
        self.assertEqual(found, ["empty section 'Summary'"])

    def test_subheadings_do_not_count_as_content(self):
        found = self.check("## Summary\n### Only a subheading\n## Guidelines\n- B\n")
        self.assertEqual(found, ["empty section 'Summary'"])

    def test_subheadings_are_free_inside_a_section(self):
        text = "## Summary\nA\n## Guidelines\n### Naming\n- B\n### Layout\n- C\n"
        self.assertEqual(self.check(text), [])

    def test_h1_title_is_rejected(self):
        found = self.check("# Title\n## Summary\nA\n## Guidelines\nB\n")
        self.assertEqual(
            [f for f in found if "h1" in f],
            ["h1 title '# Title' (assets carry no title)"],
        )

    def test_content_before_first_section_is_rejected(self):
        found = self.check("---\nname: x\n---\n## Summary\nA\n## Guidelines\nB\n")
        self.assertEqual(len(found), 1)
        self.assertIn("before the first", found[0])

    def test_headings_inside_fences_are_content(self):
        text = (
            "## Summary\nA\n"
            "## Guidelines\n"
            "Write headings like this:\n\n"
            "```markdown\n# Title\n## Notes\n```\n"
            "## Examples\n"
            "~~~\n## Another\n~~~\n"
        )
        self.assertEqual(self.check(text), [])

    def test_a_longer_fence_is_not_closed_by_a_shorter_one(self):
        text = "## Summary\nA\n## Guidelines\n````\n```\n## Notes\n````\n"
        self.assertEqual(self.check(text), [])

    def test_unclosed_fence_swallows_the_rest(self):
        found = self.check("## Summary\nA\n## Guidelines\n```\n## Examples\nD\n")
        self.assertEqual(found, [])


if __name__ == "__main__":
    unittest.main()
