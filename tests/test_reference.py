"""Generated documentation must match spec.json, and live only in docs/."""

import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


class ReferenceTest(unittest.TestCase):
    def test_generated_docs_are_current(self):
        proc = subprocess.run(
            [sys.executable, str(REPO / "scripts" / "gen-docs.py"), "--check"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)

    def test_generated_docs_are_whole_files_under_docs(self):
        generated = sorted(
            p.relative_to(REPO).as_posix()
            for p in REPO.rglob("*.generated.md")
            if "__pycache__" not in p.parts
        )
        self.assertTrue(generated)
        for name in generated:
            self.assertTrue(
                name.startswith("docs/"), "%s must live under docs/" % name
            )
            self.assertLess(
                len((REPO / name).read_text().splitlines()),
                60,
                "%s must stay light" % name,
            )


if __name__ == "__main__":
    unittest.main()
