"""Drift / orphan / JSON / heading reports for the consumer's project.

Concerns are split into:
  - `report_drift`: turn `diff.diff_owned` deltas into drift reports.
  - `report_json`: verify JSON injection keys are present.
  - `report_headings`: validate section headings for imported assets.
  - `check`: orchestrate.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

from . import diff as diff_mod, headings as headings_mod, json_merge as jmerge, lock
from .constants import DEFAULT_MANIFEST, EMISSION_LOCK_NAME
from .enums import ReportKind


@dataclass
class Report:
    kind: str
    path: str
    expected: str | None = None
    actual: str | None = None
    prev_hash: str | None = None
    file: str | None = None


def _resolve_manifest(project_root: str, manifest: str) -> str:
    return os.path.normpath(os.path.join(project_root, manifest))


def report_drift(deltas: list[diff_mod.Delta]) -> list[Report]:
    reports: list[Report] = []
    for d in deltas:
        if d.status == "unchanged":
            continue
        if d.status == "orphan":
            reports.append(
                Report(
                    kind=ReportKind.ORPHAN.value,
                    path=d.path,
                    prev_hash=(d.entry or {}).get("hash"),
                )
            )
            continue
        if d.kind == "file":
            if d.status == "missing":
                reports.append(Report(kind=ReportKind.DRIFT_MISSING.value, path=d.path))
            else:
                reports.append(
                    Report(
                        kind=ReportKind.DRIFT.value,
                        path=d.path,
                        expected=d.expected,
                        actual=d.actual,
                    )
                )
        else:
            if d.status == "missing":
                reports.append(Report(kind=ReportKind.DRIFT_DIR_MISSING.value, path=d.path))
            else:
                reports.append(
                    Report(
                        kind=ReportKind.DRIFT_DIR.value,
                        path=d.path,
                        expected=d.expected,
                        actual=d.actual,
                    )
                )
    return reports


def report_json(project_root: str, grouped: dict) -> list[Report]:
    reports: list[Report] = []
    for fp, chunk in grouped.items():
        target = os.path.join(project_root, fp)
        if not os.path.exists(target):
            reports.append(Report(kind=ReportKind.JSON_MISSING.value, path=fp, file=fp))
            continue
        from .fsutil import load_json_target
        data, status = load_json_target(target)
        if status != "ok":
            reports.append(Report(kind=ReportKind.JSON_INVALID.value, path=fp, file=fp))
            continue
        if not jmerge.chunk_present(data, chunk):
            reports.append(
                Report(kind=ReportKind.JSON_CHUNK_MISSING.value, path=fp, file=fp)
            )
    return reports


def report_headings(
    project_root: str,
    new_entries: list[dict],
    schema: dict | None,
) -> list[Report]:
    """Validate section headings of every imported asset whose source
    we still have on disk. Requires the `schema` blob
    `{categories, formats, headings}` from the Nix flake. Returns an
    empty list if no schema is provided (preserves the historical
    "check does not validate headings" behaviour for callers that
    don't pass a schema)."""
    if not schema:
        return []
    formats = schema.get("formats") or {}
    headings_schema = schema.get("headings") or {}
    errors: list[Report] = []
    for entry in new_entries:
        category = entry.get("category")
        fmt = formats.get(category)
        cat_schema = headings_schema.get(category)
        if not fmt or not cat_schema:
            continue
        # Try the imported-asset path first; fall back to source-path
        # if the project-side file is missing.
        candidate = os.path.join(project_root, entry["path"])
        if not os.path.exists(candidate):
            continue
        # Validate only files (not directories). For directory-format
        # categories we validate `SKILLS.md` if present.
        from .constants import MD_EXT, SKILLS_ENTRY
        if os.path.isdir(candidate):
            main = os.path.join(candidate, SKILLS_ENTRY)
            if os.path.isfile(main):
                for err in headings_mod.validate_file(main, cat_schema):
                    errors.append(Report(kind=ReportKind.HEADING.value, path=err))
        elif candidate.endswith(MD_EXT):
            for err in headings_mod.validate_file(candidate, cat_schema):
                errors.append(Report(kind=ReportKind.HEADING.value, path=err))
    return errors


def check(
    project_root: str,
    emission_root: str,
    config: dict,
    manifest: str = DEFAULT_MANIFEST,
    schema: dict | None = None,
) -> dict:
    """Report drift, JSON injection drift, orphans, and (when a
    `schema` is provided) heading errors. Exits non-zero on any
    report."""
    grouped_json = jmerge.collect(config)
    json_reports = report_json(project_root, grouped_json)

    emission_lock_path = os.path.join(emission_root, EMISSION_LOCK_NAME)
    new_entries = lock.read_lock(emission_lock_path) or []

    manifest_path = _resolve_manifest(project_root, manifest)
    prev_entries = lock.read_lock(manifest_path)

    deltas = diff_mod.diff_owned(project_root, emission_root, new_entries, prev_entries)
    drift_reports = report_drift(deltas)

    heading_reports: list[Report] = []
    if schema:
        heading_reports = report_headings(project_root, new_entries, schema)

    ok = not json_reports and not drift_reports and not heading_reports
    return {
        "ok": ok,
        "json_reports": [r.__dict__ for r in json_reports],
        "drift_reports": [r.__dict__ for r in drift_reports],
        "heading_reports": [r.__dict__ for r in heading_reports],
    }