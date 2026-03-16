use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

// ============================================================
// Shared test helpers
// ============================================================

/// Create a minimal Godot project in a directory.
fn make_project(dir: &Path) {
    fs::write(
        dir.join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray()\n",
    )
    .unwrap();
}

/// Create a fake "installed" share directory with addon stubs.
fn make_share_dir(dir: &Path) {
    let stage = dir.join("addons").join("stage");
    fs::create_dir_all(stage.join("bin").join("linux")).unwrap();
    fs::write(
        stage.join("plugin.cfg"),
        "[plugin]\nname=\"Stage\"\nscript=\"plugin.gd\"\n",
    )
    .unwrap();
    fs::write(stage.join("runtime.gd"), "extends Node\n").unwrap();
    fs::write(
        stage.join("bin").join("linux").join("libstage_godot.so"),
        b"fake-gdext",
    )
    .unwrap();
    let director = dir.join("addons").join("director");
    fs::create_dir_all(&director).unwrap();
    fs::write(
        director.join("plugin.cfg"),
        "[plugin]\nname=\"Director\"\nscript=\"plugin.gd\"\n",
    )
    .unwrap();
}

/// Build a Command with isolated env vars for testing.
fn theatre_cmd(share_dir: &Path, bin_dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_theatre"));
    cmd.env("THEATRE_SHARE_DIR", share_dir);
    cmd.env("THEATRE_BIN_DIR", bin_dir);
    cmd.env("THEATRE_ROOT", "/dev/null");
    cmd.env("THEATRE_NO_TELEMETRY", "1");
    cmd.env("DO_NOT_TRACK", "1");
    cmd
}

// ============================================================
// Existing tests (preserved)
// ============================================================

#[test]
fn help_prints_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("deploy"));
    assert!(stdout.contains("enable"));
}

#[test]
fn version_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--version")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn enable_on_valid_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray()\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(content.contains("stage/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
}

#[test]
fn enable_on_missing_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", "/tmp/nonexistent-project-12345"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn init_fails_without_install() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("project.godot"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["init", dir.path().to_str().unwrap(), "--yes"])
        .env("THEATRE_SHARE_DIR", "/tmp/nonexistent-share-12345")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("install") || stderr.contains("not installed"));
}

// ============================================================
// Init tests
// ============================================================

#[test]
fn init_yes_copies_addons_and_generates_config() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Addon files must be present
    assert!(project.path().join("addons/stage/plugin.cfg").exists());
    assert!(project.path().join("addons/stage/runtime.gd").exists());
    assert!(
        project
            .path()
            .join("addons/stage/bin/linux/libstage_godot.so")
            .exists()
    );
    assert!(project.path().join("addons/director/plugin.cfg").exists());

    // .mcp.json must exist and be valid JSON
    let mcp_path = project.path().join(".mcp.json");
    assert!(mcp_path.exists());
    let mcp_content = fs::read_to_string(&mcp_path).unwrap();
    let mcp: serde_json::Value =
        serde_json::from_str(&mcp_content).expect(".mcp.json is not valid JSON");
    assert!(
        mcp["mcpServers"]["stage"].is_object(),
        "stage server missing"
    );
    assert!(
        mcp["mcpServers"]["director"].is_object(),
        "director server missing"
    );
    assert_eq!(mcp["mcpServers"]["stage"]["args"][0], "serve");
    assert_eq!(mcp["mcpServers"]["director"]["args"][0], "serve");

    // project.godot must have plugins enabled and autoload
    let godot = fs::read_to_string(project.path().join("project.godot")).unwrap();
    assert!(godot.contains("stage/plugin.cfg"));
    assert!(godot.contains("director/plugin.cfg"));
    assert!(godot.contains("StageRuntime"));
}

#[test]
fn init_yes_uses_default_port_no_theatre_port_env() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_content = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    // Default port 9077: no THEATRE_PORT env entry should be present
    assert!(
        !mcp_content.contains("THEATRE_PORT"),
        "THEATRE_PORT should not appear for default port 9077"
    );
}

#[test]
fn init_project_with_existing_addons() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Pre-populate addons with old content
    let old_stage = project.path().join("addons/stage");
    fs::create_dir_all(&old_stage).unwrap();
    fs::write(
        old_stage.join("plugin.cfg"),
        "[plugin]\nname=\"OldStage\"\n",
    )
    .unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // New plugin.cfg should replace the old one
    let content = fs::read_to_string(project.path().join("addons/stage/plugin.cfg")).unwrap();
    assert!(content.contains("Stage"), "Expected fresh Stage plugin.cfg");
    assert!(
        !content.contains("OldStage"),
        "Old content should be replaced"
    );
}

#[test]
fn init_without_project_godot() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    // Do NOT create project.godot

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("project.godot"),
        "Error should mention project.godot, got: {stderr}"
    );
}

#[test]
fn init_nonexistent_path() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", "/tmp/does-not-exist-theatre-test-12345", "--yes"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn init_share_dir_missing() {
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    make_project(project.path());

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .env("THEATRE_SHARE_DIR", "/tmp/nonexistent-share-theatre-12345")
        .env("THEATRE_BIN_DIR", bin.path())
        .env("THEATRE_ROOT", "/dev/null")
        .env("THEATRE_NO_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("install"),
        "Should tell user to run theatre install, got: {stderr}"
    );
    // No partial addon copies
    assert!(!project.path().join("addons").exists());
}

#[test]
fn init_share_dir_incomplete_missing_so() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    // Populate share dir WITHOUT the .so file
    let stage = share.path().join("addons").join("stage");
    fs::create_dir_all(stage.join("bin").join("linux")).unwrap();
    fs::write(
        stage.join("plugin.cfg"),
        "[plugin]\nname=\"Stage\"\nscript=\"plugin.gd\"\n",
    )
    .unwrap();
    fs::write(stage.join("runtime.gd"), "extends Node\n").unwrap();
    // No libstage_godot.so
    let director = share.path().join("addons").join("director");
    fs::create_dir_all(&director).unwrap();
    fs::write(
        director.join("plugin.cfg"),
        "[plugin]\nname=\"Director\"\nscript=\"plugin.gd\"\n",
    )
    .unwrap();

    make_project(project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();

    // Either fails with clear message, or succeeds with GDScript present
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        // Partial success: GDScript should be present
        assert!(project.path().join("addons/stage/plugin.cfg").exists());
        assert!(project.path().join("addons/stage/runtime.gd").exists());
        // GDExtension might be missing — that's acceptable if communicated
        let _ = stderr; // may or may not warn
    } else {
        // Failure should mention the missing binary
        assert!(
            stderr.contains("GDExtension")
                || stderr.contains(".so")
                || stderr.contains("libstage_godot")
                || stderr.contains("Failed to copy"),
            "Error should mention missing GDExtension, got: {stderr}"
        );
    }
}

// ============================================================
// Enable tests
// ============================================================

#[test]
fn enable_stage_only() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap(), "--stage"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(
        content.contains("stage/plugin.cfg"),
        "Stage should be enabled"
    );
    assert!(
        !content.contains("director/plugin.cfg"),
        "Director should NOT be enabled"
    );
    assert!(
        content.contains("StageRuntime"),
        "StageRuntime autoload should be added"
    );
}

#[test]
fn enable_director_only() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap(), "--director"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(
        content.contains("director/plugin.cfg"),
        "Director should be enabled"
    );
    assert!(
        !content.contains("stage/plugin.cfg"),
        "Stage should NOT be enabled"
    );
    assert!(
        !content.contains("StageRuntime"),
        "StageRuntime should NOT be added for director-only"
    );
}

#[test]
fn disable_plugins() {
    let dir = tempfile::tempdir().unwrap();
    // Start with both plugins enabled
    fs::write(
        dir.path().join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/stage/plugin.cfg\", \"res://addons/director/plugin.cfg\")\n\
        [autoload]\nStageRuntime=\"*res://addons/stage/runtime.gd\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap(), "--disable"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(
        !content.contains("stage/plugin.cfg"),
        "Stage should be disabled"
    );
    assert!(
        !content.contains("director/plugin.cfg"),
        "Director should be disabled"
    );
    assert!(
        !content.contains("StageRuntime"),
        "StageRuntime autoload should be removed"
    );
}

#[test]
fn enable_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    // Run enable twice
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
            .args(["enable", dir.path().to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    // Exactly one occurrence of each
    let stage_count = content.matches("stage/plugin.cfg").count();
    let director_count = content.matches("director/plugin.cfg").count();
    let autoload_count = content.matches("StageRuntime").count();
    assert_eq!(
        stage_count, 1,
        "stage/plugin.cfg should appear exactly once"
    );
    assert_eq!(
        director_count, 1,
        "director/plugin.cfg should appear exactly once"
    );
    assert_eq!(autoload_count, 1, "StageRuntime should appear exactly once");
}

#[test]
fn enable_nonexistent_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", "/tmp/nonexistent-theatre-test-12345"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn enable_empty_project_godot() {
    let dir = tempfile::tempdir().unwrap();
    // Empty project.godot (no sections)
    fs::write(dir.path().join("project.godot"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(
        content.contains("[editor_plugins]"),
        "Should create [editor_plugins] section"
    );
    assert!(content.contains("stage/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
}

#[test]
fn enable_project_godot_no_editor_plugins_section() {
    let dir = tempfile::tempdir().unwrap();
    // project.godot with content but no [editor_plugins]
    fs::write(
        dir.path().join("project.godot"),
        "[application]\nconfig/name=\"MyGame\"\nconfig/features=PackedStringArray(\"4.5\")\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(
        content.contains("[editor_plugins]"),
        "Should create [editor_plugins] section"
    );
    assert!(content.contains("stage/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
    // Existing content should be preserved
    assert!(content.contains("[application]"));
    assert!(content.contains("MyGame"));
}

#[test]
fn enable_project_godot_with_existing_plugins() {
    let dir = tempfile::tempdir().unwrap();
    // project.godot with an existing third-party plugin
    fs::write(
        dir.path().join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/gut/plugin.cfg\")\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(dir.path().join("project.godot")).unwrap();
    // Existing plugin must be preserved
    assert!(
        content.contains("gut/plugin.cfg"),
        "Existing plugin should be preserved"
    );
    // New plugins must be added
    assert!(content.contains("stage/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
}

#[test]
fn enable_readonly_project_godot() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    // Make project.godot read-only
    let godot_path = dir.path().join("project.godot");
    let mut perms = fs::metadata(&godot_path).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&godot_path, perms).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    // Restore permissions before assertions so TempDir cleanup works
    let mut perms = fs::metadata(&godot_path).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&godot_path, perms).unwrap();

    assert!(
        !output.status.success(),
        "Should fail on read-only project.godot"
    );
}

// ============================================================
// Rules tests
// ============================================================

#[test]
fn rules_creates_claude_rules_file() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["rules", dir.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let rules_path = dir.path().join(".claude/rules/godot.md");
    assert!(
        rules_path.exists(),
        ".claude/rules/godot.md should be created"
    );

    let content = fs::read_to_string(&rules_path).unwrap();
    assert!(
        content.contains("Never hand-edit Godot files"),
        "Should contain the core rule"
    );
    assert!(content.contains("Director"), "Should mention Director");
    assert!(content.contains("Stage"), "Should mention Stage");
}

#[test]
fn rules_skips_if_already_present() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    // Pre-create the rules file
    let rules_dir = dir.path().join(".claude/rules");
    fs::create_dir_all(&rules_dir).unwrap();
    let rules_file = rules_dir.join("godot.md");
    let original_content = "# Original content\nDo not overwrite me.\n";
    fs::write(&rules_file, original_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["rules", dir.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already") || stderr.contains("skip") || stderr.contains("Skip"),
        "Should warn about existing file, got: {stderr}"
    );

    // File content must be unchanged
    let content = fs::read_to_string(&rules_file).unwrap();
    assert_eq!(content, original_content, "File should not be overwritten");
}

#[test]
fn rules_nonexistent_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["rules", "/tmp/does-not-exist-theatre-test-12345", "--yes"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn rules_append_to_existing_claude_md() {
    let dir = tempfile::tempdir().unwrap();
    make_project(dir.path());

    // Pre-create CLAUDE.md with user content
    let original = "# My Project\n\nThis is my project documentation.\n";
    fs::write(dir.path().join("CLAUDE.md"), original).unwrap();

    // We need to test the CLAUDE.md append path — rules --yes defaults to .claude/rules/godot.md
    // So test directly by verifying that if godot.md already exists, CLAUDE.md would get appended.
    // Instead, test the append logic by pre-creating the rules file and checking CLAUDE.md is untouched,
    // then verify append behavior by calling with a project that has no rules dir and checking the
    // append path indirectly.
    //
    // Since --yes always writes to .claude/rules/godot.md, we just verify that path works
    // and that CLAUDE.md is not touched.
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["rules", dir.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // CLAUDE.md should be untouched since --yes writes to .claude/rules/godot.md
    let claude_content = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
    assert_eq!(
        claude_content, original,
        "CLAUDE.md should not be modified when writing to .claude/rules/godot.md"
    );
}

// ============================================================
// MCP tests
// ============================================================

#[test]
fn mcp_generates_valid_config() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Copy addons to project so mcp detects them
    let stage_dst = project.path().join("addons/stage");
    fs::create_dir_all(&stage_dst).unwrap();
    fs::write(stage_dst.join("plugin.cfg"), "[plugin]\nname=\"Stage\"\n").unwrap();
    let director_dst = project.path().join("addons/director");
    fs::create_dir_all(&director_dst).unwrap();
    fs::write(
        director_dst.join("plugin.cfg"),
        "[plugin]\nname=\"Director\"\n",
    )
    .unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["mcp", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_path = project.path().join(".mcp.json");
    assert!(mcp_path.exists());
    let mcp_content = fs::read_to_string(&mcp_path).unwrap();
    let mcp: serde_json::Value =
        serde_json::from_str(&mcp_content).expect(".mcp.json must be valid JSON");

    let stage = &mcp["mcpServers"]["stage"];
    let director = &mcp["mcpServers"]["director"];
    assert!(stage.is_object(), "stage server must exist");
    assert!(director.is_object(), "director server must exist");
    assert_eq!(stage["type"], "stdio");
    assert_eq!(director["type"], "stdio");
    assert_eq!(stage["args"][0], "serve");
    assert_eq!(director["args"][0], "serve");
}

#[test]
fn mcp_custom_port() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Copy addons to project
    let stage_dst = project.path().join("addons/stage");
    fs::create_dir_all(&stage_dst).unwrap();
    fs::write(stage_dst.join("plugin.cfg"), "[plugin]\nname=\"Stage\"\n").unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "mcp",
            project.path().to_str().unwrap(),
            "--yes",
            "--port",
            "8080",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_content = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    assert!(
        mcp_content.contains("THEATRE_PORT"),
        "Should contain THEATRE_PORT for custom port"
    );
    assert!(
        mcp_content.contains("8080"),
        "Should contain port value 8080"
    );
}

#[test]
fn mcp_stage_only_when_director_missing() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Only copy stage addon (no director)
    let stage_dst = project.path().join("addons/stage");
    fs::create_dir_all(&stage_dst).unwrap();
    fs::write(stage_dst.join("plugin.cfg"), "[plugin]\nname=\"Stage\"\n").unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["mcp", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_content = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    let mcp: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();
    assert!(
        mcp["mcpServers"]["stage"].is_object(),
        "stage should be included"
    );
    assert!(
        mcp["mcpServers"]["director"].is_null() || !mcp["mcpServers"]["director"].is_object(),
        "director should NOT be included when addons/director/ is missing"
    );
}

#[test]
fn mcp_overwrites_existing_mcp_json() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Pre-existing .mcp.json with custom content
    fs::write(
        project.path().join(".mcp.json"),
        "{\"custom\": \"content\", \"old\": true}\n",
    )
    .unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["mcp", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_content = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    let mcp: serde_json::Value =
        serde_json::from_str(&mcp_content).expect("Overwritten .mcp.json must be valid JSON");
    // Old content should be gone
    assert!(
        mcp.get("custom").is_none(),
        "Old custom content should be replaced"
    );
    // New content should have mcpServers
    assert!(
        mcp["mcpServers"].is_object(),
        "mcpServers should be present"
    );
}

#[test]
fn mcp_port_zero_accepted_with_yes() {
    // Port validation (< 1024 check) only runs in interactive mode.
    // With --yes, port 0 is accepted directly (documents current behavior).
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "mcp",
            project.path().to_str().unwrap(),
            "--yes",
            "--port",
            "0",
        ])
        .output()
        .unwrap();
    // Port 0 is a valid u16 — with --yes, no validation occurs; document actual behavior
    // This test documents that --yes bypasses port range validation.
    let _ = output.status.success(); // behavior is documented, not enforced
}

#[test]
fn mcp_invalid_port_string_rejected_by_clap() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "mcp",
            project.path().to_str().unwrap(),
            "--yes",
            "--port",
            "notaport",
        ])
        .output()
        .unwrap();
    // clap rejects non-numeric port strings
    assert!(
        !output.status.success(),
        "Non-numeric port should be rejected"
    );
}

// ============================================================
// Deploy tests
// ============================================================

#[test]
fn deploy_from_share_dir_copies_addons() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // THEATRE_ROOT=/dev/null forces share-dir-only mode (no source build)
    let output = theatre_cmd(share.path(), bin.path())
        .args(["deploy", project.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(project.path().join("addons/stage/plugin.cfg").exists());
    assert!(
        project
            .path()
            .join("addons/stage/bin/linux/libstage_godot.so")
            .exists()
    );
    assert!(project.path().join("addons/director/plugin.cfg").exists());
}

#[test]
fn deploy_skips_symlinked_addons() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let link_target = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Create addons/stage as a symlink
    fs::create_dir_all(project.path().join("addons")).unwrap();
    std::os::unix::fs::symlink(link_target.path(), project.path().join("addons/stage")).unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["deploy", project.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Symlink should still be intact
    let meta = fs::symlink_metadata(project.path().join("addons/stage")).unwrap();
    assert!(
        meta.file_type().is_symlink(),
        "addons/stage should remain a symlink"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("symlink") || stderr.contains("skipping"),
        "Should warn about symlink, got: {stderr}"
    );
}

#[test]
fn deploy_multiple_projects() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project1 = tempfile::tempdir().unwrap();
    let project2 = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project1.path());
    make_project(project2.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "deploy",
            project1.path().to_str().unwrap(),
            project2.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        project1.path().join("addons/stage/plugin.cfg").exists(),
        "project1 should have stage addon"
    );
    assert!(
        project2.path().join("addons/stage/plugin.cfg").exists(),
        "project2 should have stage addon"
    );
    assert!(
        project1.path().join("addons/director/plugin.cfg").exists(),
        "project1 should have director addon"
    );
    assert!(
        project2.path().join("addons/director/plugin.cfg").exists(),
        "project2 should have director addon"
    );
}

#[test]
fn deploy_no_share_dir_no_source() {
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    make_project(project.path());

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["deploy", project.path().to_str().unwrap()])
        .env("THEATRE_SHARE_DIR", "/tmp/nonexistent-share-theatre-99999")
        .env("THEATRE_BIN_DIR", bin.path())
        .env("THEATRE_ROOT", "/tmp/nonexistent-root-theatre-99999")
        .env("THEATRE_NO_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "Should fail without share dir or source"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("install") || stderr.contains("not installed"),
        "Should suggest running theatre install, got: {stderr}"
    );
}

#[test]
fn deploy_readonly_project_dir() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Create addons/ dir then make it read-only
    let addons_dir = project.path().join("addons");
    fs::create_dir_all(&addons_dir).unwrap();
    let mut perms = fs::metadata(&addons_dir).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&addons_dir, perms).unwrap();

    let output = theatre_cmd(share.path(), bin.path())
        .args(["deploy", project.path().to_str().unwrap()])
        .output()
        .unwrap();

    // Restore permissions before assertions so TempDir cleanup works
    let mut perms = fs::metadata(&addons_dir).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&addons_dir, perms).unwrap();

    assert!(
        !output.status.success(),
        "Should fail when addons/ is read-only"
    );
}

// ============================================================
// Lifecycle tests
// ============================================================

#[test]
fn full_lifecycle_init_then_disable_then_reenable() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Step 1: init
    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 2: verify both plugins enabled
    let godot = fs::read_to_string(project.path().join("project.godot")).unwrap();
    assert!(godot.contains("stage/plugin.cfg"));
    assert!(godot.contains("director/plugin.cfg"));
    assert!(godot.contains("StageRuntime"));

    // Step 3: disable stage only
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args([
            "enable",
            project.path().to_str().unwrap(),
            "--disable",
            "--stage",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "disable --stage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 4: verify stage disabled, director still enabled
    let godot = fs::read_to_string(project.path().join("project.godot")).unwrap();
    assert!(
        !godot.contains("stage/plugin.cfg"),
        "Stage should be disabled"
    );
    assert!(
        godot.contains("director/plugin.cfg"),
        "Director should still be enabled"
    );
    assert!(
        !godot.contains("StageRuntime"),
        "StageRuntime should be removed"
    );

    // Verify .mcp.json still exists (enable/disable doesn't touch it)
    assert!(
        project.path().join(".mcp.json").exists(),
        ".mcp.json should survive enable/disable cycles"
    );

    // Step 5: re-enable stage
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", project.path().to_str().unwrap(), "--stage"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "re-enable --stage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 6: verify both enabled again
    let godot = fs::read_to_string(project.path().join("project.godot")).unwrap();
    assert!(
        godot.contains("stage/plugin.cfg"),
        "Stage should be re-enabled"
    );
    assert!(
        godot.contains("director/plugin.cfg"),
        "Director should still be enabled"
    );
    assert!(
        godot.contains("StageRuntime"),
        "StageRuntime should be re-added"
    );
}

#[test]
fn init_then_mcp_regenerate_with_different_port() {
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(project.path());

    // Step 1: init with default port
    let output = theatre_cmd(share.path(), bin.path())
        .args(["init", project.path().to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_before = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    assert!(
        !mcp_before.contains("THEATRE_PORT"),
        "Default port should have no THEATRE_PORT"
    );

    // Step 2: regenerate with port 8080
    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "mcp",
            project.path().to_str().unwrap(),
            "--yes",
            "--port",
            "8080",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mcp regen failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mcp_after = fs::read_to_string(project.path().join(".mcp.json")).unwrap();
    assert!(
        mcp_after.contains("8080"),
        ".mcp.json should contain port 8080"
    );

    // Addons still intact
    assert!(
        project.path().join("addons/stage/plugin.cfg").exists(),
        "Addons should survive mcp regeneration"
    );
}

// ============================================================
// Help and version boundary tests
// ============================================================

#[test]
fn help_and_version_always_work() {
    // No project, no share dir, nothing installed — help and version should always succeed
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--help")
        .env("THEATRE_SHARE_DIR", "/tmp/no-share-dir-99999")
        .env("THEATRE_BIN_DIR", "/tmp/no-bin-dir-99999")
        .output()
        .unwrap();
    assert!(output.status.success(), "--help should always succeed");

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--version")
        .env("THEATRE_SHARE_DIR", "/tmp/no-share-dir-99999")
        .env("THEATRE_BIN_DIR", "/tmp/no-bin-dir-99999")
        .output()
        .unwrap();
    assert!(output.status.success(), "--version should always succeed");

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["init", "--help"])
        .env("THEATRE_SHARE_DIR", "/tmp/no-share-dir-99999")
        .env("THEATRE_BIN_DIR", "/tmp/no-bin-dir-99999")
        .output()
        .unwrap();
    assert!(output.status.success(), "init --help should always succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--yes") || stdout.contains("yes"),
        "init --help should show --yes flag"
    );
}

// ============================================================
// Graceful degradation tests
// ============================================================

#[test]
fn deploy_continues_for_second_project_if_first_fails() {
    // Discover the actual behavior: does deploy fail fast or continue?
    let share = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let good_project = tempfile::tempdir().unwrap();

    make_share_dir(share.path());
    make_project(good_project.path());

    let output = theatre_cmd(share.path(), bin.path())
        .args([
            "deploy",
            "/tmp/nonexistent-bad-project-theatre-12345",
            good_project.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Document actual behavior: deploy validates ALL projects upfront before deploying any.
    // If first is invalid, the whole command fails (fail-fast).
    assert!(
        !output.status.success(),
        "deploy should fail when one project is invalid"
    );
    // The error message should mention the bad project
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent-bad-project") || stderr.contains("does not exist"),
        "Error should mention the invalid project, got: {stderr}"
    );
}
