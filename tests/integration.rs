//! The behaviour suite.
//!
//! This is the executable specification for README "How it behaves": every
//! rule there has a test here, and the two are changed in the same commit.
//! Each test drives the built binary against a throwaway registry and
//! project tree (see `common`).

mod common;

use assert_cmd::cargo::CommandCargoExt;
use common::*;
use serde_json::json;

/// What a first `sync` writes for the fixture.
const FIRST_SYNC: &str = "\
MERGE opencode.json
WRITE .opencode/agents/build.md
WRITE .opencode/rules/security.md
WRITE .opencode/rules/base/style.md
WRITE .opencode/skills/tdd/SKILL.md
WRITE .opencode/skills/tdd/checklist.md
";

/// What `check` reports before anything has been synced.
const FIRST_CHECK: &str = "\
MISSING .opencode/agents/build.md (from agents/build)
MISSING .opencode/rules/security.md (from rules/security)
MISSING .opencode/rules/base/style.md (from rules/code-style)
MISSING .opencode/skills/tdd/SKILL.md (from skills/tdd)
MISSING .opencode/skills/tdd/checklist.md (from skills/tdd)
INJECT opencode.json
";

// ============================================================ first sync

#[test]
fn first_sync_writes_every_file() {
    let world = World::fixture();
    world.sync().ok().stdout_is(FIRST_SYNC);
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        format!("{RULES_FM}{SECURITY}")
    );
    assert_eq!(
        world.read_project(".opencode/rules/base/style.md"),
        format!("{STYLE_FM}{CODE_STYLE}")
    );
    assert_eq!(world.read_project(&skill("tdd")), SKILL_MD);
    assert_eq!(
        world.read_project(".opencode/skills/tdd/checklist.md"),
        CHECKLIST
    );
    assert_eq!(world.read_project(".opencode/agents/build.md"), BUILD);
}

#[test]
fn the_manifest_lists_one_row_per_import() {
    let world = World::fixture();
    world.sync().ok();
    let rows: Vec<String> = world
        .manifest()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        rows,
        vec![
            ".opencode/agents/build.md",
            ".opencode/rules/base/style.md",
            ".opencode/rules/security.md",
            ".opencode/skills/tdd",
        ]
    );
}

#[test]
fn resync_is_idempotent_and_silent() {
    let world = World::fixture();
    world.sync().ok();
    let before = world.manifest_text();
    world.sync().ok().stdout_is("");
    assert_eq!(world.manifest_text(), before);
}

// ======================================================= drift and edits

#[test]
fn a_changed_source_is_an_update() {
    let world = World::fixture();
    world.sync().ok();
    world.write_asset("rules/security.md", "## Summary\n\nNew text.\n");
    world
        .sync()
        .ok()
        .stdout_is("UPDATE .opencode/rules/security.md\n");
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        format!("{RULES_FM}## Summary\n\nNew text.\n")
    );
}

#[test]
fn a_hand_edit_is_a_conflict_that_sync_will_not_touch() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/security.md", "local hack\n");

    world
        .check()
        .fails()
        .stdout_is("CONFLICT .opencode/rules/security.md (from rules/security)\n");
    world
        .sync()
        .fails()
        .stdout_is("CONFLICT .opencode/rules/security.md (from rules/security)\n");
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        "local hack\n"
    );
}

#[test]
fn force_overwrites_a_conflict() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/security.md", "local hack\n");
    world
        .sync_forced()
        .ok()
        .stdout_is("UPDATE .opencode/rules/security.md\n");
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        format!("{RULES_FM}{SECURITY}")
    );
}

#[test]
fn unowned_files_are_never_touched() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/custom.md", "mine\n");
    world.write_project(&skill("my-own"), "my own skill\n");
    world.sync().ok().stdout_is("");
    assert_eq!(world.read_project(".opencode/rules/custom.md"), "mine\n");
    assert_eq!(world.read_project(&skill("my-own")), "my own skill\n");
}

#[test]
fn custom_assets_survive_dropping_an_import() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/custom.md", "mine\n");
    world.write_project(&skill("my-own"), "my own skill\n");

    let mut config = base_config();
    config["skills"]["imports"] = json!([]);
    world.write_config(&config);
    world.sync().ok();

    assert!(!world.exists(".opencode/skills/tdd"));
    assert_eq!(world.read_project(".opencode/rules/custom.md"), "mine\n");
    assert_eq!(world.read_project(&skill("my-own")), "my own skill\n");
}

// =============================================================== orphans

#[test]
fn a_dropped_import_is_deleted_as_an_orphan() {
    let world = World::fixture();
    world.sync().ok();
    let mut config = base_config();
    config["rules"]["imports"].as_array_mut().unwrap().remove(0);
    world.write_config(&config);

    world
        .sync()
        .ok()
        .stdout_is("REMOVE .opencode/rules/security.md\n");
    assert!(!world.exists(".opencode/rules/security.md"));
}

#[test]
fn a_removed_artifact_inside_a_skill_is_deleted() {
    let world = World::fixture();
    world.sync().ok();
    world.remove_asset("skills/tdd/checklist.md");

    world
        .sync()
        .ok()
        .stdout_is("REMOVE .opencode/skills/tdd/checklist.md\n");
    assert!(!world.exists(".opencode/skills/tdd/checklist.md"));
    assert!(world.exists(&skill("tdd")));
}

#[test]
fn a_stray_file_inside_a_managed_asset_is_extra() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/skills/tdd/stray.md", "not mine\n");

    world
        .check()
        .fails()
        .stdout_is("EXTRA .opencode/skills/tdd/stray.md\n");
    world
        .sync()
        .ok()
        .stdout_is("REMOVE .opencode/skills/tdd/stray.md\n");
    assert!(!world.exists(".opencode/skills/tdd/stray.md"));
    world.check().ok().stdout_is("");
}

#[test]
fn orphan_removal_prunes_empty_directories() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["imports"] = json!([{"name": "security", "destination": "deep/nested"}]);
    world.write_config(&config);
    world.sync().ok();
    assert!(world.exists(".opencode/rules/deep/nested/security.md"));

    config["rules"]["imports"] = json!([]);
    world.write_config(&config);
    world.sync().ok();
    assert!(!world.exists(".opencode/rules/deep"));
}

// ================================================ manifest robustness

#[test]
fn a_missing_manifest_skips_orphan_removal() {
    let world = World::fixture();
    world.sync().ok();
    let mut config = base_config();
    config["rules"]["imports"].as_array_mut().unwrap().remove(0);
    world.write_config(&config);
    world.remove_project("agent-assets.lock");

    world.sync().ok().stdout_is("");
    assert!(
        world.exists(".opencode/rules/security.md"),
        "nothing may be deleted without a manifest to say it is ours"
    );
}

#[test]
fn a_missing_manifest_treats_drift_as_a_conflict() {
    // Without the manifest there is no memory of what we wrote, so a
    // difference might be a hand edit. `--force` is the way through.
    let world = World::fixture();
    world.sync().ok();
    world.remove_project("agent-assets.lock");
    world.write_asset("rules/security.md", "## Summary\n\nChanged.\n");

    world
        .sync()
        .fails()
        .stdout_is("CONFLICT .opencode/rules/security.md (from rules/security)\n");
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        format!("{RULES_FM}{SECURITY}")
    );
}

#[test]
fn an_unreadable_manifest_is_warned_about_and_repaired() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project("agent-assets.lock", "{ not json");

    let run = world.sync();
    run.ok();
    run.stderr_contains("unreadable manifest");
    run.stdout_is("");
    assert_eq!(world.manifest().as_array().unwrap().len(), 4);
}

// ============================================================ frontmatter

#[test]
fn category_frontmatter_is_applied() {
    let world = World::fixture();
    world.sync().ok();
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        format!("{RULES_FM}{SECURITY}")
    );
}

#[test]
fn per_import_frontmatter_overrides_the_category() {
    let world = World::fixture();
    world.sync().ok();
    assert_eq!(
        world.read_project(".opencode/rules/base/style.md"),
        format!("{STYLE_FM}{CODE_STYLE}")
    );
}

#[test]
fn frontmatter_replaces_whatever_the_source_carried() {
    let world = World::fixture();
    world.write_asset(
        "rules/security.md",
        &format!("---\nsource: kept\n---\n\n{SECURITY}"),
    );
    world.sync().ok();
    let text = world.read_project(".opencode/rules/security.md");
    assert_eq!(text, format!("{RULES_FM}{SECURITY}"));
    assert!(!text.contains("source: kept"));
}

#[test]
fn no_frontmatter_block_is_emitted_when_none_is_configured() {
    let world = World::fixture();
    world.sync().ok();
    assert_eq!(world.read_project(&skill("tdd")), SKILL_MD);
    assert_eq!(world.read_project(".opencode/agents/build.md"), BUILD);
}

#[test]
fn only_a_skill_entry_file_gets_frontmatter() {
    let world = World::fixture();
    let mut config = base_config();
    config["skills"]["injections"] = json!({"frontmatter": {"scope": "skills"}});
    world.write_config(&config);
    world.sync().ok();
    assert_eq!(
        world.read_project(&skill("tdd")),
        "---\nscope: skills\n---\n\n".to_string() + SKILL_MD
    );
    assert_eq!(
        world.read_project(".opencode/skills/tdd/checklist.md"),
        CHECKLIST
    );
}

#[test]
fn frontmatter_values_are_quoted_when_they_need_it() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["injections"]["frontmatter"] = json!({"description": "a: b"});
    world.write_config(&config);
    world.sync().ok();
    let text = world.read_project(".opencode/rules/security.md");
    assert!(
        text.starts_with("---\ndescription: 'a: b'\n---\n"),
        "{text:?}"
    );
}

// ======================================================== json injections

#[test]
fn an_injection_creates_a_missing_target() {
    let world = World::fixture();
    world.sync().ok();
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert_eq!(
        value,
        json!({"instructions": [".opencode/rules/**/*.md"], "nested": {"a": 1}})
    );
}

#[test]
fn an_injection_preserves_keys_the_user_already_had() {
    let world = World::fixture();
    world.write_project(
        "opencode.json",
        &json!({"user_only": "kept", "nested": {"b": 2}}).to_string(),
    );
    world.sync().ok();
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert_eq!(value["user_only"], json!("kept"));
    assert_eq!(value["nested"], json!({"a": 1, "b": 2}));
}

#[test]
fn an_injection_union_is_idempotent() {
    let world = World::fixture();
    world.sync().ok();
    let first = world.read_project("opencode.json");
    world.sync().ok();
    world.sync().ok();
    assert_eq!(world.read_project("opencode.json"), first);
}

#[test]
fn an_injection_keeps_the_users_own_list_items() {
    let world = World::fixture();
    world.write_project(
        "opencode.json",
        &json!({"instructions": [".claude/rules/*.md"]}).to_string(),
    );
    world.sync().ok();
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert_eq!(
        value["instructions"],
        json!([".claude/rules/*.md", ".opencode/rules/**/*.md"])
    );
}

#[test]
fn an_injection_readds_a_key_that_was_removed() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project("opencode.json", &json!({"other": true}).to_string());
    world.sync().ok();
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert!(value.get("instructions").is_some());
}

#[test]
fn injection_chunks_from_several_categories_combine() {
    let world = World::fixture();
    let mut config = base_config();
    config["agents"]["injections"] =
        json!({"json": [{"file": "./opencode.json", "content": {"from_agents": true}}]});
    world.write_config(&config);
    world.sync().ok();
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert_eq!(value["from_agents"], json!(true));
    assert!(value.get("instructions").is_some());
}

#[test]
fn an_injection_target_that_is_not_valid_json_is_refused() {
    let world = World::fixture();
    world.write_project("opencode.json", "not json at all");
    world.sync().fails().stderr_contains("not valid JSON");
}

#[test]
fn an_injection_target_inside_a_managed_asset_is_refused() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["injections"]["json"] =
        json!([{"file": "./.opencode/rules/security.md", "content": {"a": 1}}]);
    world.write_config(&config);
    world
        .sync()
        .fails()
        .stderr_contains("collides with the asset");
}

#[test]
fn an_injection_target_is_never_deleted() {
    let world = World::fixture();
    world.sync().ok();
    let mut config = base_config();
    config["rules"]["imports"].as_array_mut().unwrap().remove(0);
    config["rules"]["injections"]["json"] =
        json!([{"file": "./opencode.json", "content": {"a": 1}}]);
    world.write_config(&config);
    world.sync().ok();
    assert!(world.exists("opencode.json"));
}

// ================================================================= check

#[test]
fn check_is_quiet_and_zero_when_clean() {
    let world = World::fixture();
    world.sync().ok();
    world.check().ok().stdout_is("");
}

#[test]
fn check_reports_drift_after_a_source_change() {
    let world = World::fixture();
    world.sync().ok();
    world.write_asset("rules/security.md", "## Summary\n\nChanged.\n");
    world
        .check()
        .fails()
        .stdout_is("DRIFT .opencode/rules/security.md (from rules/security)\n");
}

#[test]
fn check_reports_missing_and_orphan_and_inject() {
    let world = World::fixture();
    world.check().fails().stdout_is(FIRST_CHECK);

    world.sync().ok();
    let mut config = base_config();
    config["rules"]["imports"].as_array_mut().unwrap().remove(0);
    world.write_config(&config);
    world.write_project("opencode.json", &json!({"other": true}).to_string());
    world
        .check()
        .fails()
        .stdout_is("ORPHAN .opencode/rules/security.md\nINJECT opencode.json\n");
}

#[test]
fn check_never_modifies_the_project() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/security.md", "tampered\n");
    world.write_project("opencode.json", &json!({"user": 1}).to_string());

    world.check().fails();
    assert_eq!(
        world.read_project(".opencode/rules/security.md"),
        "tampered\n"
    );
    let value: serde_json::Value =
        serde_json::from_str(&world.read_project("opencode.json")).unwrap();
    assert_eq!(value, json!({"user": 1}));
}

#[test]
fn force_is_rejected_outside_sync() {
    let world = World::fixture();
    world
        .run(&["check", "--force"])
        .fails()
        .stderr_contains("only applies to sync");
}

// =============================================================== colours

#[test]
fn status_words_are_coloured_when_asked() {
    let world = World::fixture();
    let written = world.run(&["sync", "--color", "always"]);
    assert!(
        written.stdout.contains("\u{1b}[32mWRITE\u{1b}[0m"),
        "{:?}",
        written.stdout
    );

    world.write_asset("rules/security.md", "## Summary\n\nChanged.\n");
    let reported = world.run(&["check", "--color", "always"]);
    reported.fails();
    assert!(
        reported.stdout.contains("\u{1b}[33mDRIFT\u{1b}[0m"),
        "{:?}",
        reported.stdout
    );
    // The path stays plain so the output is greppable.
    assert!(
        reported.stdout.contains(" .opencode/rules/security.md "),
        "{:?}",
        reported.stdout
    );
    assert!(
        reported
            .stdout
            .contains("\u{1b}[2m(from rules/security)\u{1b}[0m"),
        "{:?}",
        reported.stdout
    );
}

#[test]
fn status_words_stay_plain_when_asked() {
    let world = World::fixture();
    world.sync().ok();
    world.write_asset("rules/security.md", "## Summary\n\nChanged.\n");
    world
        .run(&["check", "--color", "never"])
        .fails()
        .stdout_is("DRIFT .opencode/rules/security.md (from rules/security)\n");
}

#[test]
fn no_colour_is_honoured_without_an_explicit_flag() {
    let world = World::fixture();
    world.sync().ok();
    world.write_asset("rules/security.md", "## Summary\n\nChanged.\n");
    world
        .run_with(&["check"], "agent-assets.lock", &[("NO_COLOR", "1")])
        .fails()
        .stdout_is("DRIFT .opencode/rules/security.md (from rules/security)\n");
}

// ================================================================ render

#[test]
fn render_writes_the_tree_and_nothing_else() {
    let world = World::fixture();
    let out = world.tmp.path().join("out");
    world.render(&out).ok().stdout_is("");
    assert_eq!(
        std::fs::read_to_string(out.join(".opencode/rules/security.md")).unwrap(),
        format!("{RULES_FM}{SECURITY}")
    );
    assert!(!world.exists(".opencode"));
    assert!(!world.exists("agent-assets.lock"));
}

#[test]
fn render_matches_what_sync_would_write() {
    let world = World::fixture();
    world.sync().ok();
    let out = world.tmp.path().join("out");
    world.render(&out).ok();

    let mut rendered: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(walk_files)
        .map(|p| p.strip_prefix(&out).unwrap().to_string_lossy().into_owned())
        .collect();
    rendered.sort();
    assert!(!rendered.is_empty());
    for relative in rendered {
        assert_eq!(
            std::fs::read(out.join(&relative)).unwrap(),
            std::fs::read(world.project.join(&relative)).unwrap(),
            "{relative}"
        );
    }
}

fn walk_files(path: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    if path.is_file() {
        return vec![path];
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries {
            out.extend(walk_files(entry.unwrap().path()));
        }
    }
    out
}

#[test]
fn rename_applies_to_directory_assets_too() {
    let world = World::fixture();
    let mut config = base_config();
    config["skills"]["imports"] = json!([{"name": "tdd", "rename": "testing"}]);
    world.write_config(&config);
    world.sync().ok();
    assert!(world.exists(&skill("testing")));
    assert!(!world.exists(".opencode/skills/tdd"));
}

// ================================================ defaults and shorthand

#[test]
fn the_default_root_is_used_when_the_category_omits_one() {
    let world = World::fixture();
    world.write_config(&json!({"rules": {"imports": ["security"]}}));
    world.sync().ok();
    assert!(world.exists(&format!("{}/rules/security.md", default_root())));
}

#[test]
fn a_bare_string_import_is_shorthand_for_its_name() {
    let world = World::fixture();
    world.write_config(&json!({"rules": {"root": "./r", "imports": ["security"]}}));
    world.sync().ok();
    assert_eq!(world.read_project("r/security.md"), SECURITY);
}

#[test]
fn an_empty_config_is_a_no_op() {
    let world = World::new();
    world.write_config(&json!({}));
    world.sync().ok().stdout_is("");
    assert_eq!(world.manifest(), json!([]));
    world.check().ok().stdout_is("");
}

// ============================================================ rejections

#[test]
fn an_unknown_asset_is_an_error() {
    let world = World::fixture();
    world.write_config(&json!({"rules": {"root": "./r", "imports": ["nope"]}}));
    world.sync().fails().stderr_contains("asset not found");
}

#[test]
fn path_traversal_is_rejected() {
    let world = World::fixture();
    world.write_config(&json!({
        "rules": {"root": "./r", "imports": [{"name": "security", "destination": "../.."}]}
    }));
    world.sync().fails().stderr_contains("must not contain");
}

#[test]
fn an_absolute_root_is_rejected() {
    let world = World::fixture();
    world.write_config(&json!({"rules": {"root": "/etc", "imports": ["security"]}}));
    world.sync().fails().stderr_contains("project-relative");
}

#[test]
fn a_malformed_import_is_rejected() {
    let world = World::fixture();
    world.write_config(&json!({"rules": {"root": "./r", "imports": [42]}}));
    world.sync().fails().stderr_contains("rules.imports[0]");
}

#[test]
fn a_non_object_frontmatter_is_rejected() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["injections"]["frontmatter"] = json!("oops");
    world.write_config(&config);
    world
        .sync()
        .fails()
        .stderr_contains("rules.injections.frontmatter");
}

#[test]
fn control_characters_in_a_path_are_rejected() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["root"] = json!("./.opencode/rules\u{1f}/evil");
    world.write_config(&config);
    world.sync().fails().stderr_contains("control characters");
}

#[test]
fn a_dot_or_dot_dot_destination_is_rejected() {
    let world = World::fixture();
    for destination in [".", "base/."] {
        let mut config = base_config();
        config["rules"]["imports"] = json!([{"name": "security", "destination": destination}]);
        world.write_config(&config);
        world.sync().fails().stderr_contains("must not contain");
    }
}

#[test]
fn a_malformed_injection_entry_is_rejected() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["injections"]["json"] = json!([{"content": {}}]);
    world.write_config(&config);
    world
        .render(&world.tmp.path().join("out"))
        .fails()
        .stderr_contains("injections.json");
}

#[test]
fn two_imports_may_not_emit_the_same_path() {
    let world = World::fixture();
    world.write_config(&json!({
        "rules": {
            "root": "./r",
            "imports": [
                {"name": "security", "rename": "same"},
                {"name": "security", "rename": "same"}
            ]
        }
    }));
    world.sync().fails().stderr_contains("same output path");
}

// ======================================================== manifest shape

#[test]
fn the_manifest_records_a_hash_for_every_emitted_file() {
    let world = World::fixture();
    world.sync().ok();
    let manifest = world.manifest();
    let rows = manifest.as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        assert!(row["path"].is_string());
        let files = row["files"].as_array().unwrap();
        assert!(!files.is_empty());
        for file in files {
            assert!(file["path"].is_string());
            assert!(
                file["hash"].as_str().unwrap().starts_with("sha256:"),
                "{file}"
            );
        }
    }
    // The hashes are of the rendered bytes, not of the source.
    let rule = rows
        .iter()
        .find(|row| row["path"] == json!(".opencode/rules/security.md"))
        .unwrap();
    assert_eq!(
        rule["files"][0]["hash"].as_str().unwrap(),
        agent_assets::manifest::hash_bytes(format!("{RULES_FM}{SECURITY}").as_bytes())
    );
}

#[test]
fn the_manifest_path_is_normalised() {
    let world = World::fixture();
    world.run_with(&["sync"], "./agent-assets.lock", &[]).ok();
    let first = world.manifest_text();
    world.run_with(&["sync"], "agent-assets.lock/", &[]).ok();
    assert_eq!(world.manifest_text(), first);
}

// ======================================================= symlink safety

#[test]
fn a_dangling_symlink_at_a_file_path_is_not_written_through() {
    let world = World::fixture();
    std::fs::create_dir_all(world.project.join(".opencode/rules")).unwrap();
    std::os::unix::fs::symlink(
        world.project.join("ghost.md"),
        world.project.join(".opencode/rules/security.md"),
    )
    .unwrap();

    world
        .sync()
        .fails()
        .stderr_contains("refusing to replace unowned");
    assert!(!world.exists("opencode.json"));
    assert!(!world.exists("agent-assets.lock"));
}

#[test]
fn a_symlink_to_a_file_outside_the_project_is_not_written_through() {
    let world = World::fixture();
    let outside = world.tmp.path().join("outside.md");
    std::fs::write(&outside, "secret\n").unwrap();
    std::fs::create_dir_all(world.project.join(".opencode/rules")).unwrap();
    std::os::unix::fs::symlink(&outside, world.project.join(".opencode/rules/security.md"))
        .unwrap();

    world
        .sync()
        .fails()
        .stderr_contains("refusing to replace unowned");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret\n");
}

#[test]
fn an_owned_symlink_at_a_file_path_is_replaced_not_followed() {
    let world = World::fixture();
    world.sync().ok();
    let outside = world.tmp.path().join("outside.md");
    std::fs::write(&outside, "secret\n").unwrap();
    let target = world.project.join(".opencode/rules/security.md");
    std::fs::remove_file(&target).unwrap();
    std::os::unix::fs::symlink(&outside, &target).unwrap();

    world.sync().ok();
    assert!(!target.is_symlink());
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        format!("{RULES_FM}{SECURITY}")
    );
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret\n");
}

#[test]
fn an_unowned_symlink_at_a_directory_asset_path_is_not_followed() {
    let world = World::fixture();
    let outside = world.tmp.path().join("outside-skill");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.md"), "keep me\n").unwrap();
    std::fs::create_dir_all(world.project.join(".opencode/skills")).unwrap();
    std::os::unix::fs::symlink(&outside, world.project.join(".opencode/skills/tdd")).unwrap();

    world
        .sync()
        .fails()
        .stderr_contains("refusing to replace unowned");
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.md")).unwrap(),
        "keep me\n"
    );
}

#[test]
fn an_owned_symlink_at_a_directory_asset_path_is_replaced_and_the_referent_is_intact() {
    let world = World::fixture();
    world.sync().ok();
    let outside = world.tmp.path().join("outside-skill");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.md"), "keep me\n").unwrap();
    let target = world.project.join(".opencode/skills/tdd");
    std::fs::remove_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&outside, &target).unwrap();

    world.sync().ok();
    assert!(!target.is_symlink());
    assert!(world.exists(".opencode/skills/tdd/SKILL.md"));
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.md")).unwrap(),
        "keep me\n"
    );
}

#[test]
fn check_calls_a_symlink_at_a_file_path_drift_even_when_the_bytes_match() {
    let world = World::fixture();
    world.sync().ok();
    let outside = world.tmp.path().join("outside.md");
    std::fs::write(&outside, format!("{RULES_FM}{SECURITY}")).unwrap();
    let target = world.project.join(".opencode/rules/security.md");
    std::fs::remove_file(&target).unwrap();
    std::os::unix::fs::symlink(&outside, &target).unwrap();

    world
        .check()
        .fails()
        .stdout_is("DRIFT .opencode/rules/security.md (from rules/security)\n");
}

#[test]
fn check_calls_a_symlink_at_a_directory_asset_path_drift() {
    let world = World::fixture();
    world.sync().ok();
    let outside = world.tmp.path().join("outside-skill");
    std::fs::create_dir_all(&outside).unwrap();
    let target = world.project.join(".opencode/skills/tdd");
    std::fs::remove_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&outside, &target).unwrap();

    world
        .check()
        .fails()
        .stdout_is("DRIFT .opencode/skills/tdd (from skills/tdd)\n");
}

#[test]
fn an_intermediate_symlinked_directory_is_refused() {
    let world = World::fixture();
    let outside = world.tmp.path().join("elsewhere");
    std::fs::create_dir_all(outside.join("rules")).unwrap();
    std::os::unix::fs::symlink(&outside, world.project.join(".opencode")).unwrap();

    world
        .sync()
        .fails()
        .stderr_contains("refusing to write through the symlinked directory");
}

// =============================================== determinism and CLI

#[test]
fn rendering_is_deterministic_across_runs() {
    let world = World::fixture();
    let first = world.tmp.path().join("one");
    let second = world.tmp.path().join("two");
    world.render(&first).ok();
    world.render(&second).ok();

    let mut paths: Vec<String> = walk_files(first.clone())
        .into_iter()
        .map(|p| {
            p.strip_prefix(&first)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    for relative in paths {
        assert_eq!(
            std::fs::read(first.join(&relative)).unwrap(),
            std::fs::read(second.join(&relative)).unwrap(),
            "{relative}"
        );
    }
}

/// The registry compiled into the binary, rendered by a fixed config, has to
/// produce exactly these bytes. This is the "emitted bytes are a function of
/// the pinned inputs" guarantee, so an engine change can never quietly rewrite
/// what a consumer sees — and it is what a downloaded binary emits, with no
/// `assets/` beside it.
///
/// Editing an asset changes its hash: update this golden in the same commit.
/// That is not friction, it is the review.
#[test]
fn rendering_the_shipped_registry_is_pinned() {
    let expected = serde_json::json!([
        {
            "path": ".opencode/agents/code-reviewer.md",
            "files": [{
                "path": ".opencode/agents/code-reviewer.md",
                "hash": "sha256:0fddf45e707801e803a3ce9a12a7b10a2316245268a71bd59f4b45725d421920"
            }]
        },
        {
            "path": ".opencode/rules/conventional-commits.md",
            "files": [{
                "path": ".opencode/rules/conventional-commits.md",
                "hash": "sha256:b26cde4ab6edf18741b527a0bc67e2dc3a28e04c311c2247e51eb94dd71e454a"
            }]
        },
        {
            "path": ".opencode/skills/tdd",
            "files": [
                {
                    "path": ".opencode/skills/tdd/SKILL.md",
                    "hash": "sha256:2b32fca3e4edcf17af8f6b6423f7c36b1d5a4193d032c6916d033c4e054348f4"
                },
                {
                    "path": ".opencode/skills/tdd/checklist.md",
                    "hash": "sha256:b388c3b1eed95d38a7d2720030e24a04761651eb70cb2d4d8866f6bc3e3f0e02"
                }
            ]
        }
    ]);

    let tmp = tempfile::TempDir::new().unwrap();
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("agent-assets.json"),
        r#"{
            "rules": {"root": "./.opencode/rules", "imports": ["conventional-commits"]},
            "skills": {"root": "./.opencode/skills", "imports": ["tdd"]},
            "agents": {"root": "./.opencode/agents", "imports": ["code-reviewer"]}
        }"#,
    )
    .unwrap();

    let mut command = std::process::Command::cargo_bin("agent-assets").unwrap();
    command
        .args(["sync", "--project"])
        .arg(&project)
        .args(["--config"])
        .arg(project.join("agent-assets.json"))
        .args(["--manifest", "agent-assets.lock"]);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.join("agent-assets.lock")).unwrap())
            .unwrap();
    assert_eq!(manifest, expected);
}

#[test]
fn help_and_version_succeed() {
    let world = World::new();
    world.run(&["--help"]).ok();
    world.run(&["--version"]).ok();
}

#[test]
fn a_missing_config_is_an_error() {
    let world = World::new();
    world
        .run(&["check"])
        .fails()
        .stderr_contains("config not found");
}

#[test]
fn the_project_root_is_discovered_by_walking_up() {
    let world = World::fixture();
    let nested = world.project.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();

    // No `--project` and no `--config`: the engine must find them the way
    // git finds its repository root.
    let mut command = std::process::Command::cargo_bin("agent-assets").unwrap();
    command
        .current_dir(&nested)
        .args(["sync", "--assets"])
        .arg(&world.registry);
    let output = command.output().unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("WRITE"),
        "discovery should have found {}: {}",
        world.project.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unknown_flags_are_rejected() {
    let world = World::new();
    world.run(&["check", "--nope"]).fails();
}

// ============================================================= inspection

/// `list` reads the registry and nothing else, so its output is a function
/// of the registry alone.
#[test]
fn list_shows_every_asset_and_the_totals() {
    let world = World::fixture();
    let total = BUILD.len() + CODE_STYLE.len() + SECURITY.len() + SKILL_MD.len() + CHECKLIST.len();
    world.run(&["list"]).ok().stdout_is(&format!(
        "\
agents/build      file  {:>6}  Builder.
rules/code-style  file  {:>6}  Prefer clarity.
rules/security    file  {:>6}  Keep secrets safe.
skills/tdd        dir   {:>6}  Test first.
4 assets · {total} bytes · agents: 1, rules: 2, skills: 1
",
        BUILD.len(),
        CODE_STYLE.len(),
        SECURITY.len(),
        SKILL_MD.len() + CHECKLIST.len(),
    ));
}

/// After `cargo install` there is no `assets/` anywhere, and `list` still
/// works: the registry is in the binary.
#[test]
fn list_uses_the_embedded_registry_without_any_setup() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut command = std::process::Command::cargo_bin("agent-assets").unwrap();
    command.current_dir(tmp.path()).arg("list");
    let output = command.output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skills/tdd"), "{stdout}");
    assert!(stdout.contains("rules/conventional-commits"), "{stdout}");
}

#[test]
fn show_prints_a_file_asset_verbatim() {
    let world = World::fixture();
    world
        .run(&["show", "rules/security"])
        .ok()
        .stdout_is(SECURITY);
}

#[test]
fn show_prints_a_directory_asset_behind_headers() {
    let world = World::fixture();
    world.run(&["show", "skills/tdd"]).ok().stdout_is(&format!(
        "==> SKILL.md <==\n{SKILL_MD}\n==> checklist.md <==\n{CHECKLIST}\n"
    ));
}

/// The rendered form is what `sync` would write, resolved by the planner:
/// the category's frontmatter with the import's on top.
#[test]
fn show_rendered_applies_the_injected_frontmatter() {
    let world = World::fixture();
    world
        .run(&["show", "--rendered", "rules/security"])
        .ok()
        .stdout_is(&format!("{RULES_FM}{SECURITY}"));
    world
        .run(&["show", "--rendered", "rules/code-style"])
        .ok()
        .stdout_is(&format!("{STYLE_FM}{CODE_STYLE}"));
}

/// An asset emitted twice is two answers, not an error: `show --rendered`
/// shows every file `sync` would write for it, each named by where it lands.
#[test]
fn show_rendered_prints_every_emitted_variant() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["imports"] = json!([
        {"name": "security", "rename": "sec-a",
         "injections": {"frontmatter": {"owner": "a"}}},
        {"name": "security", "rename": "sec-b",
         "injections": {"frontmatter": {"owner": "b"}}}
    ]);
    world.write_config(&config);

    world
        .run(&["show", "--rendered", "rules/security"])
        .ok()
        .stdout_is(&format!(
            "==> .opencode/rules/sec-a.md <==\n---\nowner: a\nteam: platform\n---\n\n{SECURITY}\n\
             ==> .opencode/rules/sec-b.md <==\n---\nowner: b\nteam: platform\n---\n\n{SECURITY}\n"
        ));
}

/// The header names where the file lands, not what the registry calls it —
/// that is the question `--rendered` answers.
#[test]
fn show_rendered_names_where_each_file_lands() {
    let world = World::fixture();
    world
        .run(&["show", "--rendered", "skills/tdd"])
        .ok()
        .stdout_is(&format!(
            "==> .opencode/skills/tdd/SKILL.md <==\n{SKILL_MD}\n\
             ==> .opencode/skills/tdd/checklist.md <==\n{CHECKLIST}\n"
        ));
}

/// `sync` writes nothing for an asset nobody imports, so neither does the
/// preview of it.
#[test]
fn show_rendered_refuses_an_asset_that_is_not_imported() {
    let world = World::fixture();
    world.write_asset("rules/pacing.md", "## Summary\n\nSlow down.\n");
    world
        .run(&["show", "--rendered", "rules/pacing"])
        .fails()
        .stderr_contains("not imported");
}

/// The source is in the binary; the rendered form depends on the project's
/// injections, so only that one needs a config.
#[test]
fn show_needs_a_config_only_for_the_rendered_form() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut bare = std::process::Command::cargo_bin("agent-assets").unwrap();
    bare.current_dir(tmp.path())
        .args(["show", "rules/conventional-commits"]);
    assert!(bare.output().unwrap().status.success());

    let mut rendered = std::process::Command::cargo_bin("agent-assets").unwrap();
    rendered
        .current_dir(tmp.path())
        .args(["show", "--rendered", "rules/conventional-commits"]);
    let output = rendered.output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("config not found"),
        "{:?}",
        output.stderr
    );
}

#[test]
fn show_rejects_a_target_that_is_not_a_category_and_a_name() {
    let world = World::fixture();
    world
        .run(&["show", "rules"])
        .fails()
        .stderr_contains("expected <category>/<name>");
}

/// A category's `README.md` documents the category; it is not an asset.
#[test]
fn a_catalog_index_is_not_an_asset() {
    let world = World::fixture();
    world.write_asset("rules/README.md", "# Rules\n");

    let listed = world.run(&["list"]);
    listed.ok();
    assert!(!listed.stdout.contains("README"), "{:?}", listed.stdout);

    world
        .run(&["show", "rules/README"])
        .fails()
        .stderr_contains("catalog index");

    world.write_config(&json!({"rules": {"root": "./r", "imports": ["README"]}}));
    world.sync().fails().stderr_contains("catalog index");
}

/// `status` is `check` without the filter, and always exits 0 — the state of
/// the tree is information, not an error.
#[test]
fn status_reports_every_path_and_always_exits_zero() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/security.md", "local hack\n");

    world.run(&["status"]).ok().stdout_is(
        "\
CLEAN .opencode/agents/build.md (from agents/build)
CLEAN .opencode/rules/base/style.md (from rules/code-style)
CONFLICT .opencode/rules/security.md (from rules/security)
CLEAN .opencode/skills/tdd/SKILL.md (from skills/tdd)
CLEAN .opencode/skills/tdd/checklist.md (from skills/tdd)
4 imports · 3 clean · 1 conflict · 0 custom files
",
    );
}

/// A directory asset is one import however many files it carries, so five
/// `MISSING` lines tally as four. The pending injection is a path-level fact
/// rather than a state of any import, so it is counted apart.
#[test]
fn status_tallies_imports_not_files() {
    let world = World::fixture();
    world.run(&["status"]).ok().stdout_is(
        "\
MISSING .opencode/agents/build.md (from agents/build)
MISSING .opencode/rules/base/style.md (from rules/code-style)
MISSING .opencode/rules/security.md (from rules/security)
MISSING .opencode/skills/tdd/SKILL.md (from skills/tdd)
MISSING .opencode/skills/tdd/checklist.md (from skills/tdd)
INJECT opencode.json
4 imports · 0 clean · 4 missing · 1 injection · 0 custom files
",
    );
}

/// Files nobody owns are never touched — and `status` says how many there
/// are, so a reader can tell ours from theirs at a glance.
#[test]
fn status_counts_custom_files_alongside_ours() {
    let world = World::fixture();
    world.sync().ok();
    world.write_project(".opencode/rules/custom.md", "mine\n");
    world.write_project(&skill("my-own"), "mine\n");

    let run = world.run(&["status"]);
    run.ok();
    assert!(
        run.stdout
            .ends_with("4 imports · 4 clean · 2 custom files\n"),
        "{:?}",
        run.stdout
    );
}

// ============================================================= validate

/// The fixture assets, given the sections their schema requires. The fixture
/// itself only needs names to exist; `validate` reads the whole document, so
/// a test that wants silence has to give them their sections too.
fn conform(world: &World) {
    world.write_asset(
        "rules/security.md",
        "## Summary\n\nKeep secrets safe.\n\n## Guidelines\n\n- A.\n",
    );
    world.write_asset(
        "rules/code-style.md",
        "## Summary\n\nPrefer clarity.\n\n## Guidelines\n\n- A.\n",
    );
    world.write_asset(
        "skills/tdd/SKILL.md",
        "## Description\n\nTest first.\n\n## Steps\n\n1. Do it.\n",
    );
    world.write_asset(
        "agents/build.md",
        "## Role\n\nBuilder.\n\n## Instructions\n\nBuild.\n",
    );
}

#[test]
fn validate_is_silent_when_the_config_and_registry_are_sound() {
    let world = World::fixture();
    conform(&world);
    world.run(&["validate"]).ok().stdout_is("");
}

#[test]
fn validate_reports_every_config_problem_at_once() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["imports"] = json!(["missing-one", "missing-two"]);
    world.write_config(&config);

    let run = world.run(&["validate"]);
    run.fails();
    assert!(run.stdout.contains("missing-one"), "{}", run.stdout);
    assert!(run.stdout.contains("missing-two"), "{}", run.stdout);
}

#[test]
fn validate_reports_a_registry_asset_that_breaks_its_sections() {
    let world = World::fixture();
    conform(&world);
    world.write_asset("rules/broken.md", "## Summary\n\nNo guidelines.\n");

    let run = world.run(&["validate"]);
    run.fails();
    assert!(run.stdout.contains("rules/broken"), "{}", run.stdout);
    assert!(
        run.stdout.contains("missing required section 'Guidelines'"),
        "{}",
        run.stdout
    );
}

#[test]
fn validate_never_modifies_the_project() {
    let world = World::fixture();
    conform(&world);
    world.sync().ok();
    let before = world.read_project(".opencode/rules/security.md");

    world.run(&["validate"]).ok();

    assert_eq!(world.read_project(".opencode/rules/security.md"), before);
}

/// The config is sound on its own; the file it aims at is not JSON. That is
/// still a reason the run would stop, so `validate` says so before `sync`
/// discovers it in the middle of writing.
#[test]
fn validate_reports_an_injection_target_that_is_not_json() {
    let world = World::fixture();
    world.write_project("opencode.json", "not json\n");

    let run = world.run(&["validate"]);
    run.fails();
    assert!(run.stdout.contains("is not valid JSON"), "{}", run.stdout);
}

#[test]
fn sync_refuses_a_bad_injection_target() {
    let world = World::fixture();
    world.write_project("opencode.json", "not json\n");

    let run = world.sync();
    run.fails();
    assert!(run.stderr.contains("is not valid JSON"), "{}", run.stderr);
}

/// The pre-flight gates on config alone: every reason at once, on stderr,
/// before anything is written. Registry problems do not stop a sync.
#[test]
fn sync_refuses_a_bad_config_with_every_reason_at_once() {
    let world = World::fixture();
    let mut config = base_config();
    config["rules"]["imports"] = json!(["missing-one", "missing-two"]);
    world.write_config(&config);

    let run = world.sync();
    run.fails();
    assert!(run.stderr.contains("missing-one"), "{}", run.stderr);
    assert!(run.stderr.contains("missing-two"), "{}", run.stderr);
    assert!(!world.exists(".opencode/rules/security.md"));
}

// ---------------------------------------------------------------- helpers
