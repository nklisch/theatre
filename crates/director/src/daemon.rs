use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdout, Command};

use crate::oneshot::{OperationError, OperationResult};

const DEFAULT_PORT: u16 = 6550;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages a single headless Godot daemon process.
pub struct DaemonHandle {
    child: Child,
    stream: TcpStream,
    project_path: PathBuf,
    port: u16,
    // Keeps the read end of stdout's pipe open so the daemon can write to it
    // (e.g. shutdown messages) without getting SIGPIPE.
    _stdout: BufReader<ChildStdout>,
}

/// Errors specific to daemon lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("daemon failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("daemon did not become ready within {0:?}")]
    ReadyTimeout(Duration),

    #[error("daemon TCP connection failed: {0}")]
    ConnectionFailed(#[source] std::io::Error),

    #[error("daemon TCP I/O error: {0}")]
    IoError(#[source] std::io::Error),

    #[error("daemon response parse error: {source}\nraw: {raw}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        raw: String,
    },

    #[error("daemon process exited unexpectedly")]
    ProcessExited,
}

impl From<DaemonError> for OperationError {
    fn from(e: DaemonError) -> Self {
        OperationError::ProcessFailed {
            status: -1,
            stderr: e.to_string(),
        }
    }
}

impl DaemonHandle {
    /// Spawn a new daemon for the given project.
    ///
    /// Launches `godot --headless --path <project> --script addons/director/daemon.gd`,
    /// waits for the `{"source":"director","status":"ready"}` signal on stdout,
    /// then connects via TCP.
    pub async fn spawn(
        godot_bin: &Path,
        project_path: &Path,
        port: u16,
    ) -> Result<Self, DaemonError> {
        let mut cmd = Command::new(godot_bin);
        cmd.args([
            "--headless",
            "--path",
            &project_path.to_string_lossy(),
            "--script",
            "addons/director/daemon.gd",
        ])
        .env("DIRECTOR_DAEMON_PORT", port.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(DaemonError::SpawnFailed)?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let mut reader = BufReader::new(stdout);

        // Wait for the ready signal on stdout within READY_TIMEOUT.
        let ready_result = tokio::time::timeout(READY_TIMEOUT, async {
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line).await?;
                if n == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "daemon exited before emitting ready signal",
                    ));
                }
                let trimmed = line.trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed)
                    && val.get("source").and_then(|v| v.as_str()) == Some("director")
                    && val.get("status").and_then(|v| v.as_str()) == Some("ready")
                {
                    return Ok(reader);
                }
            }
        })
        .await;

        let stdout_reader = match ready_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                let _ = child.kill().await;
                return Err(DaemonError::SpawnFailed(e));
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err(DaemonError::ReadyTimeout(READY_TIMEOUT));
            }
        };

        // Connect to the daemon's TCP port.
        let addr = format!("127.0.0.1:{port}");
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            DaemonError::ConnectionFailed(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "TCP connect timed out",
            ))
        })?
        .map_err(DaemonError::ConnectionFailed)?;

        Ok(DaemonHandle {
            child,
            stream,
            project_path: project_path.to_path_buf(),
            port,
            _stdout: stdout_reader,
        })
    }

    /// Send an operation to the daemon and return the result.
    ///
    /// Wire format: length-prefixed JSON (4-byte BE u32 + JSON payload).
    pub async fn send_operation(
        &mut self,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, DaemonError> {
        let request = serde_json::json!({
            "operation": operation,
            "params": params,
        });

        tokio::time::timeout(OPERATION_TIMEOUT, async {
            write_message(&mut self.stream, &request).await?;
            let response = read_message(&mut self.stream).await?;
            serde_json::from_value(response).map_err(|source| DaemonError::ParseFailed {
                source,
                raw: String::new(),
            })
        })
        .await
        .map_err(|_| {
            DaemonError::IoError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "operation timed out",
            ))
        })?
    }

    /// Send quit command and wait for process exit.
    pub async fn shutdown(mut self) -> Result<(), DaemonError> {
        let quit_msg = serde_json::json!({"operation": "quit", "params": {}});
        // Best-effort send — ignore errors if the daemon is already gone.
        let _ = write_message(&mut self.stream, &quit_msg).await;
        self.child.wait().await.map_err(DaemonError::SpawnFailed)?;
        Ok(())
    }

    /// Check if the daemon process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// The project path this daemon was spawned for.
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    /// The port this daemon is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        // Best-effort kill if shutdown() was not called.
        let _ = self.child.start_kill();
    }
}

/// Resolve the daemon port from env var or default.
pub fn resolve_daemon_port() -> u16 {
    std::env::var("DIRECTOR_DAEMON_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Write a length-prefixed JSON message to a TCP stream.
async fn write_message(
    stream: &mut TcpStream,
    value: &serde_json::Value,
) -> Result<(), DaemonError> {
    let json = serde_json::to_vec(value).map_err(|source| DaemonError::ParseFailed {
        source,
        raw: String::new(),
    })?;
    let len = (json.len() as u32).to_be_bytes();
    stream.write_all(&len).await.map_err(DaemonError::IoError)?;
    stream.write_all(&json).await.map_err(DaemonError::IoError)?;
    stream.flush().await.map_err(DaemonError::IoError)?;
    Ok(())
}

/// Read a length-prefixed JSON message from a TCP stream.
async fn read_message(stream: &mut TcpStream) -> Result<serde_json::Value, DaemonError> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(DaemonError::IoError)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .map_err(DaemonError::IoError)?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    serde_json::from_slice(&buf).map_err(|source| DaemonError::ParseFailed { source, raw })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_daemon_port_default() {
        // Remove env var if set, then check default.
        // SAFETY: single-threaded test context.
        unsafe { std::env::remove_var("DIRECTOR_DAEMON_PORT") };
        assert_eq!(resolve_daemon_port(), 6550);
    }

    #[test]
    fn test_resolve_daemon_port_from_env() {
        // SAFETY: single-threaded test context.
        unsafe { std::env::set_var("DIRECTOR_DAEMON_PORT", "7777") };
        assert_eq!(resolve_daemon_port(), 7777);
        unsafe { std::env::remove_var("DIRECTOR_DAEMON_PORT") };
    }
}
