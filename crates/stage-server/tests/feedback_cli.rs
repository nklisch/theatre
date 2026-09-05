use serde_json::json;
use std::{fs, process::Command};

#[test]
fn disconnected_cli_feedback_and_early_errors_keep_pending_notices() {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("project.godot"), "config_version=5\n").unwrap();
    let directory = project.path().join(".theatre/feedback/feedback_cli");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("item.json"), json!({
        "feedback_id":"feedback_cli", "source":"runtime", "timestamp_ms":1,
        "project_path":project.path(), "process_id":7, "run_id":"run_7", "scene":"res://main.tscn",
        "surface":"root_viewport", "selection":[], "pointer":{"status":"unavailable"},
        "capture":{"status":"unavailable","reason":"headless"},
        "readback_render_frame":1,"readback_physics_frame":2,"note":"Retained after exit"
    }).to_string()).unwrap();
    for (tool, params, code, error) in [
        ("feedback", "{\"action\":\"status\"}", 0, None),
        (
            "feedback",
            "{\"action\":\"retrieve\",\"feedback_id\":\"feedback_cli\"}",
            0,
            None,
        ),
        (
            "spatial_delta",
            "{}",
            2,
            Some("persistent_session_required"),
        ),
        (
            "spatial_action",
            "{\"action\":\"teleport\"}",
            2,
            Some("invalid_parameters"),
        ),
        ("scene_tree", "not-json", 2, Some("invalid_json")),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_stage"))
            .args([tool, params])
            .current_dir(project.path())
            .env("THEATRE_PROJECT_DIR", project.path())
            .env("GODOT_BIN", project.path().join("no-engine"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            response["feedback_notice"]
                .as_str()
                .unwrap()
                .contains("1 pending")
        );
        if let Some(error) = error {
            assert_eq!(response["error"], error);
        }
    }
    assert_eq!(
        theatre_feedback::Queue::open(project.path())
            .unwrap()
            .status()
            .unwrap()
            .pending_count,
        1
    );
}
