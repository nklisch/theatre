use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn project_reload_returns_script_count() {
    let f = DirectorFixture::new();
    let data = f.run("project_reload", json!({})).unwrap().unwrap_data();

    let count = data["scripts_checked"].as_u64().unwrap();
    assert!(
        count > 0,
        "test project should have at least one .gd script, got {count}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn project_reload_returns_autoloads_dict() {
    let f = DirectorFixture::new();
    let data = f.run("project_reload", json!({})).unwrap().unwrap_data();

    assert!(
        data["autoloads"].is_object(),
        "autoloads should be a dictionary object"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn project_reload_scripts_checked_is_integer() {
    let f = DirectorFixture::new();
    let data = f.run("project_reload", json!({})).unwrap().unwrap_data();

    assert!(
        data["scripts_checked"].as_u64().is_some(),
        "scripts_checked should be a non-negative integer, got: {:?}",
        data["scripts_checked"]
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_status_headless_returns_not_connected() {
    let f = DirectorFixture::new();
    let data = f.run("editor_status", json!({})).unwrap().unwrap_data();

    assert_eq!(
        data["editor_connected"], false,
        "headless mode should report editor_connected: false"
    );
    assert_eq!(
        data["active_scene"], "",
        "headless mode should have empty active_scene"
    );
    assert!(
        data["open_scenes"].as_array().unwrap().is_empty(),
        "headless mode should have empty open_scenes"
    );
    assert_eq!(
        data["game_running"], false,
        "headless mode should have game_running: false"
    );
    assert!(
        data["autoloads"].is_object(),
        "editor_status should return autoloads dict"
    );
    assert!(
        data["recent_log"].is_array(),
        "editor_status should return recent_log array"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_status_recent_log_contains_strings() {
    let f = DirectorFixture::new();
    let data = f.run("editor_status", json!({})).unwrap().unwrap_data();

    let log = data["recent_log"].as_array().unwrap();
    // Each entry must be a string (may be empty if no log file exists yet)
    for entry in log {
        assert!(
            entry.is_string(),
            "recent_log entries should be strings, got: {entry:?}"
        );
    }
}

#[test]
#[ignore = "requires Godot binary"]
fn autoload_add_succeeds_and_returns_fields() {
    let f = DirectorFixture::new();
    let name = "TestAutoload_add_unit";
    let script_path = "tmp/test_autoload_add.gd";

    let data = f
        .run(
            "autoload_add",
            json!({"name": name, "script_path": script_path, "enabled": true}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["name"], name);
    assert_eq!(data["script_path"], script_path);
    assert_eq!(data["enabled"], true);

    // Clean up: remove the autoload we just added
    f.run("autoload_remove", json!({"name": name}))
        .unwrap()
        .unwrap_data();
}

#[test]
#[ignore = "requires Godot binary"]
fn autoload_add_disabled_sets_enabled_false() {
    let f = DirectorFixture::new();
    let name = "TestAutoload_add_disabled";
    let script_path = "tmp/test_autoload_disabled.gd";

    let data = f
        .run(
            "autoload_add",
            json!({"name": name, "script_path": script_path, "enabled": false}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["name"], name);
    assert_eq!(data["enabled"], false);

    // Clean up
    f.run("autoload_remove", json!({"name": name}))
        .unwrap()
        .unwrap_data();
}

#[test]
#[ignore = "requires Godot binary"]
fn autoload_remove_nonexistent_errors() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "autoload_remove",
            json!({"name": "ThisAutoloadDoesNotExist_xyz_unit"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(
        err.contains("not found") || err.contains("Autoload not found"),
        "expected not-found error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn autoload_remove_echoes_name() {
    let f = DirectorFixture::new();
    let name = "TestAutoload_remove_echo";
    let script_path = "tmp/test_autoload_echo.gd";

    // First add the autoload
    f.run(
        "autoload_add",
        json!({"name": name, "script_path": script_path}),
    )
    .unwrap()
    .unwrap_data();

    // Now remove it and verify the name is echoed back
    let data = f
        .run("autoload_remove", json!({"name": name}))
        .unwrap()
        .unwrap_data();

    assert_eq!(
        data["name"], name,
        "autoload_remove should echo back the name"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn project_settings_set_returns_keys_set() {
    let f = DirectorFixture::new();

    let data = f
        .run(
            "project_settings_set",
            json!({"settings": {"application/config/description": "theatre-test-description"}}),
        )
        .unwrap()
        .unwrap_data();

    let keys_set = data["keys_set"].as_array().unwrap();
    assert!(
        keys_set
            .iter()
            .any(|k| k.as_str() == Some("application/config/description")),
        "keys_set should include the key we set, got: {keys_set:?}"
    );

    // Restore: erase the key
    f.run(
        "project_settings_set",
        json!({"settings": {"application/config/description": null}}),
    )
    .unwrap()
    .unwrap_data();
}

#[test]
#[ignore = "requires Godot binary"]
fn project_settings_set_invalid_key_format_errors() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "project_settings_set",
            json!({"settings": {"no_slash_here": "value"}}),
        )
        .unwrap()
        .unwrap_err();

    assert!(
        err.contains("Invalid key format") || err.contains("section/key"),
        "expected invalid-key-format error, got: {err}"
    );
}
