"""Build the emission tree for a resolved config.

Decomposition (each step is pure unless noted):

  1. `plan(config, src_dir)`       — list of (category, imp, source_path, target_path).
  2. `render(plan_step, injections)` — produce emitted bytes/text per asset.
  3. `materialize(plan, rendered)` — write files to disk.
  4. `hash_written(plan, out_dir)` — compute per-asset hashes.
  5. `write_lock(out_dir, hashes)` — write the emission lock.

`build(config, src_dir, out_dir)` orchestrates these five.
"""

from __future__ import annotations

import os
import shutil
from dataclasses import dataclass

from . import frontmatter, hashing, lock
from .constants import (
    EMISSION_LOCK_NAME,
    MD_EXT,
    SKILLS_ENTRY,
)
from .enums import FormatKind
from .fsutil import copy_tree, write_text
from .normalize import normalize_config, normalize_import

# Per-category format. Mirrors `lib/schemas/formats.nix`. Passed in by
# the caller (derived from the schema blob) instead of being hardcoded.
FORMATS: dict[str, FormatKind] = {
    "rules": FormatKind.FILE,
    "skills": FormatKind.DIRECTORY,
    "agents": FormatKind.FILE,
}


@dataclass
class PlannedAsset:
    category: str
    name: str
    imp: dict
    source_path: str
    target_path: str  # absolute, inside out_dir
    rel_target: str   # project-relative
    fmt: FormatKind


def _resolve_target_path(category: str, imp: dict, root: str) -> str:
    rename = imp.get("rename") or imp["name"]
    destination = imp.get("destination") or ""
    rel_dir = os.path.join(root, destination) if destination else root
    if FORMATS[category] == FormatKind.FILE:
        return os.path.join(rel_dir, f"{rename}{MD_EXT}")
    return os.path.join(rel_dir, imp["name"])


def plan(config: dict, src_dir: str, out_dir: str) -> list[PlannedAsset]:
    """Produce a list of `PlannedAsset` from the resolved config.

    Source paths come from the consumer's `name` (we trust the registry
    here — `assets.nix` is the same logic, in Nix form)."""
    config = normalize_config(config)
    planned: list[PlannedAsset] = []
    for category in FORMATS:
        cat_cfg = config.get(category)
        if not cat_cfg:
            continue
        root = cat_cfg["root"]
        for imp in cat_cfg["imports"]:
            imp = normalize_import(imp)
            name = imp["name"]
            fmt = FORMATS[category]
            if fmt == FormatKind.FILE:
                source = os.path.join(src_dir, category, f"{name}{MD_EXT}")
            else:
                source = os.path.join(src_dir, category, name)
            if not os.path.exists(source):
                kind = "asset" if fmt == FormatKind.FILE else "skill"
                raise FileNotFoundError(f"{kind} not found: {source}")
            rel_target = _resolve_target_path(category, imp, root)
            target = os.path.normpath(os.path.join(out_dir, rel_target))
            planned.append(
                PlannedAsset(
                    category=category,
                    name=name,
                    imp=imp,
                    source_path=source,
                    target_path=target,
                    rel_target=rel_target,
                    fmt=fmt,
                )
            )
    return planned


def render(asset: PlannedAsset, category_injections: dict) -> str:
    """Render the source file content for `asset`. Returns the text
    that should be written to disk (frontmatter already merged).

    Directory-format assets get their `SKILLS.md` rendered with
    frontmatter; non-`SKILLS.md` artifacts are copied verbatim in
    `materialize` (no rendering pass)."""
    with open(asset.source_path, "r", encoding="utf-8") as f:
        source_text = f.read()

    if asset.fmt != FormatKind.DIRECTORY:
        import_inj = asset.imp.get("injections") or {}
        return frontmatter.apply(
            source_text,
            category_injections.get("frontmatter"),
            import_inj.get("frontmatter"),
        )

    # Directory-format: source_path is the skill directory. The
    # `SKILLS.md` inside is what gets frontmatter. Other files are
    # passed through in `materialize`.
    return source_text


def materialize(asset: PlannedAsset, rendered_skill_text: str | None = None) -> None:
    """Write `asset` to its `target_path`.

    For file assets, `rendered_skill_text` is unused (callers should
    use the rendered text directly). For directory assets, the skill
    directory is copied verbatim and `SKILLS.md` is replaced with
    `rendered_skill_text`."""
    if asset.fmt == FormatKind.FILE:
        if rendered_skill_text is None:
            with open(asset.source_path, "r", encoding="utf-8") as f:
                text = f.read()
        else:
            text = rendered_skill_text
        write_text(asset.target_path, text)
        return

    copy_tree(asset.source_path, asset.target_path)
    if rendered_skill_text is not None:
        skills_md = os.path.join(asset.target_path, SKILLS_ENTRY)
        write_text(skills_md, rendered_skill_text)


def hash_written(asset: PlannedAsset, out_dir: str) -> str:
    """Compute the lock entry hash for a written asset."""
    if asset.fmt == FormatKind.FILE:
        return hashing.format_hash(hashing.sha256_file(asset.target_path))
    registry_files = hashing.list_registry_files(asset.target_path)
    return hashing.dir_hash(asset.target_path, registry_files) or ""


def build(config: dict, src_dir: str, out_dir: str) -> dict:
    """Orchestrate plan → render → materialize → hash → write_lock."""
    os.makedirs(out_dir, exist_ok=True)
    planned = plan(config, src_dir, out_dir)
    entries: list[dict] = []

    for asset in planned:
        cat_inj = config.get(asset.category, {}).get("injections") or {}

        if asset.fmt == FormatKind.FILE:
            rendered = render(asset, cat_inj)
            materialize(asset, rendered)
        else:
            skills_md_src = os.path.join(asset.source_path, SKILLS_ENTRY)
            import_inj = asset.imp.get("injections") or {}
            if os.path.exists(skills_md_src):
                with open(skills_md_src, "r", encoding="utf-8") as f:
                    rendered = frontmatter.apply(
                        f.read(),
                        cat_inj.get("frontmatter"),
                        import_inj.get("frontmatter"),
                    )
            else:
                rendered = ""
            materialize(asset, rendered)

        hash_value = hash_written(asset, out_dir)
        entries.append(
            lock.entry(
                category=asset.category,
                name=asset.name,
                path=asset.rel_target,
                hash=hash_value,
            )
        )

    lock_path = os.path.join(out_dir, EMISSION_LOCK_NAME)
    lock.write_lock(lock_path, entries)
    return {"entries": entries, "lock_path": lock_path}