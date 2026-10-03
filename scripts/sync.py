#!/usr/bin/env python3
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_assets import sync as sync_mod


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True, help="project root directory")
    parser.add_argument("--emission", required=True, help="emission tree root")
    parser.add_argument("--config", required=True, help="resolved config JSON")
    parser.add_argument("--manifest", default="./agent-assets.lock",
                        help="lock file path, relative to the project root")
    args = parser.parse_args()

    with open(args.config, "r", encoding="utf-8") as f:
        config = json.load(f)

    result = sync_mod.sync(args.project, args.emission, config, manifest=args.manifest)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()