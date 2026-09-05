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
    hook_process(project, response, None)
}

fn hook_process(cwd: &Path, response: &str, selected_project: Option<&Path>) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_theatre"));
    command
        .arg("feedback-hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env_remove("THEATRE_PROJECT_DIR");
    if let Some(project) = selected_project {
        command.env("THEATRE_PROJECT_DIR", project);
    }
    let mut child = command.spawn().unwrap();
    write!(child.stdin.take().unwrap(), "{}", json!({"hook_event_name":"PostToolUse", "cwd":cwd, "tool_name":"Read", "tool_response":response})).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write_pending_feedback(project: &Path, feedback_id: &str) {
    let dir = project.join(".theatre/feedback").join(feedback_id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("item.json"), json!({
        "feedback_id":feedback_id, "source":"editor", "timestamp_ms":1,
        "project_path":project, "process_id":12, "run_id":null, "scene":"res://main.tscn",
        "surface":"unavailable", "selection":[{"path":"/root/Player","class":"Node2D"}],
        "pointer":{"status":"unavailable"}, "capture":{"status":"unavailable","reason":"closed viewport"},
        "readback_render_frame":1,"readback_physics_frame":2,"note":"Please inspect the selected player"
    }).to_string()).unwrap();
}

#[test]
fn cli_and_shared_native_hook_helper_preserve_project_evidence() {
    let project = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    for path in [project.path(), other.path()] {
        fs::write(path.join("project.godot"), "config_version=5\n").unwrap();
    }
    let dir = project.path().join(".theatre/feedback/feedback_native");
    write_pending_feedback(project.path(), "feedback_native");
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

#[test]
fn hook_uses_explicit_nested_project_then_falls_back_to_cwd_ancestors() {
    let repository = tempfile::tempdir().unwrap();
    let nested = repository.path().join("examples/sandbox");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("project.godot"), "config_version=5\n").unwrap();
    write_pending_feedback(&nested, "feedback_nested");

    let selected = hook_process(repository.path(), "ok", Some(&nested));
    assert!(
        selected["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("1 pending")
    );

    let child = nested.join("scenes");
    fs::create_dir_all(&child).unwrap();
    assert_eq!(hook_process(&child, "ok", None), selected);

    let wrong = repository.path().join("missing-project");
    assert_eq!(hook_process(&child, "ok", Some(&wrong)), json!({}));
    assert_eq!(hook_process(&child, "ok", Some(Path::new(""))), json!({}));
}
