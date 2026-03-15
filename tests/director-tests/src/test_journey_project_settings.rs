use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

/// Multi-step journey exercising the full project settings workflow:
/// project_reload → editor_status → autoload_add → project_reload (verify)
/// → project_settings_set → autoload_remove → project_reload (verify gone)
/// → editor_status (verify gone)
#[test]
#[ignore = "requires Godot binary"]
fn journey_project_settings_workflow() {
    let f = DirectorFixture::new();

    // Unique names to avoid test conflicts across concurrent runs
    let autoload_name = "TestAutoload_journey";
    let autoload_script = "tmp/j_test_autoload.gd";
    let settings_key = "application/config/description";
    let settings_value = "theatre-journey-test";

    // -----------------------------------------------------------------------
    // Step 1: project_reload — baseline, check scripts_checked > 0
    // -----------------------------------------------------------------------
    let reload1 = f.run("project_reload", json!({})).unwrap().unwrap_data();
    let script_count = reload1["scripts_checked"].as_u64().unwrap();
    assert!(
        script_count > 0,
        "Step 1: test project should have at least one .gd script, got {script_count}"
    );
    assert!(
        reload1["autoloads"].is_object(),
        "Step 1: project_reload should return autoloads dict"
    );

    // -----------------------------------------------------------------------
    // Step 2: editor_status — headless mode, autoloads returned
    // -----------------------------------------------------------------------
    let status1 = f.run("editor_status", json!({})).unwrap().unwrap_data();
    assert_eq!(
        status1["editor_connected"], false,
        "Step 2: headless should report editor_connected: false"
    );
    assert!(
        status1["autoloads"].is_object(),
        "Step 2: editor_status should return autoloads dict"
    );
    assert!(
        status1["recent_log"].is_array(),
        "Step 2: editor_status should return recent_log array"
    );

    // Ensure our test autoload is not already present (clean state)
    let initial_autoloads = reload1["autoloads"].as_object().unwrap();
    assert!(
        !initial_autoloads.contains_key(autoload_name),
        "Step 2: test autoload should not exist before this journey"
    );

    // -----------------------------------------------------------------------
    // Step 3: autoload_add — register a test autoload
    // -----------------------------------------------------------------------
    let add_data = f
        .run(
            "autoload_add",
            json!({"name": autoload_name, "script_path": autoload_script, "enabled": true}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(
        add_data["name"], autoload_name,
        "Step 3: autoload_add should echo back name"
    );
    assert_eq!(
        add_data["script_path"], autoload_script,
        "Step 3: autoload_add should echo back script_path"
    );
    assert_eq!(
        add_data["enabled"], true,
        "Step 3: autoload_add should echo back enabled"
    );

    // -----------------------------------------------------------------------
    // Step 4: project_reload — verify new autoload appears in autoloads dict
    // -----------------------------------------------------------------------
    let reload2 = f.run("project_reload", json!({})).unwrap().unwrap_data();
    let autoloads2 = reload2["autoloads"].as_object().unwrap();
    assert!(
        autoloads2.contains_key(autoload_name),
        "Step 4: project_reload should show newly added autoload '{autoload_name}', got: {autoloads2:?}"
    );
    // The script path stored strips the "*" enabled prefix and "res://" prefix
    assert_eq!(
        autoloads2[autoload_name].as_str().unwrap(),
        autoload_script,
        "Step 4: autoload script path should match what was registered"
    );

    // -----------------------------------------------------------------------
    // Step 5: project_settings_set — set a test setting
    // -----------------------------------------------------------------------
    let settings_data = f
        .run(
            "project_settings_set",
            json!({"settings": {settings_key: settings_value}}),
        )
        .unwrap()
        .unwrap_data();

    let keys_set = settings_data["keys_set"].as_array().unwrap();
    assert!(
        keys_set.iter().any(|k| k.as_str() == Some(settings_key)),
        "Step 5: keys_set should contain '{settings_key}', got: {keys_set:?}"
    );

    // -----------------------------------------------------------------------
    // Step 6: autoload_remove — remove the test autoload
    // -----------------------------------------------------------------------
    let remove_data = f
        .run("autoload_remove", json!({"name": autoload_name}))
        .unwrap()
        .unwrap_data();

    assert_eq!(
        remove_data["name"], autoload_name,
        "Step 6: autoload_remove should echo back the removed name"
    );

    // -----------------------------------------------------------------------
    // Step 7: project_reload — verify autoload is gone
    // -----------------------------------------------------------------------
    let reload3 = f.run("project_reload", json!({})).unwrap().unwrap_data();
    let autoloads3 = reload3["autoloads"].as_object().unwrap();
    assert!(
        !autoloads3.contains_key(autoload_name),
        "Step 7: project_reload should no longer show removed autoload '{autoload_name}'"
    );

    // -----------------------------------------------------------------------
    // Step 8: editor_status — verify autoload is gone here too
    // -----------------------------------------------------------------------
    let status2 = f.run("editor_status", json!({})).unwrap().unwrap_data();
    let status_autoloads = status2["autoloads"].as_object().unwrap();
    assert!(
        !status_autoloads.contains_key(autoload_name),
        "Step 8: editor_status should not show removed autoload '{autoload_name}'"
    );

    // -----------------------------------------------------------------------
    // Cleanup: restore the description setting we set in step 5
    // -----------------------------------------------------------------------
    f.run(
        "project_settings_set",
        json!({"settings": {settings_key: null}}),
    )
    .unwrap()
    .unwrap_data();
}

/// Verify that autoload_remove on a non-existent name returns an error.
/// Separate from the main journey to avoid polluting project state.
#[test]
#[ignore = "requires Godot binary"]
fn journey_autoload_remove_nonexistent_errors() {
    let f = DirectorFixture::new();

    let err = f
        .run(
            "autoload_remove",
            json!({"name": "ThisJourneyAutoload_DoesNotExist_xyz"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(
        err.contains("not found") || err.contains("Autoload not found"),
        "Expected not-found error for non-existent autoload, got: {err}"
    );
}
