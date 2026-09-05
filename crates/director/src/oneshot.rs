use std::path::Path;
use std::time::Duration;

/// Result of a headless Godot operation, parsed from stdout JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OperationResult {
    pub success: bool,
    #[serde(default)]
    pub persistence: crate::responses::Persistence,
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
            Err(OperationError::OperationFailed(Box::new(self)))
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

    #[error("Godot process timed out after {duration:?}: {stderr}")]
    Timeout { duration: Duration, stderr: String },

    #[error("Failed to parse operation output as JSON: {source}\nstdout: {stdout}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        stdout: String,
    },

    #[error("Operation failed: {}", .0.error.as_deref().unwrap_or("unknown error"))]
    OperationFailed(Box<OperationResult>),

    #[error("daemon backend failed: {daemon}; one-shot fallback also failed: {fallback}")]
    FallbackFailed {
        daemon: String,
        fallback: Box<OperationError>,
    },
}

const TIMEOUT: Duration = Duration::from_secs(30);

/// Output of a validation run — parsed result plus raw stderr.
pub struct ValidationOutput {
    pub result: OperationResult,
    pub stderr: String,
}

/// Spawn the headless Godot subprocess and wait for it to finish.
/// Returns `(stdout, stderr, exit_status)`.
async fn run_subprocess(
    godot_bin: &Path,
    project_path: &Path,
    operation: &str,
    params: &serde_json::Value,
) -> Result<(String, String, std::process::ExitStatus), OperationError> {
    let params_json = params.to_string();

    let mut cmd = crate::process::godot_command(godot_bin).map_err(OperationError::SpawnFailed)?;
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

    let mut child =
        crate::process::OwnedChild::spawn(&mut cmd).map_err(OperationError::SpawnFailed)?;
    let stdout = child.take_stdout().expect("stdout was piped");
    let stderr = child.take_stderr().expect("stderr was piped");
    let stdout_task = tokio::spawn(crate::process::read_all(stdout));
    let stderr_tail = crate::process::spawn_stderr_tail(stderr);

    let status = match tokio::time::timeout(TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            let _ = child.terminate_and_wait().await;
            let _ = stdout_task.await;
            let _ = stderr_tail.finish().await;
            return Err(OperationError::SpawnFailed(error));
        }
        Err(_) => {
            let _ = child.terminate_and_wait().await;
            let _ = stdout_task.await;
            let stderr = stderr_tail.finish().await;
            return Err(OperationError::Timeout {
                duration: TIMEOUT,
                stderr,
            });
        }
    };

    let stdout = stdout_task.await;
    let stderr = stderr_tail.finish().await;
    let stdout = stdout
        .map_err(|error| {
            OperationError::SpawnFailed(std::io::Error::other(format!(
                "stdout reader task failed: {error}"
            )))
        })?
        .map_err(OperationError::SpawnFailed)?;

    Ok((
        String::from_utf8_lossy(&stdout).into_owned(),
        stderr,
        status,
    ))
}

/// Parse the last JSON-like line from stdout into an `OperationResult`.
fn parse_stdout(
    stdout: &str,
    stderr: &str,
    status: &std::process::ExitStatus,
) -> Result<OperationResult, OperationError> {
    // Parse the last JSON-like line of stdout (starts with '{').
    // Non-JSON lines like "[Stage] TCP server stopped" may appear after
    // the result when the GDExtension prints during Godot's shutdown.
    let json_line = stdout
        .lines()
        .rev()
        .find(|line| line.trim().starts_with('{'))
        .ok_or_else(|| OperationError::ProcessFailed {
            status: status.code().unwrap_or(-1),
            stderr: stderr.to_owned(),
        })?;

    serde_json::from_str(json_line).map_err(|source| OperationError::ParseFailed {
        source,
        stdout: stdout.to_owned(),
    })
}

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
    let (stdout, stderr, status) =
        run_subprocess(godot_bin, project_path, operation, params).await?;
    parse_stdout(&stdout, &stderr, &status)
}

/// Run a Director operation via headless one-shot, returning stderr alongside the result.
///
/// Identical to `run_oneshot` but always returns stderr (even on success).
/// Used by `project_reload` to capture Godot's parse error output.
pub async fn run_validation(
    godot_bin: &Path,
    project_path: &Path,
    operation: &str,
    params: &serde_json::Value,
) -> Result<ValidationOutput, OperationError> {
    let (stdout, stderr, status) =
        run_subprocess(godot_bin, project_path, operation, params).await?;
    let result = parse_stdout(&stdout, &stderr, &status)?;
    Ok(ValidationOutput { result, stderr })
}
