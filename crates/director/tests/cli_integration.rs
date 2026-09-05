use std::{path::Path, process::Command};

#[cfg(windows)]
use std::io::{BufRead as _, BufReader};
#[cfg(windows)]
use std::time::{Duration, Instant};

fn publish_pending_feedback(project: &Path) {
    let directory = project.join(".theatre/feedback/feedback_cli");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("item.json"),
        serde_json::json!({
            "feedback_id": "feedback_cli",
            "source": "editor",
            "timestamp_ms": 1,
            "project_path": project,
            "process_id": 7,
            "scene": "res://main.tscn",
            "surface": "2d",
            "selection": [],
            "pointer": {"status": "unavailable"},
            "capture": {"status": "unavailable", "reason": "test"},
            "readback_render_frame": 0,
            "readback_physics_frame": 0,
            "note": "Inspect this run"
        })
        .to_string(),
    )
    .unwrap();
}

fn assert_pending_notice(result: &serde_json::Value) {
    assert!(
        result["feedback_notice"]
            .as_str()
            .unwrap()
            .contains("1 pending")
    );
}

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
    assert!(stderr.contains("editor_run"));
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
fn editor_run_requires_action_specific_scene_path() {
    let project = tempfile::TempDir::new().unwrap();
    std::fs::write(project.path().join("project.godot"), "config_version=5\n").unwrap();
    publish_pending_feedback(project.path());
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("editor_run")
        .arg(
            serde_json::json!({
                "project_path": project.path(),
                "action": "start"
            })
            .to_string(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("scene_path is required")
    );
    assert_pending_notice(&result);
}

#[test]
fn editor_run_invalid_parameters_keep_pending_notice() {
    let project = tempfile::TempDir::new().unwrap();
    std::fs::write(project.path().join("project.godot"), "config_version=5\n").unwrap();
    publish_pending_feedback(project.path());
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("editor_run")
        .arg(
            serde_json::json!({
                "project_path": project.path(),
                "action": "unknown"
            })
            .to_string(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"], "invalid_parameters");
    assert_pending_notice(&result);
}

#[test]
fn editor_run_has_no_headless_fallback() {
    let project = tempfile::TempDir::new().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    std::fs::write(
        project.path().join("project.godot"),
        format!("config_version=5\n[director]\nconnection/editor_port={port}\n"),
    )
    .unwrap();
    publish_pending_feedback(project.path());
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("editor_run")
        .arg(
            serde_json::json!({
                "project_path": project.path(),
                "action": "status"
            })
            .to_string(),
        )
        .env("GODOT_BIN", "/must-not-be-launched")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let message = result["message"].as_str().unwrap();
    assert!(message.contains("requires the Godot editor"), "{message}");
    assert!(
        message.contains("no headless backend was started"),
        "{message}"
    );
    assert_pending_notice(&result);
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

#[cfg(windows)]
#[test]
fn killing_supervisor_terminates_descendant_process_tree() {
    let script = concat!(
        "$child = Start-Process powershell.exe -WindowStyle Hidden -PassThru ",
        "-ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 120'; ",
        "[Console]::Out.WriteLine($child.Id); [Console]::Out.Flush(); ",
        "Wait-Process -Id $child.Id"
    );
    let mut supervisor = Command::new(env!("CARGO_BIN_EXE_director"))
        .args([
            "__process-supervisor",
            "powershell.exe",
            "-NoProfile",
            "-Command",
            script,
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let mut line = String::new();
    BufReader::new(supervisor.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let descendant_pid: u32 = line.trim().parse().unwrap();
    assert!(process_is_running(descendant_pid));

    supervisor.kill().unwrap();
    supervisor.wait().unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while process_is_running(descendant_pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !process_is_running(descendant_pid),
        "descendant process {descendant_pid} survived supervisor termination"
    );
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

    unsafe {
        const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
        let process = OpenProcess(SYNCHRONIZE_ACCESS, 0, pid);
        if process.is_null() {
            return false;
        }
        let result = WaitForSingleObject(process, 0) == WAIT_TIMEOUT;
        CloseHandle(process);
        result
    }
}
