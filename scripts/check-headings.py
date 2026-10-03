#!/usr/bin/env python3
"""Thin wrapper to call `python -m agent_assets headings`."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_assets.__main__ import main

if __name__ == "__main__":
    sys.exit(main(["headings", *sys.argv[1:]]))