#!/usr/bin/env python3
"""Scenario tests for the agent-assets toolchain.

Each scenario exercises a single behaviour: first-sync, idempotency,
overwrite, orphan removal, JSON injection, drift detection, etc. The
`fixture` context manager yields `(tmp, cfg, emission, project)` after
the standard `build + first sync` preamble so scenarios can focus on
their own assertion.
"""

from __future__ import annotations

import contextlib
import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts"))

from agent_assets import check, constants, overlay, sync
from agent_assets.hashing import format_hash, sha256_file


REPO_ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))


# -- Fixtures -------------------------------------------------------------


def base_config(**overrides):
    """Return a fresh dict matching `fixture_config_a` from earlier
    versions. `overrides` lets scenarios tweak any nested value."""
    cfg = {
        "rules": {
            "root": "./.opencode/rules",
            "imports": ["security", "code-style"],
            "injections": {
                "frontmatter": {"team": "platform"},
                "json": [
                    {
                        "file": "./opencode.json",
                        "content": {
                            "instructions": [".opencode/rules/**/*.md"],
                        },
                    },
                ],
            },
        },
        "skills": {
            "root": "./.opencode/skills",
            "imports": ["migration"],
        },
        "agents": {
            "root": "./.opencode/agents",
            "imports": ["build"],
        },
    }
    for path, value in overrides.items():
        cur = cfg
        keys = path.split(".")
        for k in keys[:-1]:
            cur = cur.setdefault(k, {})
        cur[keys[-1]] = value
    return cfg


def config_without(*keys):
    """Return the base config with the given top-level keys cleared."""
    cfg = base_config()
    for key in keys:
        cfg[key] = {"root": f"./{key}", "imports": []}
    return cfg


def config_only(category, imports, **inj):
    """Return a minimal config with only `category` non-empty.

    Other categories are omitted entirely so `normalize_config` will
    fall back to its default `root`. Useful for tests that want to
    exercise the default-root behaviour."""
    cfg = {c: {"imports": []} for c in ("rules", "skills", "agents") if c != category}
    cfg[category] = {"imports": list(imports)}
    if inj:
        cfg[category]["injections"] = inj
    return cfg


@contextlib.contextmanager
def project(cfg=None):
    """Standard preamble: tmpdir, build emission, first sync."""
    cfg = cfg if cfg is not None else base_config()
    with tempfile.TemporaryDirectory() as td:
        emission = os.path.join(td, "emission")
        project_dir = os.path.join(td, "project")
        os.makedirs(project_dir)
        overlay.build(cfg, REPO_ROOT, emission)
        sync.sync(project_dir, emission, cfg)
        yield td, cfg, emission, project_dir


@contextlib.contextmanager
def project_two_emissions(cfg_a=None, cfg_b=None):
    """Two-emission variant for scenarios that diff two configs.

    Performs build_a, sync_a, build_b, sync_b in that order. The scenario
    receives `(td, cfg_a, cfg_b, emission_a, emission_b, project_dir)`
    and may also re-sync or inspect intermediate state."""
    cfg_a = cfg_a if cfg_a is not None else base_config()
    cfg_b = cfg_b if cfg_b is not None else cfg_a
    with tempfile.TemporaryDirectory() as td:
        emission_a = os.path.join(td, "emission_a")
        emission_b = os.path.join(td, "emission_b")
        project_dir = os.path.join(td, "project")
        os.makedirs(project_dir)
        overlay.build(cfg_a, REPO_ROOT, emission_a)
        sync.sync(project_dir, emission_a, cfg_a)
        overlay.build(cfg_b, REPO_ROOT, emission_b)
        sync.sync(project_dir, emission_b, cfg_b)
        yield td, cfg_a, cfg_b, emission_a, emission_b, project_dir


# -- Assertions -----------------------------------------------------------


def assert_eq(actual, expected, label):
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")


def assert_true(cond, label):
    if not cond:
        raise AssertionError(label)


# -- Scenario registry ----------------------------------------------------


scenarios = []


def scenario(name):
    def deco(fn):
        scenarios.append((name, fn))
        return fn
    return deco


@scenario("01_first_sync_writes_files_and_creates_lock")
def _():
    with project() as (_td, _cfg, _emission, project_dir):
        assert_true(os.path.exists(os.path.join(project_dir, ".opencode/rules/security.md")), "security.md")
        assert_true(os.path.exists(os.path.join(project_dir, ".opencode/skills/migration/SKILLS.md")), "SKILLS.md")
        assert_true(os.path.exists(os.path.join(project_dir, ".opencode/agents/build.md")), "build.md")
        assert_true(os.path.exists(os.path.join(project_dir, constants.DEFAULT_MANIFEST)), "lock")
        assert_true(os.path.exists(os.path.join(project_dir, "opencode.json")), "opencode.json")
        with open(os.path.join(project_dir, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data["instructions"], [".opencode/rules/**/*.md"], "json instructions")


@scenario("02_idempotent_resync_no_writes")
def _():
    with project() as (_td, cfg, emission, project_dir):
        mtime_before = os.path.getmtime(os.path.join(project_dir, ".opencode/rules/security.md"))
        result = sync.sync(project_dir, emission, cfg)
        assert_true(any(a["action"] in ("skip", "skip-dir") for a in result["actions"]), "at least one skip")
        mtime_after = os.path.getmtime(os.path.join(project_dir, ".opencode/rules/security.md"))
        assert_eq(mtime_before, mtime_after, "no mtime change")


@scenario("03_user_edit_overwritten")
def _():
    with project() as (_td, cfg, emission, project_dir):
        target = os.path.join(project_dir, ".opencode/rules/security.md")
        with open(target, "a") as f:
            f.write("\nUSER EDIT\n")
        result = sync.sync(project_dir, emission, cfg)
        actions = [a for a in result["actions"] if a["path"].endswith("security.md")]
        assert_true(any(a["action"] == "overwrite" for a in actions), "overwritten")


@scenario("04_remove_import_deletes_file")
def _():
    cfg_a = base_config()
    cfg_b = base_config()
    cfg_b["rules"]["imports"] = ["code-style"]
    cfg_b["agents"]["imports"] = []
    cfg_b["skills"]["imports"] = []
    with project_two_emissions(cfg_a, cfg_b) as (
        _td, _a, _b, _ea, _eb, project_dir,
    ):
        assert_true(not os.path.exists(os.path.join(project_dir, ".opencode/rules/security.md")), "security removed")


@scenario("05_custom_file_under_managed_root_preserved")
def _():
    with project() as (_td, cfg, emission, project_dir):
        custom = os.path.join(project_dir, ".opencode/rules/custom.md")
        with open(custom, "w") as f:
            f.write("custom")
        sync.sync(project_dir, emission, cfg)
        assert_true(os.path.exists(custom), "custom preserved")


@scenario("06_missing_prior_lock_writes_fresh_no_orphan_deletion")
def _():
    cfg_a = base_config()
    cfg_b = config_without("agents")
    cfg_b["rules"]["imports"] = ["code-style"]
    with tempfile.TemporaryDirectory() as td:
        project_dir = os.path.join(td, "project")
        emission_a = os.path.join(td, "emission_a")
        emission_b = os.path.join(td, "emission_b")
        os.makedirs(project_dir)
        overlay.build(cfg_a, REPO_ROOT, emission_a)
        sync.sync(project_dir, emission_a, cfg_a)
        os.remove(os.path.join(project_dir, constants.DEFAULT_MANIFEST))
        stray = os.path.join(project_dir, ".opencode/rules/security.md")
        assert_true(os.path.exists(stray), "stray from before")
        overlay.build(cfg_b, REPO_ROOT, emission_b)
        result = sync.sync(project_dir, emission_b, cfg_b)
        assert_true(not result["had_prev_lock"], "no prior lock")
        assert_true(os.path.exists(stray), "stray remains (no orphan deletion without prior)")


@scenario("07_remove_skill_deletes_directory")
def _():
    cfg_a = base_config()
    cfg_b = base_config()
    cfg_b["skills"]["imports"] = []
    with project_two_emissions(cfg_a, cfg_b) as (
        _td, _a, _b, _ea, _eb, project_dir,
    ):
        assert_true(not os.path.exists(os.path.join(project_dir, ".opencode/skills/migration")), "skill dir removed")


@scenario("08_two_categories_injecting_same_json")
def _():
    cfg = {
        "rules": {
            "root": "./.opencode/rules",
            "imports": ["security"],
            "injections": {
                "json": [{"file": "./opencode.json", "content": {"a": [1]}}],
            },
        },
        "agents": {
            "root": "./.opencode/agents",
            "imports": [],
            "injections": {
                "json": [{"file": "./opencode.json", "content": {"b": [2]}}],
            },
        },
        "skills": {"root": "./x", "imports": []},
    }
    with project(cfg) as (_td, _cfg, _emission, project_dir):
        with open(os.path.join(project_dir, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("a"), [1], "rules key")
        assert_eq(data.get("b"), [2], "agents key")


@scenario("09_check_clean_state_exits_zero")
def _():
    with project() as (_td, cfg, emission, project_dir):
        result = check.check(project_dir, emission, cfg)
        assert_true(result["ok"], f"check on clean state must be ok: {result}")


@scenario("10_skill_resync_idempotent")
def _():
    with project() as (_td, cfg, emission, project_dir):
        skill_md = os.path.join(project_dir, ".opencode/skills/migration/SKILLS.md")
        mtime_before = os.path.getmtime(skill_md)
        result = sync.sync(project_dir, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "skip-dir" for a in skill_actions), "skill dir skipped")
        mtime_after = os.path.getmtime(skill_md)
        assert_eq(mtime_before, mtime_after, "no mtime change in skill")


@scenario("11_user_adds_file_inside_skill_no_overwrite")
def _():
    with project() as (_td, cfg, emission, project_dir):
        extra = os.path.join(project_dir, ".opencode/skills/migration/user-note.md")
        with open(extra, "w") as f:
            f.write("user note")
        sync.sync(project_dir, emission, cfg)
        assert_true(os.path.exists(extra), "user note preserved")


@scenario("12_user_modifies_skill_artifact_recomposes")
def _():
    with project() as (_td, cfg, emission, project_dir):
        target = os.path.join(project_dir, ".opencode/skills/migration/rollback-checklist.md")
        with open(target, "a") as f:
            f.write("\nUSER EDIT\n")
        result = sync.sync(project_dir, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "recopy-dir" for a in skill_actions), "skill dir recopy")


@scenario("13_user_deletes_skill_artifact_recopies")
def _():
    with project() as (_td, cfg, emission, project_dir):
        os.remove(os.path.join(project_dir, ".opencode/skills/migration/rollback-checklist.md"))
        result = sync.sync(project_dir, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "recopy-dir" for a in skill_actions), "skill dir recopy")


def _json_injection_cfg(content, target="./opencode.json"):
    return {
        "rules": {
            "root": "./.opencode/rules",
            "imports": ["security"],
            "injections": {
                "json": [{"file": target, "content": content}],
            },
        },
        "skills": {"root": "./x", "imports": []},
        "agents": {"root": "./x", "imports": []},
    }


@scenario("14_json_injection_to_nonexistent_file_creates")
def _():
    cfg = _json_injection_cfg({"k": "v"}, target="./fresh.json")
    with project(cfg) as (_td, _cfg, _emission, project_dir):
        assert_true(os.path.exists(os.path.join(project_dir, "fresh.json")), "fresh.json created")


@scenario("15_json_injection_preserves_user_keys")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with tempfile.TemporaryDirectory() as td:
        project_dir = os.path.join(td, "project")
        emission = os.path.join(td, "emission")
        os.makedirs(project_dir)
        with open(os.path.join(project_dir, "opencode.json"), "w") as f:
            json.dump({"user_only": "kept"}, f)
        overlay.build(cfg, REPO_ROOT, emission)
        sync.sync(project_dir, emission, cfg)
        with open(os.path.join(project_dir, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("user_only"), "kept", "user key kept")
        assert_eq(data.get("ours"), 1, "our key present")


@scenario("16_json_injection_idempotent_resync")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with project(cfg) as (_td, _cfg, emission, project_dir):
        before = json.load(open(os.path.join(project_dir, "opencode.json")))
        sync.sync(project_dir, emission, cfg)
        after = json.load(open(os.path.join(project_dir, "opencode.json")))
        assert_eq(before, after, "idempotent")


@scenario("17_user_removes_our_json_key_sync_readds")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with project(cfg) as (_td, _cfg, emission, project_dir):
        with open(os.path.join(project_dir, "opencode.json"), "w") as f:
            json.dump({"something_else": True}, f)
        sync.sync(project_dir, emission, cfg)
        with open(os.path.join(project_dir, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("ours"), 1, "our key restored")


@scenario("18_user_deletes_json_file_sync_recreates")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with project(cfg) as (_td, _cfg, emission, project_dir):
        os.remove(os.path.join(project_dir, "opencode.json"))
        sync.sync(project_dir, emission, cfg)
        assert_true(os.path.exists(os.path.join(project_dir, "opencode.json")), "recreated")


@scenario("19_check_json_file_with_our_keys_exits_zero")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with project(cfg) as (_td, _cfg, emission, project_dir):
        result = check.check(project_dir, emission, cfg)
        assert_true(result["ok"], "check ok")


@scenario("20_check_json_file_with_missing_keys_reports_drift")
def _():
    cfg = _json_injection_cfg({"ours": 1})
    with tempfile.TemporaryDirectory() as td:
        project_dir = os.path.join(td, "project")
        emission = os.path.join(td, "emission")
        os.makedirs(project_dir)
        with open(os.path.join(project_dir, "opencode.json"), "w") as f:
            json.dump({"something_else": True}, f)
        overlay.build(cfg, REPO_ROOT, emission)
        result = check.check(project_dir, emission, cfg)
        assert_true(not result["ok"], "check reports drift")


@scenario("21_self_validation_lock_hash_matches_file")
def _():
    with project() as (_td, _cfg, _emission, project_dir):
        with open(os.path.join(project_dir, constants.DEFAULT_MANIFEST)) as f:
            lock_data = json.load(f)
        for entry in lock_data["entries"]:
            target = os.path.join(project_dir, entry["path"])
            if os.path.isdir(target):
                continue
            assert_eq(
                format_hash(sha256_file(target)),
                entry["hash"],
                f"hash match {entry['path']}",
            )


@scenario("22_idempotent_skill_recopy_no_extra_files_removed")
def _():
    with project() as (_td, cfg, emission, project_dir):
        extra = os.path.join(project_dir, ".opencode/skills/migration/user-note.md")
        with open(extra, "w") as f:
            f.write("user note")
        sync.sync(project_dir, emission, cfg)
        assert_true(os.path.exists(extra), "user-added file in skill dir preserved")


@scenario("23_sync_emission_contains_lock_and_files")
def _():
    cfg = base_config()
    with tempfile.TemporaryDirectory() as td:
        emission = os.path.join(td, "emission")
        overlay.build(cfg, REPO_ROOT, emission)
        assert_true(os.path.exists(os.path.join(emission, constants.EMISSION_LOCK_NAME)), "emission has lock")


@scenario("24_root_default_when_omitted")
def _():
    cfg = config_only("rules", ["security"])
    with project(cfg) as (_td, _cfg, _emission, project_dir):
        assert_true(
            os.path.exists(os.path.join(project_dir, ".agent-assets/rules/security.md")),
            "default root used",
        )


# -- Entry point ----------------------------------------------------------


def main():
    failures = []
    for name, fn in scenarios:
        try:
            fn()
            print(f"PASS {name}")
        except Exception as e:
            failures.append((name, e))
            print(f"FAIL {name}: {e}", file=sys.stderr)
    if failures:
        print(f"\n{len(failures)} failure(s)", file=sys.stderr)
        sys.exit(1)
    print(f"\nAll {len(scenarios)} scenarios passed")


if __name__ == "__main__":
    main()