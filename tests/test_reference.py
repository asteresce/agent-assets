"""docs/reference.generated.md must match spec.json."""

import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


class ReferenceTest(unittest.TestCase):
    def test_generated_reference_is_current(self):
        proc = subprocess.run(
            [sys.executable, str(REPO / "scripts" / "gen-reference.py"), "--check"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)


if __name__ == "__main__":
    unittest.main()
