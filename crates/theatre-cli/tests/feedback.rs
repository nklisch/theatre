use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn command(project: &Path, action: Value) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["feedback", "--project"])
        .arg(project)
        .arg(action.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn hook(project: &Path) -> Value {
    hook_with_response(project, "config_version=5")
}

fn hook_with_response(project: &Path, response: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("feedback-hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    write!(child.stdin.take().unwrap(), "{}", json!({"hook_event_name":"PostToolUse", "cwd":project, "tool_name":"Read", "tool_response":response})).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_and_shared_native_hook_helper_preserve_project_evidence() {
    let project = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    for path in [project.path(), other.path()] {
        fs::write(path.join("project.godot"), "config_version=5\n").unwrap();
    }
    let dir = project.path().join(".theatre/feedback/feedback_native");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("item.json"), json!({
        "feedback_id":"feedback_native", "source":"editor", "timestamp_ms":1,
        "project_path":project.path(), "process_id":12, "run_id":null, "scene":"res://main.tscn",
        "surface":"unavailable", "selection":[{"path":"/root/Player","class":"Node2D"}],
        "pointer":{"status":"unavailable"},"capture":{"status":"unavailable","reason":"closed viewport"},
        "readback_render_frame":1,"readback_physics_frame":2,"note":"Please inspect the selected player"
    }).to_string()).unwrap();
    let notice = hook(project.path());
    assert!(
        notice["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("1 pending")
    );
    assert_eq!(notice["hookSpecificOutput"]["hookEventName"], "PostToolUse");
    assert!(!notice.to_string().contains("base64"));
    assert_eq!(hook(other.path()), json!({}));
    assert_eq!(hook(project.path()), notice);
    assert_eq!(
        hook_with_response(project.path(), &"x".repeat(1024 * 1024 + 1)),
        notice
    );
    let status = command(project.path(), json!({"action":"status"}));
    assert_eq!(status["pending_count"], 1);
    assert!(status["feedback_notice"].is_string());
    let retrieve = command(
        project.path(),
        json!({"action":"retrieve","feedback_id":"feedback_native"}),
    );
    assert_eq!(
        retrieve["item"]["note"],
        "Please inspect the selected player"
    );
    command(
        project.path(),
        json!({"action":"handle","feedback_id":"feedback_native"}),
    );
    assert_eq!(hook(project.path()), json!({}));
    assert!(dir.join("item.json").exists());
    let retrieve = command(
        project.path(),
        json!({"action":"retrieve","feedback_id":"feedback_native"}),
    );
    assert_eq!(retrieve["handled"], true);
    command(
        project.path(),
        json!({"action":"delete","feedback_id":"feedback_native"}),
    );
    assert!(!dir.exists());
}
