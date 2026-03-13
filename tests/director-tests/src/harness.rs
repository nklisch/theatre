#![allow(dead_code)]

use std::io::Read;
use spectator_protocol::codec;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub use director::oneshot::OperationResult;

/// Extension methods for `OperationResult` used in tests.
pub trait OperationResultExt {
    /// Unwrap a successful result, panicking with the error message if not successful.
    fn unwrap_data(self) -> serde_json::Value;

    /// Unwrap a failed result, panicking with the data if successful.
    fn unwrap_err(self) -> String;
}

impl OperationResultExt for OperationResult {
    fn unwrap_data(self) -> serde_json::Value {
        if !self.success {
            panic!(
                "Expected success, got error: {}",
                self.error.unwrap_or_else(|| "unknown".into())
            );
        }
        self.data
    }

    fn unwrap_err(self) -> String {
        if self.success {
            panic!("Expected error, got success: {:?}", self.data);
        }
        self.error.unwrap_or_else(|| "unknown error".into())
    }
}

/// A Director operation runner for E2E tests.
///
/// Spawns `godot --headless --path <project> --script addons/director/operations.gd
/// -- <op> '<json>'` and parses the JSON result from stdout.
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
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
    pub fn run(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<OperationResult> {
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

        // Parse the last JSON-like line of stdout (starts with '{' or '[').
        // Non-JSON lines like "[Spectator] TCP server stopped" may appear after
        // the result when the GDExtension prints during Godot's shutdown.
        let json_line = stdout
            .lines()
            .rev()
            .find(|line| line.trim().starts_with('{'))
            .ok_or_else(|| {
                anyhow::anyhow!("No JSON output from Godot.\nstdout: {stdout}\nstderr: {stderr}")
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

    /// Temp scene path for journey tests (distinct prefix from unit tests).
    pub fn journey_scene_path(name: &str) -> String {
        format!("tmp/j_{name}.tscn")
    }

    /// Temp resource path for journey tests.
    pub fn temp_resource_path(name: &str) -> String {
        format!("tmp/j_{name}.tres")
    }

    /// Read a scene and find a node by path, returning its JSON object.
    ///
    /// `node_path` is slash-separated (e.g. `"Player/Sprite"`).
    /// Pass `"."` to return the root node.
    /// Panics with a clear message if the node is not found.
    pub fn read_node(&self, scene_path: &str, node_path: &str) -> serde_json::Value {
        let data = self
            .run("scene_read", serde_json::json!({"scene_path": scene_path}))
            .unwrap_or_else(|e| panic!("scene_read failed for '{scene_path}': {e}"))
            .unwrap_data();
        let root = data["root"].clone();
        if node_path == "." {
            return root;
        }
        let mut current = root;
        for part in node_path.split('/') {
            let empty = vec![];
            let children = current["children"].as_array().unwrap_or(&empty);
            current = children
                .iter()
                .find(|c| c["name"].as_str() == Some(part))
                .unwrap_or_else(|| {
                    panic!(
                        "Node '{part}' not found while navigating '{node_path}' in '{scene_path}'"
                    )
                })
                .clone();
        }
        current
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
// Shared Godot fixture helper
// ---------------------------------------------------------------------------

/// Spawn a headless Godot process, wait for the director ready signal on stdout,
/// then connect via TCP.
///
/// Returns `(child, stream)` on success. Panics if Godot fails to start,
/// the ready signal is not received, or the TCP connection fails.
///
/// - `script`: relative script path passed as `--script` (e.g. `"addons/director/daemon.gd"`)
/// - `port_env`: environment variable name for the port (e.g. `"DIRECTOR_DAEMON_PORT"`)
/// - `port`: port number to pass via the env var and connect to
fn spawn_godot_fixture(script: &str, port_env: &str, port: u16) -> (Child, TcpStream) {
    use std::io::BufRead;

    let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
    let project_dir = project_dir_path();

    let mut child = Command::new(&godot_bin)
        .args([
            "--headless",
            "--path",
            &project_dir.to_string_lossy(),
            "--script",
            script,
        ])
        .env(port_env, port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to launch Godot ({godot_bin}) for {script}: {e}"));

    // Read stdout line-by-line until we see the ready signal, then keep
    // draining it in a background thread so the pipe stays open and Godot
    // doesn't get SIGPIPE when printing later output (e.g. Spectator logs).
    let stdout = child.stdout.take().expect("stdout was piped");
    let mut reader = std::io::BufReader::new(stdout);
    let mut ready = false;

    for line in reader.by_ref().lines() {
        let line = line.expect("reading Godot stdout");
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
        panic!("Godot process ({script}) did not emit ready signal");
    }

    // Keep draining stdout so the pipe never fills and Godot never gets SIGPIPE.
    std::thread::spawn(move || for _ in reader.lines() {});

    // Connect TCP.
    let addr = format!("127.0.0.1:{port}");
    let stream = TcpStream::connect(&addr)
        .unwrap_or_else(|e| panic!("Failed to connect to {script} at {addr}: {e}"));
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .expect("set_read_timeout");

    (child, stream)
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
        let project_dir = project_dir_path();
        let (child, stream) =
            spawn_godot_fixture("addons/director/daemon.gd", "DIRECTOR_DAEMON_PORT", port);
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
        codec::write_message(stream, &request).map_err(|e| anyhow::anyhow!("write error: {e}"))?;
        let response: serde_json::Value =
            codec::read_message(stream).map_err(|e| anyhow::anyhow!("read error: {e}"))?;
        serde_json::from_value(response).map_err(|e| anyhow::anyhow!("parse error: {e}"))
    }

    /// Send the quit command to the daemon.
    pub fn quit(&mut self) -> anyhow::Result<()> {
        let request = serde_json::json!({"operation": "quit", "params": {}});
        let stream = self.stream.as_mut().expect("stream is open");
        codec::write_message(stream, &request).map_err(|e| anyhow::anyhow!("write error: {e}"))?;
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
            let _ = codec::write_message(stream, &quit);
        }
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// EditorFixture — synchronous test harness for the mock editor plugin server
// ---------------------------------------------------------------------------

const EDITOR_DEFAULT_PORT: u16 = 16551; // offset from production port to avoid conflicts

/// A synchronous test harness for the Director editor plugin.
///
/// Spawns `godot --headless --path <project> --script addons/director/mock_editor_server.gd`,
/// waits for the ready signal on stdout, then connects via TCP.
/// Uses the same protocol as the real editor plugin (plugin.gd) but runs headlessly.
pub struct EditorFixture {
    child: Option<Child>,
    stream: Option<TcpStream>,
    port: u16,
    project_dir: PathBuf,
}

impl EditorFixture {
    pub fn start() -> Self {
        Self::start_with_port(EDITOR_DEFAULT_PORT)
    }

    pub fn start_with_port(port: u16) -> Self {
        let project_dir = project_dir_path();
        let (child, stream) = spawn_godot_fixture(
            "addons/director/mock_editor_server.gd",
            "DIRECTOR_EDITOR_PORT",
            port,
        );
        EditorFixture {
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
        codec::write_message(stream, &request).map_err(|e| anyhow::anyhow!("write error: {e}"))?;
        let response: serde_json::Value =
            codec::read_message(stream).map_err(|e| anyhow::anyhow!("read error: {e}"))?;
        serde_json::from_value(response).map_err(|e| anyhow::anyhow!("parse error: {e}"))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }
}

impl Drop for EditorFixture {
    fn drop(&mut self) {
        // Best-effort kill — no quit operation in the editor protocol.
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// CliFixture — test harness for the `director <op> '<json>'` CLI subcommand
// ---------------------------------------------------------------------------

/// A test harness that invokes the `director` binary CLI subcommand.
///
/// Runs `director <operation> '<json>'` and parses the JSON result from stdout.
/// Tests the full Rust binary path (arg parsing, backend selection, Godot invocation).
pub struct CliFixture {
    director_bin: PathBuf,
    project_dir: PathBuf,
}

impl CliFixture {
    pub fn new() -> Self {
        // The director binary is built by cargo in the workspace target dir.
        let director_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/director")
            .canonicalize()
            .expect("director binary must be built (run `cargo build -p director` first)");
        Self {
            director_bin,
            project_dir: project_dir_path(),
        }
    }

    /// Run a Director operation via the CLI and return the parsed result.
    ///
    /// Injects `project_path` into params automatically.
    pub fn run(
        &self,
        operation: &str,
        mut params: serde_json::Value,
    ) -> anyhow::Result<OperationResult> {
        // Inject project_path if not already set.
        if let serde_json::Value::Object(ref mut map) = params {
            map.entry("project_path").or_insert_with(|| {
                serde_json::Value::String(self.project_dir.to_string_lossy().into())
            });
        }

        let output = Command::new(&self.director_bin)
            .args([operation, &params.to_string()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to launch director CLI: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "director CLI exited with status {}\nstdout: {stdout}\nstderr: {stderr}",
                output.status
            ));
        }

        // Parse the full stdout as JSON (CLI prints pretty JSON).
        serde_json::from_str(stdout.trim()).map_err(|e| {
            anyhow::anyhow!("Failed to parse CLI JSON: {e}\nstdout: {stdout}\nstderr: {stderr}")
        })
    }
}

