use std::process::Command;

#[test]
fn director_help_lists_tools() {
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("--help")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scene_create"));
    assert!(stderr.contains("node_add"));
    assert!(stderr.contains("batch"));
}

#[test]
fn director_version_is_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("--version")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(parsed.get("version").is_some());
}

#[test]
fn director_missing_params_exits_2() {
    // No JSON arg and empty stdin — falls through to missing_project_path since
    // stdin is never a terminal in test runners. Providing empty stdin gives {}.
    // The meaningful check is: missing project_path → structured JSON error, exit 2.
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("scene_read")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // stdin is null (empty), so params = {}, which triggers missing_project_path
    assert_eq!(parsed["error"], "missing_project_path");
}

#[test]
fn director_invalid_json_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .args(["scene_read", "not valid json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["error"], "invalid_json");
}

#[test]
fn director_missing_project_path_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .args(["scene_read", r#"{"scene_path":"res://main.tscn"}"#])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["error"], "missing_project_path");
}
