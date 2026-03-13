use std::path::Path;
use std::time::Duration;

use spectator_protocol::codec::async_io;
use tokio::net::TcpStream;

use crate::oneshot::{OperationError, OperationResult};

const DEFAULT_PORT: u16 = 6551;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors specific to the editor plugin TCP client.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("editor plugin not reachable on port {0}")]
    NotReachable(u16),

    #[error("editor plugin TCP I/O error: {0}")]
    IoError(#[source] std::io::Error),

    #[error("editor plugin response parse error: {source}\nraw: {raw}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        raw: String,
    },

    #[error("editor plugin operation timed out")]
    Timeout,
}

impl From<EditorError> for OperationError {
    fn from(e: EditorError) -> Self {
        OperationError::ProcessFailed {
            status: -1,
            stderr: e.to_string(),
        }
    }
}

/// TCP client handle for a running Director EditorPlugin.
///
/// Unlike DaemonHandle, this does not manage a process — the editor
/// is already running. EditorHandle only manages the TCP connection.
pub struct EditorHandle {
    stream: TcpStream,
    port: u16,
}

impl EditorHandle {
    /// Attempt to connect to the editor plugin on the given port.
    ///
    /// Returns `Err(EditorError::NotReachable)` if the plugin is not
    /// listening (editor closed or plugin not enabled). The connect
    /// attempt times out after CONNECT_TIMEOUT (2s).
    pub async fn connect(port: u16) -> Result<Self, EditorError> {
        let addr = format!("127.0.0.1:{port}");
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| EditorError::NotReachable(port))?
            .map_err(|_| EditorError::NotReachable(port))?;
        Ok(EditorHandle { stream, port })
    }

    /// Send an operation and return the result.
    ///
    /// Wire format: length-prefixed JSON (4-byte BE u32 + JSON payload),
    /// identical to the daemon protocol.
    pub async fn send_operation(
        &mut self,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, EditorError> {
        let request = serde_json::json!({
            "operation": operation,
            "params": params,
        });

        tokio::time::timeout(OPERATION_TIMEOUT, async {
            async_io::write_message(&mut self.stream, &request)
                .await
                .map_err(codec_error_to_editor)?;
            let response: serde_json::Value = async_io::read_message(&mut self.stream)
                .await
                .map_err(codec_error_to_editor)?;
            serde_json::from_value(response).map_err(|source| EditorError::ParseFailed {
                source,
                raw: String::new(),
            })
        })
        .await
        .map_err(|_| EditorError::Timeout)?
    }

    /// Check if the TCP connection is still alive (non-blocking peek).
    pub fn is_alive(&self) -> bool {
        // A zero-byte peek succeeds if the socket is open.
        // WouldBlock means alive but no data; Err means dead.
        match self.stream.try_read(&mut [0u8; 0]) {
            Ok(_) => true,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Resolve the editor plugin port.
///
/// Priority: DIRECTOR_EDITOR_PORT env var > project.godot setting > default 6551.
pub fn resolve_editor_port(project_path: &Path) -> u16 {
    // 1. Env var
    if let Ok(val) = std::env::var("DIRECTOR_EDITOR_PORT")
        && let Ok(port) = val.parse::<u16>()
    {
        return port;
    }

    // 2. project.godot
    let godot_file = project_path.join("project.godot");
    if let Ok(contents) = std::fs::read_to_string(&godot_file)
        && let Some(port) = parse_editor_port_from_project(&contents)
    {
        return port;
    }

    // 3. Default
    DEFAULT_PORT
}

/// Parse the editor port from project.godot content.
///
/// Looks for `connection/editor_port=<number>` under the `[director]` section.
fn parse_editor_port_from_project(contents: &str) -> Option<u16> {
    let mut in_director_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_director_section = trimmed == "[director]";
            continue;
        }
        if in_director_section && let Some(val) = trimmed.strip_prefix("connection/editor_port=") {
            return val.trim().trim_matches('"').parse().ok();
        }
    }
    None
}

/// Map a `CodecError` to `EditorError`.
fn codec_error_to_editor(e: spectator_protocol::codec::CodecError) -> EditorError {
    use spectator_protocol::codec::CodecError;
    match e {
        CodecError::Io(io) => EditorError::IoError(io),
        CodecError::Serialize(src) => EditorError::ParseFailed {
            source: src,
            raw: String::new(),
        },
        CodecError::Deserialize(src) => EditorError::ParseFailed {
            source: src,
            raw: String::new(),
        },
        CodecError::MessageTooLarge(n) => EditorError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("message too large: {n} bytes"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_default_port() {
        unsafe { std::env::remove_var("DIRECTOR_EDITOR_PORT") };
        let port = resolve_editor_port(Path::new("/nonexistent"));
        assert_eq!(port, 6551);
    }

    #[test]
    fn resolve_env_var_port() {
        unsafe { std::env::set_var("DIRECTOR_EDITOR_PORT", "7777") };
        let port = resolve_editor_port(Path::new("/nonexistent"));
        assert_eq!(port, 7777);
        unsafe { std::env::remove_var("DIRECTOR_EDITOR_PORT") };
    }

    #[test]
    fn parse_project_godot_port() {
        let contents = "\
[application]\nconfig/name=\"Test\"\n\n[director]\nconnection/editor_port=6600\n";
        assert_eq!(parse_editor_port_from_project(contents), Some(6600));
    }

    #[test]
    fn parse_project_godot_no_section() {
        let contents = "[application]\nconfig/name=\"Test\"\n";
        assert_eq!(parse_editor_port_from_project(contents), None);
    }

    #[test]
    fn parse_project_godot_wrong_section() {
        let contents = "[spectator]\nconnection/editor_port=6600\n";
        assert_eq!(parse_editor_port_from_project(contents), None);
    }
}
