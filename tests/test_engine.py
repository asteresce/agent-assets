#!/usr/bin/env python3
"""Integration tests for bin/agent-assets.

Each test drives the real script against a throwaway registry and project
tree, then asserts on the emitted files, the manifest and the exit codes.
"""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ENGINE = REPO / "bin" / "agent-assets"

SECURITY = "## Summary\n\nKeep secrets safe.\n"
CODE_STYLE = "## Summary\n\nPrefer clarity.\n"
SKILLS_MD = "## Description\n\nMigrate things.\n"
CHECKLIST = "- plan\n- execute\n"
BUILD = "## Role\n\nBuilder.\n"

# Rendered form of SECURITY/CODE_STYLE under the base config's frontmatter.
RULES_FM = "---\nowner: default\nteam: platform\n---\n\n"
STYLE_FM = "---\nowner: import-team\nteam: platform\n---\n\n"


class EngineTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="agent-assets-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.src = self.tmp / "registry"
        self.project = self.tmp / "project"
        self.project.mkdir()
        self.config = self.tmp / "agent-assets.json"

        rules = self.src / "rules"
        rules.mkdir(parents=True)
        (rules / "security.md").write_text(SECURITY)
        (rules / "code-style.md").write_text(CODE_STYLE)

        skill = self.src / "skills" / "migration"
        skill.mkdir(parents=True)
        (skill / "SKILL.md").write_text(SKILLS_MD)
        (skill / "checklist.md").write_text(CHECKLIST)

        agents = self.src / "agents"
        agents.mkdir(parents=True)
        (agents / "build.md").write_text(BUILD)

        self.write_config(self.base_config())

    # ------------------------------------------------------------- fixtures

    @staticmethod
    def base_config():
        return {
            "rules": {
                "root": "./.opencode/rules",
                "imports": [
                    "security",
                    {
                        "name": "code-style",
                        "rename": "style",
                        "destination": "base",
                        "injections": {"frontmatter": {"owner": "import-team"}},
                    },
                ],
                "injections": {
                    "frontmatter": {"team": "platform", "owner": "default"},
                    "json": [
                        {
                            "file": "./opencode.json",
                            "content": {
                                "instructions": [".opencode/rules/**/*.md"],
                                "nested": {"a": 1},
                            },
                        }
                    ],
                },
            },
            "skills": {"root": "./.opencode/skills", "imports": ["migration"]},
            "agents": {"root": "./.opencode/agents", "imports": ["build"]},
        }

    def write_config(self, config):
        self.config.write_text(json.dumps(config, indent=2) + "\n")

    # -------------------------------------------------------------- helpers

    def run_engine(self, *args, expect=0):
        # Invoked via `bash` rather than the shebang: the Nix build sandbox
        # has no /usr/bin/env, so a `#!/usr/bin/env bash` script cannot be
        # exec'd there.
        cmd = [
            "bash",
            str(ENGINE),
            *args,
            "--config",
            str(self.config),
            "--src",
            str(self.src),
            "--project",
            str(self.project),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        self.assertEqual(
            proc.returncode,
            expect,
            "unexpected exit %d for %s\nstdout: %s\nstderr: %s"
            % (proc.returncode, cmd, proc.stdout, proc.stderr),
        )
        return proc

    def sync(self, expect=0):
        return self.run_engine("sync", expect=expect)

    def check(self, expect=0):
        return self.run_engine("check", expect=expect)

    def path(self, rel):
        return self.project / rel

    def read(self, rel):
        return self.path(rel).read_text()

    def write(self, rel, text):
        p = self.path(rel)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def manifest(self):
        return json.loads(self.read("agent-assets.lock"))

    def manifest_paths(self):
        return sorted(e["path"] for e in self.manifest())

    def frontmatter_of(self, text):
        """Split a rendered asset into (frontmatter, body).

        The rendered form is `---\\n<yaml>---\\n\\n<body>`; the blank line
        after the closing delimiter is part of the separator, not the body.
        """
        self.assertTrue(text.startswith("---\n"), "missing frontmatter: %r" % text)
        _, head, body = text.split("---\n", 2)
        self.assertTrue(body.startswith("\n"), "missing blank line: %r" % text)
        return head, body[1:]

    # -------------------------------------------------------- first sync

    def test_first_sync_writes_every_file(self):
        self.sync()
        self.assertEqual(self.read(".opencode/rules/security.md"), RULES_FM + SECURITY)
        self.assertEqual(self.read(".opencode/rules/base/style.md"), STYLE_FM + CODE_STYLE)
        self.assertEqual(self.read(".opencode/skills/migration/SKILL.md"), SKILLS_MD)
        self.assertEqual(self.read(".opencode/skills/migration/checklist.md"), CHECKLIST)
        self.assertEqual(self.read(".opencode/agents/build.md"), BUILD)

    def test_manifest_lists_one_entry_per_import(self):
        self.sync()
        self.assertEqual(
            self.manifest_paths(),
            [
                ".opencode/agents/build.md",
                ".opencode/rules/base/style.md",
                ".opencode/rules/security.md",
                ".opencode/skills/migration",
            ],
        )
        entry = next(e for e in self.manifest() if e["name"] == "migration")
        self.assertEqual(entry["category"], "skills")
        self.assertEqual(entry["path"], ".opencode/skills/migration")

    def test_manifest_has_no_hashes(self):
        self.sync()
        for entry in self.manifest():
            self.assertEqual(set(entry), {"category", "name", "path"})

    def test_resync_is_idempotent_and_silent(self):
        self.sync()
        before = self.manifest()
        proc = self.sync()
        self.assertEqual(proc.stdout, "")
        self.assertEqual(self.manifest(), before)

    # ------------------------------------------------------- drift/update

    def test_source_change_updates_file(self):
        self.sync()
        (self.src / "rules" / "security.md").write_text("## Summary\n\nNew text.\n")
        proc = self.sync()
        self.assertEqual(proc.stdout, "UPDATE .opencode/rules/security.md\n")
        self.assertIn("New text.", self.read(".opencode/rules/security.md"))

    def test_user_edits_to_owned_file_are_overwritten(self):
        self.sync()
        self.write(".opencode/rules/security.md", "local hack\n")
        proc = self.sync()
        self.assertEqual(proc.stdout, "UPDATE .opencode/rules/security.md\n")
        self.assertEqual(self.read(".opencode/rules/security.md"), RULES_FM + SECURITY)

    def test_unowned_files_are_never_touched(self):
        self.sync()
        self.write(".opencode/rules/custom.md", "mine\n")
        self.write(".opencode/skills/my-own/SKILL.md", "my own skill\n")
        self.sync()
        self.assertEqual(self.read(".opencode/rules/custom.md"), "mine\n")
        self.assertEqual(self.read(".opencode/skills/my-own/SKILL.md"), "my own skill\n")
        self.assertNotIn(".opencode/rules/custom.md", self.manifest_paths())
        self.assertNotIn(".opencode/skills/my-own", self.manifest_paths())

    def test_custom_assets_survive_dropping_an_import(self):
        self.sync()
        self.write(".opencode/rules/custom.md", "mine\n")
        self.write(".opencode/skills/my-own/SKILL.md", "my own skill\n")
        config = self.base_config()
        config["skills"]["imports"] = []
        self.write_config(config)
        self.sync()
        self.assertFalse(self.path(".opencode/skills/migration").exists())
        self.assertEqual(self.read(".opencode/rules/custom.md"), "mine\n")
        self.assertEqual(self.read(".opencode/skills/my-own/SKILL.md"), "my own skill\n")

    # ------------------------------------------------------------- orphans

    def test_removed_import_is_deleted_as_orphan(self):
        config = self.base_config()
        del config["rules"]["imports"][0]
        self.write_config(config)
        self.sync()
        self.assertFalse(self.path(".opencode/rules/security.md").exists())

    def test_removed_artifact_inside_skill_dir_is_deleted(self):
        self.sync()
        self.assertTrue(self.path(".opencode/skills/migration/checklist.md").exists())
        (self.src / "skills" / "migration" / "checklist.md").unlink()
        proc = self.sync()
        self.assertEqual(proc.stdout, "REMOVE .opencode/skills/migration/checklist.md\n")
        self.assertFalse(self.path(".opencode/skills/migration/checklist.md").exists())
        self.assertTrue(self.path(".opencode/skills/migration/SKILL.md").exists())

    def test_asset_contents_are_immutable(self):
        self.sync()
        self.write(".opencode/skills/migration/stray.md", "not mine\n")
        proc = self.check(expect=1)
        self.assertEqual(proc.stdout, "EXTRA .opencode/skills/migration/stray.md\n")
        proc = self.sync()
        self.assertEqual(proc.stdout, "REMOVE .opencode/skills/migration/stray.md\n")
        self.assertFalse(self.path(".opencode/skills/migration/stray.md").exists())
        self.check()

    def test_orphan_removal_prunes_empty_dirs(self):
        config = self.base_config()
        config["rules"]["imports"] = [
            {"name": "security", "destination": "deep/nested"}
        ]
        self.write_config(config)
        self.sync()
        self.assertTrue(self.path(".opencode/rules/deep/nested/security.md").exists())
        config["rules"]["imports"] = []
        self.write_config(config)
        self.sync()
        self.assertFalse(self.path(".opencode/rules/deep").exists())

    def test_missing_manifest_syncs_without_deleting(self):
        self.sync()
        self.write(".opencode/rules/security.md", "do not delete me\n")
        self.path("agent-assets.lock").unlink()
        proc = self.sync()
        self.assertNotIn("REMOVE", proc.stdout)
        # the owned file is still rewritten from the registry
        self.assertEqual(self.read(".opencode/rules/security.md"), RULES_FM + SECURITY)

    def test_unreadable_manifest_skips_orphan_removal(self):
        self.sync()
        self.path("agent-assets.lock").write_text("{ not json")
        proc = self.sync()
        self.assertNotIn("REMOVE", proc.stdout)
        self.assertIn("unreadable manifest", proc.stderr)
        # and the manifest is repaired
        self.assertEqual(len(self.manifest()), 4)

    # ---------------------------------------------------------- frontmatter

    def test_category_frontmatter_is_applied(self):
        self.sync()
        head, body = self.frontmatter_of(self.read(".opencode/rules/security.md"))
        self.assertEqual(head, "owner: default\nteam: platform\n")
        self.assertEqual(body, SECURITY)

    def test_per_import_frontmatter_overrides_category(self):
        self.sync()
        head, body = self.frontmatter_of(self.read(".opencode/rules/base/style.md"))
        self.assertEqual(head, "owner: import-team\nteam: platform\n")
        self.assertEqual(body, CODE_STYLE)

    def test_frontmatter_injection_replaces_source_frontmatter(self):
        (self.src / "rules" / "security.md").write_text(
            "---\nsource: kept\n---\n\n" + SECURITY
        )
        self.sync()
        text = self.read(".opencode/rules/security.md")
        head, body = self.frontmatter_of(text)
        self.assertEqual(head, "owner: default\nteam: platform\n")
        self.assertNotIn("source: kept", text)
        self.assertEqual(body, SECURITY)

    def test_no_frontmatter_block_when_none_configured(self):
        self.sync()
        self.assertEqual(
            self.read(".opencode/skills/migration/SKILL.md"), SKILLS_MD
        )
        self.assertEqual(self.read(".opencode/agents/build.md"), BUILD)

    def test_skills_md_gets_frontmatter_but_artifacts_do_not(self):
        config = self.base_config()
        config["skills"]["injections"] = {"frontmatter": {"scope": "skills"}}
        self.write_config(config)
        self.sync()
        head, body = self.frontmatter_of(self.read(".opencode/skills/migration/SKILL.md"))
        self.assertEqual(head, "scope: skills\n")
        self.assertEqual(body, SKILLS_MD)
        self.assertEqual(self.read(".opencode/skills/migration/checklist.md"), CHECKLIST)

    def test_frontmatter_values_are_quoted_when_needed(self):
        config = self.base_config()
        config["rules"]["injections"]["frontmatter"] = {"description": "a: b"}
        self.write_config(config)
        self.sync()
        head, _ = self.frontmatter_of(self.read(".opencode/rules/security.md"))
        self.assertEqual(head, "description: 'a: b'\n")

    # ------------------------------------------------------- json injection

    def test_json_injection_creates_missing_file(self):
        self.sync()
        data = json.loads(self.read("opencode.json"))
        self.assertEqual(
            data, {"instructions": [".opencode/rules/**/*.md"], "nested": {"a": 1}}
        )

    def test_json_injection_preserves_user_keys(self):
        self.write("opencode.json", json.dumps({"user_only": "kept", "nested": {"b": 2}}))
        self.sync()
        data = json.loads(self.read("opencode.json"))
        self.assertEqual(data["user_only"], "kept")
        self.assertEqual(data["nested"], {"a": 1, "b": 2})

    def test_json_injection_list_union_is_idempotent(self):
        self.sync()
        first = json.loads(self.read("opencode.json"))
        self.sync()
        self.sync()
        self.assertEqual(json.loads(self.read("opencode.json")), first)

    def test_json_injection_keeps_user_list_items(self):
        self.write(
            "opencode.json", json.dumps({"instructions": [".claude/rules/*.md"]})
        )
        self.sync()
        self.assertEqual(
            json.loads(self.read("opencode.json"))["instructions"],
            [".claude/rules/*.md", ".opencode/rules/**/*.md"],
        )

    def test_json_injection_readds_removed_key(self):
        self.sync()
        self.write("opencode.json", json.dumps({"other": True}))
        self.sync()
        self.assertIn("instructions", json.loads(self.read("opencode.json")))

    def test_json_injection_chunks_from_categories_are_combined(self):
        config = self.base_config()
        config["agents"]["injections"] = {
            "json": [{"file": "./opencode.json", "content": {"from_agents": True}}]
        }
        self.write_config(config)
        self.sync()
        data = json.loads(self.read("opencode.json"))
        self.assertEqual(data["from_agents"], True)
        self.assertIn("instructions", data)

    def test_json_injection_rejects_invalid_target(self):
        self.write("opencode.json", "not json at all")
        self.sync(expect=1)

    # ------------------------------------------------------------- check

    def test_check_is_quiet_and_zero_when_clean(self):
        self.sync()
        proc = self.check()
        self.assertEqual(proc.stdout, "")

    def test_check_reports_drift(self):
        self.sync()
        self.write(".opencode/rules/security.md", "tampered\n")
        proc = self.check(expect=1)
        self.assertEqual(
            proc.stdout, "DRIFT .opencode/rules/security.md (from rules/security)\n"
        )

    def test_check_reports_missing(self):
        self.sync()
        self.path(".opencode/agents/build.md").unlink()
        proc = self.check(expect=1)
        self.assertEqual(
            proc.stdout, "MISSING .opencode/agents/build.md (from agents/build)\n"
        )

    def test_check_reports_orphan(self):
        self.sync()
        config = self.base_config()
        config["rules"]["imports"] = []
        config["rules"]["injections"] = {}
        self.write_config(config)
        proc = self.check(expect=1)
        self.assertIn("ORPHAN .opencode/rules/security.md\n", proc.stdout)
        self.assertIn("ORPHAN .opencode/rules/base/style.md\n", proc.stdout)

    def test_check_reports_injection_gap(self):
        self.sync()
        self.write("opencode.json", json.dumps({"user": 1}))
        proc = self.check(expect=1)
        self.assertEqual(proc.stdout, "INJECT opencode.json\n")

    def test_check_reports_missing_injection_target(self):
        self.sync()
        self.path("opencode.json").unlink()
        proc = self.check(expect=1)
        self.assertEqual(proc.stdout, "INJECT opencode.json\n")

    def test_check_never_modifies_the_project(self):
        self.sync()
        self.write(".opencode/rules/security.md", "tampered\n")
        self.write("opencode.json", json.dumps({"user": 1}))
        self.check(expect=1)
        self.assertEqual(self.read(".opencode/rules/security.md"), "tampered\n")
        self.assertEqual(json.loads(self.read("opencode.json")), {"user": 1})

    # ------------------------------------------------------------- render

    def test_render_writes_tree_only(self):
        out = self.tmp / "out"
        self.run_engine("render", "--out", str(out))
        self.assertEqual(
            (out / ".opencode/rules/security.md").read_text(), RULES_FM + SECURITY
        )
        self.assertFalse(self.path(".opencode").exists())
        self.assertFalse(self.path("agent-assets.lock").exists())

    def test_render_matches_sync_output(self):
        out = self.tmp / "out"
        self.sync()
        self.run_engine("render", "--out", str(out))
        rendered = sorted(
            str(p.relative_to(out)) for p in out.rglob("*") if p.is_file()
        )
        self.assertTrue(rendered)
        for rel in rendered:
            self.assertEqual(
                (out / rel).read_bytes(),
                self.path(rel).read_bytes(),
                rel,
            )

    def test_rename_applies_to_directory_assets(self):
        config = self.base_config()
        config["skills"]["imports"] = [{"name": "migration", "rename": "db-migration"}]
        self.write_config(config)
        self.sync()
        self.assertTrue(self.path(".opencode/skills/db-migration/SKILL.md").exists())
        self.assertFalse(self.path(".opencode/skills/migration").exists())
        self.assertIn(".opencode/skills/db-migration", self.manifest_paths())

    # --------------------------------------------------------------- misc

    def test_default_root_is_used_when_omitted(self):
        config = {"rules": {"imports": ["security"]}}
        self.write_config(config)
        self.sync()
        self.assertTrue(self.path(".agent-assets/rules/security.md").exists())

    def test_string_shorthand_import(self):
        config = {"rules": {"root": "./r", "imports": ["security"]}}
        self.write_config(config)
        self.sync()
        self.assertEqual(self.read("r/security.md"), SECURITY)

    def test_unknown_asset_is_an_error(self):
        config = {"rules": {"root": "./r", "imports": ["nope"]}}
        self.write_config(config)
        proc = self.sync(expect=1)
        self.assertIn("asset not found", proc.stderr)

    def test_path_traversal_is_rejected(self):
        config = {
            "rules": {"root": "./r", "imports": [{"name": "security", "destination": "../.."}]}
        }
        self.write_config(config)
        proc = self.sync(expect=1)
        self.assertIn("must not contain '..'", proc.stderr)

    def test_absolute_root_is_rejected(self):
        config = {"rules": {"root": "/etc", "imports": ["security"]}}
        self.write_config(config)
        proc = self.sync(expect=1)
        self.assertIn("project-relative", proc.stderr)

    def test_bad_import_shape_is_an_error(self):
        config = {"rules": {"root": "./r", "imports": [42]}}
        self.write_config(config)
        proc = self.sync(expect=1)
        self.assertIn("imports must be", proc.stderr)

    def test_non_object_frontmatter_is_an_error(self):
        config = self.base_config()
        config["rules"]["injections"]["frontmatter"] = "oops"
        self.write_config(config)
        proc = self.sync(expect=1)
        self.assertIn("must be an object", proc.stderr)

    def test_no_op_config_is_fine(self):
        self.write_config({})
        proc = self.sync()
        self.assertEqual(proc.stdout, "")
        self.assertEqual(self.manifest(), [])
        self.check()


if __name__ == "__main__":
    unittest.main()
