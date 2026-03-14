/// Integration tests for the `stage` binary CLI mode.
///
/// These tests exercise the binary's command-line interface directly,
/// using `env!("CARGO_BIN_EXE_stage")` to locate the built binary.
use std::process::Command;

fn stage_bin() -> &'static str {
    env!("CARGO_BIN_EXE_stage")
}

/// Run the stage binary with the given args and return (status code, stdout, stderr).
fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(stage_bin())
        .args(args)
        .output()
        .expect("failed to run stage binary");
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

/// `stage --version` prints JSON to stdout and exits 0.
#[test]
fn version_prints_json_to_stdout() {
    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0, "expected exit code 0");
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("--version output must be valid JSON");
    assert!(
        v.get("version").is_some(),
        "version JSON must have 'version' key"
    );
}

/// `stage -V` also prints version JSON.
#[test]
fn version_short_flag_prints_json() {
    let (code, stdout, _) = run(&["-V"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("-V output must be valid JSON");
    assert!(v.get("version").is_some());
}

/// `stage` with no args exits with code 1 and prints usage to stderr.
#[test]
fn no_args_exits_with_error_and_usage_on_stderr() {
    let (code, _stdout, stderr) = run(&[]);
    assert_eq!(code, 1, "expected exit code 1 for no args");
    assert!(
        stderr.contains("Usage"),
        "stderr must contain 'Usage': {stderr}"
    );
}

/// `stage unknown_tool '{}'` exits with code 2 and stdout has error JSON.
#[test]
fn unknown_tool_exits_code_2_with_error_json() {
    let (code, stdout, _) = run(&["unknown_tool_xyz", "{}"]);
    assert_eq!(code, 2, "expected exit code 2 for unknown tool");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON on unknown tool");
    assert_eq!(
        v.get("error").and_then(|e| e.as_str()),
        Some("unknown_tool"),
        "error field must be 'unknown_tool'"
    );
    assert!(
        v.get("available_tools").is_some(),
        "error JSON must list available_tools"
    );
}

/// `stage spatial_snapshot 'not json'` exits with code 2 and stdout has error JSON.
#[test]
fn invalid_json_exits_code_2_with_error_json() {
    let (code, stdout, _) = run(&["spatial_snapshot", "not valid json {"]);
    assert_eq!(code, 2, "expected exit code 2 for invalid JSON");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON on invalid params");
    assert_eq!(
        v.get("error").and_then(|e| e.as_str()),
        Some("invalid_json"),
        "error field must be 'invalid_json'"
    );
}

/// `stage spatial_snapshot '{}'` with no Godot running exits code 1 with connection error JSON.
#[test]
fn no_godot_exits_code_1_with_connection_error() {
    // Use a port almost certainly not in use
    let (code, stdout, _) = run(&["spatial_snapshot", "{}"]);
    // Either connection_failed (no Godot on default port) or exit 1
    assert_eq!(code, 1, "expected exit code 1 when Godot not running");
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON on connection error");
    let error = v.get("error").and_then(|e| e.as_str()).unwrap_or("");
    assert!(
        error == "connection_failed" || error == "tool_error",
        "error must be connection_failed or tool_error, got: {error}"
    );
}
