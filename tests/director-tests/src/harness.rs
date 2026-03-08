use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// A Director operation runner for E2E tests.
///
/// Spawns `godot --headless --path <project> --script addons/director/operations.gd
/// -- <op> '<json>'` and parses the JSON result from stdout.
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
}

/// Parsed operation result from GDScript stdout.
#[derive(Debug, serde::Deserialize)]
pub struct OperationResult {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

impl OperationResult {
    pub fn unwrap_data(self) -> serde_json::Value {
        if !self.success {
            panic!(
                "Expected success, got error: {}",
                self.error.unwrap_or_else(|| "unknown".into())
            );
        }
        self.data
    }

    pub fn unwrap_err(self) -> String {
        if self.success {
            panic!("Expected error, got success: {:?}", self.data);
        }
        self.error.unwrap_or_else(|| "unknown error".into())
    }
}

impl DirectorFixture {
    pub fn new() -> Self {
        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        Self {
            godot_bin,
            project_dir: Self::project_dir(),
        }
    }

    /// Run a Director operation and return the parsed result.
    pub fn run(&self, operation: &str, params: serde_json::Value) -> anyhow::Result<OperationResult> {
        let output = Command::new(&self.godot_bin)
            .args([
                "--headless",
                "--path",
                &self.project_dir.to_string_lossy(),
                "--script",
                "addons/director/operations.gd",
                "--",
                operation,
                &params.to_string(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to launch Godot ({}): {e}", self.godot_bin))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the last non-empty line of stdout as JSON
        let json_line = stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("No output from Godot.\nstderr: {stderr}")
            })?;

        serde_json::from_str(json_line).map_err(|e| {
            anyhow::anyhow!("Failed to parse JSON: {e}\nline: {json_line}\nfull stdout: {stdout}\nstderr: {stderr}")
        })
    }

    /// Create a temporary scene file path that won't conflict between tests.
    pub fn temp_scene_path(name: &str) -> String {
        format!("tmp/test_{name}.tscn")
    }

    fn project_dir() -> PathBuf {
        project_dir_path()
    }

    pub fn project_dir_path() -> PathBuf {
        project_dir_path()
    }
}

/// Returns the absolute path to the Godot test project directory.
pub fn project_dir_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../godot-project")
        .canonicalize()
        .expect("tests/godot-project dir must exist")
}

/// Assert two f64 values are approximately equal (within 0.01).
pub fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected ~{expected}, got {actual}"
    );
}

// ---------------------------------------------------------------------------
// DaemonFixture — synchronous test harness for the headless daemon
// ---------------------------------------------------------------------------

const DAEMON_DEFAULT_PORT: u16 = 16550; // offset from production port to avoid conflicts

/// A synchronous test harness for the Director headless daemon.
///
/// Spawns `godot --headless --path <project> --script addons/director/daemon.gd`,
/// waits for the ready signal on stdout, then connects via TCP.
pub struct DaemonFixture {
    child: Option<Child>,
    stream: Option<TcpStream>,
    port: u16,
    project_dir: PathBuf,
}

impl DaemonFixture {
    pub fn start() -> Self {
        Self::start_with_port(DAEMON_DEFAULT_PORT)
    }

    pub fn start_with_port(port: u16) -> Self {
        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        let project_dir = project_dir_path();

        let mut child = Command::new(&godot_bin)
            .args([
                "--headless",
                "--path",
                &project_dir.to_string_lossy(),
                "--script",
                "addons/director/daemon.gd",
            ])
            .env("DIRECTOR_DAEMON_PORT", port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to launch Godot daemon ({godot_bin}): {e}"));

        // Read stdout line-by-line until we see the ready signal.
        let stdout = child.stdout.take().expect("stdout was piped");
        let mut reader = std::io::BufReader::new(stdout);
        let mut ready = false;

        use std::io::BufRead;
        for line in reader.by_ref().lines() {
            let line = line.expect("reading daemon stdout");
            let trimmed = line.trim().to_string();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&trimmed)
                && val.get("source").and_then(|v| v.as_str()) == Some("director")
                && val.get("status").and_then(|v| v.as_str()) == Some("ready")
            {
                ready = true;
                break;
            }
        }

        if !ready {
            let _ = child.kill();
            panic!("Daemon did not emit ready signal");
        }

        // Connect TCP.
        let addr = format!("127.0.0.1:{port}");
        let stream = TcpStream::connect(&addr)
            .unwrap_or_else(|e| panic!("Failed to connect to daemon at {addr}: {e}"));
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .expect("set_read_timeout");

        DaemonFixture {
            child: Some(child),
            stream: Some(stream),
            port,
            project_dir,
        }
    }

    /// Send an operation via length-prefixed JSON and read the response.
    pub fn run(
        &mut self,
        operation: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<OperationResult> {
        let request = serde_json::json!({
            "operation": operation,
            "params": params,
        });
        let stream = self.stream.as_mut().expect("stream is open");
        daemon_write_message(stream, &request)?;
        let response = daemon_read_message(stream)?;
        serde_json::from_value(response).map_err(|e| anyhow::anyhow!("parse error: {e}"))
    }

    /// Send the quit command to the daemon.
    pub fn quit(&mut self) -> anyhow::Result<()> {
        let request = serde_json::json!({"operation": "quit", "params": {}});
        let stream = self.stream.as_mut().expect("stream is open");
        daemon_write_message(stream, &request)?;
        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        // Best-effort quit then kill.
        if let Some(ref mut stream) = self.stream {
            let quit = serde_json::json!({"operation": "quit", "params": {}});
            let _ = daemon_write_message(stream, &quit);
        }
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Write a length-prefixed JSON message to a synchronous TCP stream.
fn daemon_write_message(stream: &mut TcpStream, value: &serde_json::Value) -> anyhow::Result<()> {
    let json = serde_json::to_vec(value)?;
    let len = (json.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(&json)?;
    stream.flush()?;
    Ok(())
}

/// Read a length-prefixed JSON message from a synchronous TCP stream.
fn daemon_read_message(stream: &mut TcpStream) -> anyhow::Result<serde_json::Value> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    serde_json::from_slice(&buf)
        .map_err(|e| anyhow::anyhow!("JSON parse error: {e}\nraw: {raw}"))
}
