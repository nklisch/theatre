use serde_json::json;

use crate::harness::{DaemonFixture, DirectorFixture};

/// Full lifecycle: spawn → ping → operation → quit.
#[test]
#[ignore = "requires Godot binary"]
fn daemon_lifecycle() {
    let mut d = DaemonFixture::start_with_port(16551);

    // Ping should return success.
    let pong = d.run("ping", json!({})).expect("ping failed");
    assert!(pong.success, "ping should succeed");
    assert_eq!(
        pong.data.get("status").and_then(|v| v.as_str()),
        Some("ok"),
        "ping data.status should be ok"
    );

    // scene_create should succeed.
    let scene_path = DirectorFixture::temp_scene_path("daemon_lifecycle");
    let result = d
        .run(
            "scene_create",
            json!({
                "scene_path": scene_path,
                "root_type": "Node2D",
                "project_path": d.project_dir().to_string_lossy().as_ref(),
            }),
        )
        .expect("scene_create failed");
    assert!(result.success, "scene_create should succeed: {:?}", result.error);

    // scene_read should return the created scene.
    let read = d
        .run(
            "scene_read",
            json!({
                "scene_path": scene_path,
                "project_path": d.project_dir().to_string_lossy().as_ref(),
            }),
        )
        .expect("scene_read failed");
    assert!(read.success, "scene_read should succeed: {:?}", read.error);

    // Clean quit.
    d.quit().expect("quit failed");
}

/// Verify that the daemon correctly responds to an unknown operation.
#[test]
#[ignore = "requires Godot binary"]
fn daemon_unknown_operation() {
    let mut d = DaemonFixture::start_with_port(16552);

    let result = d
        .run("this_does_not_exist", json!({}))
        .expect("request failed");
    assert!(!result.success, "unknown operation should return failure");
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("Unknown operation"),
        "error message should mention Unknown operation"
    );

    d.quit().expect("quit failed");
}

/// Verify one-shot fallback works (Backend falls back when daemon spawn is skipped).
///
/// This test runs an operation directly via DirectorFixture (one-shot) to
/// confirm the baseline still works independently of the daemon.
#[test]
#[ignore = "requires Godot binary"]
fn fallback_to_oneshot() {
    let f = DirectorFixture::new();
    let scene_path = DirectorFixture::temp_scene_path("oneshot_fallback");
    let project_path = DirectorFixture::project_dir_path()
        .to_string_lossy()
        .into_owned();

    let result = f
        .run(
            "scene_create",
            json!({
                "scene_path": scene_path,
                "root_type": "Node",
                "project_path": project_path,
            }),
        )
        .expect("one-shot scene_create failed");
    assert!(
        result.success,
        "one-shot scene_create should succeed: {:?}",
        result.error
    );
}
