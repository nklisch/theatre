use std::path::{Path, PathBuf};
use std::time::Duration;

use stage_protocol::codec::async_io;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;

use crate::oneshot::{OperationError, OperationResult};
use crate::process::{OwnedChild, StderrTail};

const DEFAULT_PORT: u16 = 6550;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Manages a single headless Godot daemon process.
pub struct DaemonHandle {
    child: OwnedChild,
    stream: TcpStream,
    project_path: PathBuf,
    port: u16,
    stdout_task: Option<tokio::task::JoinHandle<()>>,
    stderr: Option<StderrTail>,
}

/// Errors specific to daemon lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("daemon failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("daemon did not become ready within {duration:?}: {stderr}")]
    ReadyTimeout { duration: Duration, stderr: String },

    #[error("daemon TCP connection failed: {source}: {stderr}")]
    ConnectionFailed {
        #[source]
        source: std::io::Error,
        stderr: String,
    },

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

    #[error("daemon readiness ping failed: {message}: {stderr}")]
    PingFailed { message: String, stderr: String },

    #[error("daemon operation timed out after {duration:?}: {stderr}")]
    OperationTimeout { duration: Duration, stderr: String },

    #[error("daemon did not shut down within {duration:?}: {stderr}")]
    ShutdownTimeout { duration: Duration, stderr: String },
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
        let mut cmd = crate::process::godot_command(godot_bin).map_err(DaemonError::SpawnFailed)?;
        cmd.args([
            "--headless",
            "--path",
            &project_path.to_string_lossy(),
            "--script",
            "addons/director/daemon.gd",
        ])
        .env("DIRECTOR_DAEMON_PORT", port.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

        let mut child = OwnedChild::spawn(&mut cmd).map_err(DaemonError::SpawnFailed)?;

        let stdout = child.take_stdout().expect("stdout was piped");
        let stderr = child.take_stderr().expect("stderr was piped");
        let stderr = crate::process::spawn_stderr_tail(stderr);
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
                let _ = child.terminate_and_wait().await;
                let captured = stderr.finish().await;
                return Err(DaemonError::PingFailed {
                    message: e.to_string(),
                    stderr: captured,
                });
            }
            Err(_) => {
                let _ = child.terminate_and_wait().await;
                let captured = stderr.finish().await;
                return Err(DaemonError::ReadyTimeout {
                    duration: READY_TIMEOUT,
                    stderr: captured,
                });
            }
        };

        // Connect to the daemon's TCP port.
        let addr = format!("127.0.0.1:{port}");
        let stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(source)) => {
                let _ = child.terminate_and_wait().await;
                let captured = stderr.finish().await;
                return Err(DaemonError::ConnectionFailed {
                    source,
                    stderr: captured,
                });
            }
            Err(_) => {
                let _ = child.terminate_and_wait().await;
                let captured = stderr.finish().await;
                return Err(DaemonError::ConnectionFailed {
                    source: std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "TCP connect timed out",
                    ),
                    stderr: captured,
                });
            }
        };

        // Continue draining stdout after the ready line. Merely retaining the
        // pipe can deadlock a verbose or large project once its buffer fills.
        let stdout_task = tokio::spawn(async move {
            let mut reader = stdout_reader;
            let _ = tokio::io::copy(&mut reader, &mut tokio::io::sink()).await;
        });

        let mut handle = DaemonHandle {
            child,
            stream,
            project_path: project_path.to_path_buf(),
            port,
            stdout_task: Some(stdout_task),
            stderr: Some(stderr),
        };

        let ping_error = match handle.send_operation("ping", &serde_json::json!({})).await {
            Ok(result) if result.success => None,
            Ok(result) => Some(
                result
                    .error
                    .unwrap_or_else(|| "ping returned an unsuccessful response".into()),
            ),
            Err(error) => Some(error.to_string()),
        };
        if let Some(message) = ping_error {
            let captured = handle.stderr_snapshot();
            handle.terminate().await;
            return Err(DaemonError::PingFailed {
                message,
                stderr: captured,
            });
        }

        Ok(handle)
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
            async_io::write_message(&mut self.stream, &request)
                .await
                .map_err(codec_error_to_daemon)?;
            let response: serde_json::Value = async_io::read_message(&mut self.stream)
                .await
                .map_err(codec_error_to_daemon)?;
            serde_json::from_value(response).map_err(|source| DaemonError::ParseFailed {
                source,
                raw: String::new(),
            })
        })
        .await
        .map_err(|_| DaemonError::OperationTimeout {
            duration: OPERATION_TIMEOUT,
            stderr: self.stderr_snapshot(),
        })?
    }

    /// Send quit command and wait for process exit.
    pub async fn shutdown(mut self) -> Result<(), DaemonError> {
        let quit_msg = serde_json::json!({"operation": "quit", "params": {}});
        // Best-effort send — ignore errors if the daemon is already gone.
        let _ = async_io::write_message::<serde_json::Value>(&mut self.stream, &quit_msg).await;
        match tokio::time::timeout(SHUTDOWN_TIMEOUT, self.child.wait()).await {
            Ok(result) => {
                result.map_err(DaemonError::SpawnFailed)?;
                if let Some(stderr) = self.stderr.take() {
                    let _ = stderr.finish().await;
                }
                if let Some(stdout_task) = self.stdout_task.take() {
                    let _ = stdout_task.await;
                }
                Ok(())
            }
            Err(_) => {
                let _ = self.child.terminate_and_wait().await;
                let stderr = match self.stderr.take() {
                    Some(stderr) => stderr.finish().await,
                    None => String::new(),
                };
                if let Some(stdout_task) = self.stdout_task.take() {
                    let _ = stdout_task.await;
                }
                Err(DaemonError::ShutdownTimeout {
                    duration: SHUTDOWN_TIMEOUT,
                    stderr,
                })
            }
        }
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

    fn stderr_snapshot(&self) -> String {
        self.stderr
            .as_ref()
            .map(StderrTail::snapshot)
            .unwrap_or_default()
    }

    async fn terminate(&mut self) {
        let _ = self.child.terminate_and_wait().await;
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.finish().await;
        }
        if let Some(stdout_task) = self.stdout_task.take() {
            let _ = stdout_task.await;
        }
    }
}

/// Resolve the daemon port from env var or default.
pub fn resolve_daemon_port() -> u16 {
    std::env::var("DIRECTOR_DAEMON_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Map a `CodecError` to `DaemonError`.
fn codec_error_to_daemon(e: stage_protocol::codec::CodecError) -> DaemonError {
    use stage_protocol::codec::CodecError;
    match e {
        CodecError::Io(io) => DaemonError::IoError(io),
        CodecError::Serialize(src) => DaemonError::ParseFailed {
            source: src,
            raw: String::new(),
        },
        CodecError::Deserialize(src) => DaemonError::ParseFailed {
            source: src,
            raw: String::new(),
        },
        CodecError::MessageTooLarge(n) => DaemonError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("message too large: {n} bytes"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes env-var-mutating tests (they race under parallel test threads).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_resolve_daemon_port_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("DIRECTOR_DAEMON_PORT") };
        assert_eq!(resolve_daemon_port(), 6550);
    }

    #[test]
    fn test_resolve_daemon_port_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("DIRECTOR_DAEMON_PORT", "7777") };
        assert_eq!(resolve_daemon_port(), 7777);
        unsafe { std::env::remove_var("DIRECTOR_DAEMON_PORT") };
    }
}
