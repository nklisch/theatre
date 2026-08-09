use std::process::Command;

#[cfg(windows)]
use std::io::{BufRead as _, BufReader};
#[cfg(windows)]
use std::time::{Duration, Instant};

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
