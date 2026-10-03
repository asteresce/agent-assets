#!/usr/bin/env python3
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_assets import overlay


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True, help="path to resolved config JSON")
    parser.add_argument("--src", required=True, help="path to the agent-assets source tree")
    parser.add_argument("--out", required=True, help="output directory")
    parser.add_argument("--manifest", help="optional manifest path (for verification)")
    args = parser.parse_args()

    with open(args.config, "r", encoding="utf-8") as f:
        config = json.load(f)

    result = overlay.build(config, registry=None, src_dir=args.src, out_dir=args.out)

    print(json.dumps({
        "entries": result["entries"],
        "lock_path": result["lock_path"],
    }))


if __name__ == "__main__":
    main()