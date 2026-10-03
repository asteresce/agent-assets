"""Single CLI entry point for the agent-assets toolchain.

Subcommands:
  - `build`:    render the emission tree from a config + src dir.
  - `sync`:     apply the emission tree to the project.
  - `check`:    report drift, JSON injection drift, and heading errors.
  - `headings`: validate section headings in shared assets.
  - `version`:  print the lock-file schema version.

Replaces the previous per-subcommand scripts in `scripts/`.
"""

from __future__ import annotations

import argparse
import json
import os
import sys

from . import check as check_mod, constants, overlay, sync as sync_mod
from .headings import validate_category


def _load_config(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def cmd_build(args: argparse.Namespace) -> int:
    config = _load_config(args.config)
    result = overlay.build(config, src_dir=args.src, out_dir=args.out)
    print(json.dumps({"entries": result["entries"], "lock_path": result["lock_path"]}))
    return 0


def cmd_sync(args: argparse.Namespace) -> int:
    config = _load_config(args.config)
    result = sync_mod.sync(args.project, args.emission, config, manifest=args.manifest)
    print(json.dumps(result, indent=2))
    return 0


def cmd_check(args: argparse.Namespace) -> int:
    config = _load_config(args.config)
    schema = _load_schema(args.schema) if args.schema else None
    result = check_mod.check(
        args.project, args.emission, config, manifest=args.manifest, schema=schema
    )
    print(json.dumps(result, indent=2))
    return 0 if result["ok"] else 1


def _load_schema(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def cmd_headings(args: argparse.Namespace) -> int:
    with open(args.schema, "r", encoding="utf-8") as f:
        schema = json.load(f)
    categories = schema["categories"]
    formats = schema["formats"]
    headings = schema["headings"]
    errors: list[str] = []
    for category in categories:
        cat_schema = headings.get(category)
        cat_format = formats.get(category, "file")
        if not cat_schema:
            continue
        errors.extend(
            validate_category(category, category, cat_schema, cat_format)
        )
    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1
    print("All headings are valid.")
    return 0


def cmd_version(args: argparse.Namespace) -> int:
    print(f"agent-assets {constants.LOCK_VERSION}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="agent-assets")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("build", help="render the emission tree")
    p.add_argument("--config", required=True)
    p.add_argument("--src", required=True)
    p.add_argument("--out", required=True)
    p.set_defaults(func=cmd_build)

    p = sub.add_parser("sync", help="apply the emission tree to the project")
    p.add_argument("--project", required=True)
    p.add_argument("--emission", required=True)
    p.add_argument("--config", required=True)
    p.add_argument("--manifest", default=constants.DEFAULT_MANIFEST)
    p.set_defaults(func=cmd_sync)

    p = sub.add_parser("check", help="report drift, JSON injection drift, heading errors")
    p.add_argument("--project", required=True)
    p.add_argument("--emission", required=True)
    p.add_argument("--config", required=True)
    p.add_argument("--manifest", default=constants.DEFAULT_MANIFEST)
    p.add_argument("--schema", help="path to the schema JSON from Nix")
    p.set_defaults(func=cmd_check)

    p = sub.add_parser("headings", help="validate section headings in shared assets")
    p.add_argument("--schema", required=True)
    p.set_defaults(func=cmd_headings)

    p = sub.add_parser("version", help="print the lock-file schema version")
    p.set_defaults(func=cmd_version)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())