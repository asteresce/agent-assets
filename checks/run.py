#!/usr/bin/env python3
import hashlib
import json
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts"))

from agent_assets import check, overlay, sync


REPO_ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))


def fixture_config_a():
    return {
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


def fixture_config_b():
    cfg = fixture_config_a()
    cfg["rules"]["imports"] = ["code-style"]
    cfg["agents"]["imports"] = []
    cfg["skills"]["imports"] = []
    return cfg


def build_emission(cfg, src_dir, out_dir):
    overlay.build(cfg, src_dir=src_dir, out_dir=out_dir)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def assert_eq(actual, expected, label):
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")


def assert_true(cond, label):
    if not cond:
        raise AssertionError(label)


def scenario(name):
    def deco(fn):
        scenarios.append((name, fn))
        return fn
    return deco


scenarios = []


@scenario("01_first_sync_writes_files_and_creates_lock")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        result = sync.sync(project, emission, cfg)
        assert_true(os.path.exists(os.path.join(project, ".opencode/rules/security.md")), "security.md")
        assert_true(os.path.exists(os.path.join(project, ".opencode/skills/migration/SKILLS.md")), "SKILLS.md")
        assert_true(os.path.exists(os.path.join(project, ".opencode/agents/build.md")), "build.md")
        assert_true(os.path.exists(os.path.join(project, "agent-assets.lock")), "lock")
        assert_true(os.path.exists(os.path.join(project, "opencode.json")), "opencode.json")
        with open(os.path.join(project, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data["instructions"], [".opencode/rules/**/*.md"], "json instructions")
        return result


@scenario("02_idempotent_resync_no_writes")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        mtime_before = os.path.getmtime(os.path.join(project, ".opencode/rules/security.md"))
        result = sync.sync(project, emission, cfg)
        skipped = [a for a in result["actions"] if a["action"].startswith("skip")]
        assert_true(len(skipped) >= 1, "at least one skip")
        mtime_after = os.path.getmtime(os.path.join(project, ".opencode/rules/security.md"))
        assert_eq(mtime_before, mtime_after, "no mtime change")


@scenario("03_user_edit_overwritten")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        target = os.path.join(project, ".opencode/rules/security.md")
        with open(target, "a") as f:
            f.write("\nUSER EDIT\n")
        result = sync.sync(project, emission, cfg)
        actions = [a for a in result["actions"] if a["path"].endswith("security.md")]
        assert_true(any(a["action"] == "overwrite" for a in actions), "overwritten")


@scenario("04_remove_import_deletes_file")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg_a = fixture_config_a()
        cfg_b = fixture_config_b()
        emission_a = os.path.join(td, "emission_a")
        emission_b = os.path.join(td, "emission_b")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg_a, REPO_ROOT, emission_a)
        sync.sync(project, emission_a, cfg_a)
        assert_true(os.path.exists(os.path.join(project, ".opencode/rules/security.md")), "security present")
        build_emission(cfg_b, REPO_ROOT, emission_b)
        sync.sync(project, emission_b, cfg_b)
        assert_true(not os.path.exists(os.path.join(project, ".opencode/rules/security.md")), "security removed")


@scenario("05_custom_file_under_managed_root_preserved")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        custom = os.path.join(project, ".opencode/rules/custom.md")
        with open(custom, "w") as f:
            f.write("custom")
        sync.sync(project, emission, cfg)
        assert_true(os.path.exists(custom), "custom preserved")


@scenario("06_missing_prior_lock_writes_fresh_no_orphan_deletion")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg_a = fixture_config_a()
        cfg_b = fixture_config_b()
        emission_a = os.path.join(td, "emission_a")
        emission_b = os.path.join(td, "emission_b")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg_a, REPO_ROOT, emission_a)
        sync.sync(project, emission_a, cfg_a)
        os.remove(os.path.join(project, "agent-assets.lock"))
        stray = os.path.join(project, ".opencode/rules/security.md")
        assert_true(os.path.exists(stray), "stray from before")
        build_emission(cfg_b, REPO_ROOT, emission_b)
        result = sync.sync(project, emission_b, cfg_b)
        assert_true(not result["had_prev_lock"], "no prior lock")
        assert_true(os.path.exists(stray), "stray remains (no orphan deletion without prior)")


@scenario("07_remove_skill_deletes_directory")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg_a = fixture_config_a()
        cfg_b = fixture_config_a()
        cfg_b["skills"]["imports"] = []
        emission_a = os.path.join(td, "emission_a")
        emission_b = os.path.join(td, "emission_b")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg_a, REPO_ROOT, emission_a)
        sync.sync(project, emission_a, cfg_a)
        build_emission(cfg_b, REPO_ROOT, emission_b)
        sync.sync(project, emission_b, cfg_b)
        assert_true(not os.path.exists(os.path.join(project, ".opencode/skills/migration")), "skill dir removed")


@scenario("08_two_categories_injecting_same_json")
def _():
    with tempfile.TemporaryDirectory() as td:
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
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        with open(os.path.join(project, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("a"), [1], "rules key")
        assert_eq(data.get("b"), [2], "agents key")


@scenario("09_check_clean_state_exits_zero")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        result = check.check(project, emission, cfg)
        assert_true(result["ok"], f"check on clean state must be ok: {result}")


@scenario("10_skill_resync_idempotent")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        skill_md = os.path.join(project, ".opencode/skills/migration/SKILLS.md")
        mtime_before = os.path.getmtime(skill_md)
        result = sync.sync(project, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "skip-dir" for a in skill_actions), "skill dir skipped")
        mtime_after = os.path.getmtime(skill_md)
        assert_eq(mtime_before, mtime_after, "no mtime change in skill")


@scenario("11_user_adds_file_inside_skill_no_overwrite")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        extra = os.path.join(project, ".opencode/skills/migration/user-note.md")
        with open(extra, "w") as f:
            f.write("user note")
        sync.sync(project, emission, cfg)
        assert_true(os.path.exists(extra), "user note preserved")


@scenario("12_user_modifies_skill_artifact_recomposes")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        target = os.path.join(project, ".opencode/skills/migration/rollback-checklist.md")
        with open(target, "a") as f:
            f.write("\nUSER EDIT\n")
        result = sync.sync(project, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "recopy-dir" for a in skill_actions), "skill dir recopy")


@scenario("13_user_deletes_skill_artifact_recopies")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        os.remove(os.path.join(project, ".opencode/skills/migration/rollback-checklist.md"))
        result = sync.sync(project, emission, cfg)
        skill_actions = [a for a in result["actions"] if a["path"].endswith("migration")]
        assert_true(any(a["action"] == "recopy-dir" for a in skill_actions), "skill dir recopy")


@scenario("14_json_injection_to_nonexistent_file_creates")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./fresh.json", "content": {"k": "v"}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        assert_true(os.path.exists(os.path.join(project, "fresh.json")), "fresh.json created")


@scenario("15_json_injection_preserves_user_keys")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        with open(os.path.join(project, "opencode.json"), "w") as f:
            json.dump({"user_only": "kept"}, f)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        with open(os.path.join(project, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("user_only"), "kept", "user key kept")
        assert_eq(data.get("ours"), 1, "our key present")


@scenario("16_json_injection_idempotent_resync")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        before = json.load(open(os.path.join(project, "opencode.json")))
        sync.sync(project, emission, cfg)
        after = json.load(open(os.path.join(project, "opencode.json")))
        assert_eq(before, after, "idempotent")


@scenario("17_user_removes_our_json_key_sync_readds")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        with open(os.path.join(project, "opencode.json"), "w") as f:
            json.dump({"something_else": True}, f)
        sync.sync(project, emission, cfg)
        with open(os.path.join(project, "opencode.json")) as f:
            data = json.load(f)
        assert_eq(data.get("ours"), 1, "our key restored")


@scenario("18_user_deletes_json_file_sync_recreates")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        os.remove(os.path.join(project, "opencode.json"))
        sync.sync(project, emission, cfg)
        assert_true(os.path.exists(os.path.join(project, "opencode.json")), "recreated")


@scenario("19_check_json_file_with_our_keys_exits_zero")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        result = check.check(project, emission, cfg)
        assert_true(result["ok"], "check ok")


@scenario("20_check_json_file_with_missing_keys_reports_drift")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = {
            "rules": {
                "root": "./.opencode/rules",
                "imports": ["security"],
                "injections": {
                    "json": [{"file": "./opencode.json", "content": {"ours": 1}}],
                },
            },
            "skills": {"root": "./x", "imports": []},
            "agents": {"root": "./x", "imports": []},
        }
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        with open(os.path.join(project, "opencode.json"), "w") as f:
            json.dump({"something_else": True}, f)
        build_emission(cfg, REPO_ROOT, emission)
        result = check.check(project, emission, cfg)
        assert_true(not result["ok"], "check reports drift")


@scenario("21_self_validation_lock_hash_matches_file")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        with open(os.path.join(project, "agent-assets.lock")) as f:
            lock_data = json.load(f)
        for entry in lock_data["entries"]:
            if not os.path.isdir(os.path.join(project, entry["path"])):
                actual = hashlib.sha256(open(os.path.join(project, entry["path"]), "rb").read()).hexdigest()
                expected = entry["hash"].split("-", 1)[1]
                import base64
                b32 = base64.b32encode(bytes.fromhex(actual)).decode().rstrip("=")
                assert_eq(b32, expected, f"hash match {entry['path']}")


@scenario("22_idempotent_skill_recopy_no_extra_files_removed")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        project = os.path.join(td, "project")
        os.makedirs(project)
        build_emission(cfg, REPO_ROOT, emission)
        sync.sync(project, emission, cfg)
        extra = os.path.join(project, ".opencode/skills/migration/user-note.md")
        with open(extra, "w") as f:
            f.write("user note")
        sync.sync(project, emission, cfg)
        assert_true(os.path.exists(extra), "user-added file in skill dir preserved")


@scenario("23_sync_emission_contains_lock_and_files")
def _():
    with tempfile.TemporaryDirectory() as td:
        cfg = fixture_config_a()
        emission = os.path.join(td, "emission")
        build_emission(cfg, REPO_ROOT, emission)
        assert_true(os.path.exists(os.path.join(emission, "agent-assets.lock")), "emission has lock")


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