use std::path::Path;
use std::time::Duration;

/// Result of a headless Godot operation, parsed from stdout JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Unwrap a successful result or return an error.
    pub fn into_data(self) -> Result<serde_json::Value, OperationError> {
        if self.success {
            Ok(self.data)
        } else {
            Err(OperationError::OperationFailed {
                error: self.error.unwrap_or_else(|| "unknown error".into()),
                operation: self.operation.unwrap_or_else(|| "unknown".into()),
                context: self.context.unwrap_or(serde_json::Value::Null),
            })
        }
    }
}

/// Errors from subprocess execution (not from the GDScript operation itself).
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("Godot process failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("Godot process exited with status {status}: {stderr}")]
    ProcessFailed { status: i32, stderr: String },

    #[error("Godot process timed out after {0:?}")]
    Timeout(Duration),

    #[error("Failed to parse operation output as JSON: {source}\nstdout: {stdout}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        stdout: String,
    },

    #[error("Operation failed: {error}")]
    OperationFailed {
        error: String,
        operation: String,
        context: serde_json::Value,
    },
}

const TIMEOUT: Duration = Duration::from_secs(30);

/// Run a Director operation via headless Godot one-shot.
///
/// Spawns: `godot --headless --path <project_path> --script
/// addons/director/operations.gd -- <operation> '<params_json>'`
///
/// Parses the last line of stdout as JSON `OperationResult`.
pub async fn run_oneshot(
    godot_bin: &Path,
    project_path: &Path,
    operation: &str,
    params: &serde_json::Value,
) -> Result<OperationResult, OperationError> {
    let params_json = params.to_string();

    let mut cmd = tokio::process::Command::new(godot_bin);
    cmd.args([
        "--headless",
        "--path",
        &project_path.to_string_lossy(),
        "--script",
        "addons/director/operations.gd",
        "--",
        operation,
        &params_json,
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = cmd.spawn().map_err(OperationError::SpawnFailed)?;

    let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| OperationError::Timeout(TIMEOUT))?
        .map_err(OperationError::SpawnFailed)?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    // Parse the last JSON-like line of stdout (starts with '{').
    // Non-JSON lines like "[Stage] TCP server stopped" may appear after
    // the result when the GDExtension prints during Godot's shutdown.
    let json_line = stdout
        .lines()
        .rev()
        .find(|line| line.trim().starts_with('{'))
        .ok_or_else(|| OperationError::ProcessFailed {
            status: output.status.code().unwrap_or(-1),
            stderr: stderr.clone(),
        })?;

    serde_json::from_str(json_line).map_err(|source| OperationError::ParseFailed {
        source,
        stdout: stdout.clone(),
    })
}
