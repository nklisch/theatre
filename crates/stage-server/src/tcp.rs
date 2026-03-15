use anyhow::Result;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
use rmcp::model::ErrorData as McpError;
use stage_core::config::SessionConfig;
use stage_core::delta::DeltaEngine;
use stage_core::index::SpatialIndex;
use stage_core::watch::WatchEngine;
use stage_protocol::{
    codec::async_io,
    handshake::{HandshakeAck, HandshakeError, PROTOCOL_VERSION, SceneDimensions},
    messages::Message,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, oneshot};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

/// Handle to the TCP connection's write half, for sending queries.
pub struct TcpClientHandle {
    pub writer: WriteHalf<TcpStream>,
    next_id: u64,
}

impl TcpClientHandle {
    pub fn next_request_id(&mut self) -> String {
        self.next_id += 1;
        format!("req_{}", self.next_id)
    }
}

/// Result of a TCP query to the addon.
pub enum QueryResult {
    Ok(serde_json::Value),
    Err { code: String, message: String },
}

/// Shared state between MCP handlers and TCP client task.
pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
    /// Pending query response channels: request_id → sender.
    pub pending_queries: HashMap<String, oneshot::Sender<QueryResult>>,
    /// Spatial index built from the most recent snapshot.
    pub spatial_index: SpatialIndex,
    /// Delta engine: tracks entity state changes between queries.
    pub delta_engine: DeltaEngine,
    /// Watch engine: manages watch subscriptions and evaluates conditions.
    pub watch_engine: WatchEngine,
    /// Active session configuration (merged from TOML defaults + spatial_config overrides).
    pub config: SessionConfig,
    /// Cached filesystem path to clip storage (resolved from addon or disk cache).
    pub clip_storage_path: Option<String>,
    /// Scene dimensions from handshake.
    pub scene_dimensions: SceneDimensions,
    /// Project directory (for stage.toml, disk cache, etc.).
    pub project_dir: PathBuf,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tcp_writer: None,
            connected: false,
            session_id: None,
            handshake_info: None,
            pending_queries: HashMap::new(),
            spatial_index: SpatialIndex::empty(),
            delta_engine: DeltaEngine::new(),
            watch_engine: WatchEngine::new(),
            config: SessionConfig::default(),
            clip_storage_path: None,
            scene_dimensions: SceneDimensions::Three,
            project_dir: PathBuf::new(),
        }
    }
}

/// Information received from the addon during handshake.
#[derive(Debug, Clone)]
pub struct HandshakeInfo {
    pub stage_version: String,
    pub godot_version: String,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
    pub project_name: String,
}

/// Background task: connect to addon, handle handshake, reconnect on disconnect.
pub async fn tcp_client_loop(state: Arc<Mutex<SessionState>>, port: u16) {
    loop {
        tracing::info!("Connecting to Godot addon on 127.0.0.1:{}...", port);

        match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            Ok(stream) => {
                tracing::info!("Connected to addon");

                match handle_connection(stream, state.clone()).await {
                    Ok(()) => tracing::info!("Connection closed normally"),
                    Err(e) => tracing::warn!("Connection error: {}", e),
                }

                // Clean up state on disconnect — cancel all pending queries
                {
                    let mut s = state.lock().await;
                    s.tcp_writer = None;
                    s.connected = false;
                    // Drop all pending senders — receivers will get RecvError
                    s.pending_queries.clear();
                    // Clear delta baseline on disconnect (game state resets)
                    s.delta_engine = DeltaEngine::new();
                    // Watch engine persists — watches survive reconnect
                }

                tracing::info!("Addon disconnected, will retry in 2s");
            }
            Err(e) => {
                tracing::debug!("Connection failed: {}", e);
            }
        }

        sleep(RECONNECT_DELAY).await;
    }
}

/// Connect to the Godot addon once (no reconnection loop).
/// Used by CLI one-shot mode.
pub async fn connect_once(state: &Arc<Mutex<SessionState>>, port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let timeout = tokio::time::Duration::from_secs(5);

    let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| anyhow::anyhow!("Connection timed out after 5s"))?
        .map_err(|e| anyhow::anyhow!("TCP connection failed: {e}"))?;

    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = handle_connection(stream, state_clone).await {
            tracing::warn!("Connection ended: {e}");
        }
    });

    // Wait for handshake to complete
    let handshake_timeout = tokio::time::Duration::from_secs(3);
    let start = tokio::time::Instant::now();
    loop {
        {
            let s = state.lock().await;
            if s.connected {
                return Ok(());
            }
        }
        if start.elapsed() > handshake_timeout {
            return Err(anyhow::anyhow!(
                "Handshake timed out — addon did not respond within 3s"
            ));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
}

pub(crate) async fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<SessionState>>,
) -> Result<()> {
    let (mut reader, writer) = tokio::io::split(stream);

    // Step 1: Read handshake from addon (timeout: Godot may have another client active)
    let msg: Message = tokio::time::timeout(HANDSHAKE_TIMEOUT, async_io::read_message(&mut reader))
        .await
        .map_err(|_| {
            anyhow::anyhow!("Handshake timeout after 10s — Godot may have another active client")
        })?
        .map_err(|e| anyhow::anyhow!("Failed to read handshake: {}", e))?;

    let handshake = match msg {
        Message::Handshake(h) => h,
        other => {
            anyhow::bail!("Expected handshake, got: {:?}", other);
        }
    };

    tracing::info!(
        "Handshake received: project={}, godot={}, dimensions={}D, protocol=v{}",
        handshake.project_name,
        handshake.godot_version,
        handshake.scene_dimensions,
        handshake.protocol_version,
    );

    // Step 2: Validate protocol version
    if handshake.protocol_version != PROTOCOL_VERSION {
        let error = HandshakeError::version_mismatch(handshake.protocol_version);
        let mut writer = writer;
        async_io::write_message(&mut writer, &Message::HandshakeError(error))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send handshake error: {}", e))?;
        anyhow::bail!(
            "Protocol version mismatch: expected {}, got {}",
            PROTOCOL_VERSION,
            handshake.protocol_version
        );
    }

    // Step 3: Send ACK
    let session_id = format!("sess_{}", Uuid::new_v4().as_simple());
    let ack = HandshakeAck::new(session_id.clone());
    let mut writer = writer;
    async_io::write_message(&mut writer, &Message::HandshakeAck(ack))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send handshake ACK: {}", e))?;

    tracing::info!("Handshake complete — session {}", session_id);

    // Step 4: Update shared state
    {
        let mut s = state.lock().await;
        s.tcp_writer = Some(TcpClientHandle { writer, next_id: 0 });
        s.connected = true;
        s.session_id = Some(session_id);
        s.scene_dimensions = handshake.dimensions();
        s.handshake_info = Some(HandshakeInfo {
            stage_version: handshake.stage_version,
            godot_version: handshake.godot_version,
            scene_dimensions: handshake.scene_dimensions,
            physics_ticks_per_sec: handshake.physics_ticks_per_sec,
            project_name: handshake.project_name,
        });
    }

    // Step 5: Read loop — dispatch incoming messages
    loop {
        match async_io::read_message::<Message>(&mut reader).await {
            Ok(Message::Response { request_id, data }) => {
                let mut s = state.lock().await;
                if let Some(sender) = s.pending_queries.remove(&request_id) {
                    let _ = sender.send(QueryResult::Ok(data));
                }
            }
            Ok(Message::Error {
                request_id,
                code,
                message,
            }) => {
                let mut s = state.lock().await;
                if let Some(sender) = s.pending_queries.remove(&request_id) {
                    let _ = sender.send(QueryResult::Err { code, message });
                }
            }
            Ok(Message::Event { event, data }) => {
                if event == "signal_emitted" {
                    let mut s = state.lock().await;
                    if let (Some(node), Some(signal), Some(frame)) = (
                        data.get("node").and_then(|v| v.as_str()),
                        data.get("signal").and_then(|v| v.as_str()),
                        data.get("frame").and_then(|v| v.as_u64()),
                    ) {
                        s.delta_engine.push_event(stage_core::delta::BufferedEvent {
                            event_type: stage_core::delta::BufferedEventType::SignalEmitted,
                            path: node.to_string(),
                            frame,
                            data: serde_json::json!({
                                "signal": signal,
                                "args": data.get("args").cloned().unwrap_or(serde_json::json!([])),
                            }),
                        });
                    }
                } else {
                    tracing::debug!("Received event from addon: {event}");
                }
            }
            Ok(msg) => {
                tracing::debug!("Received message from addon: {:?}", msg);
            }
            Err(e) => {
                tracing::debug!("Read error (likely disconnect): {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Send a query to the addon and wait for the response.
///
/// The state lock is held briefly to send the query and register the pending
/// response channel, then released before awaiting the response.
pub async fn query_addon(
    state: &Arc<Mutex<SessionState>>,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let (tx, rx) = oneshot::channel();
    let request_id;

    // First lock: get request_id, register pending channel, send the query
    {
        let mut s = state.lock().await;

        // Check connected and get request_id — brief borrow of tcp_writer
        {
            let writer = s.tcp_writer.as_mut().ok_or_else(|| {
                McpError::internal_error("Not connected to Godot addon. Is the game running?", None)
            })?;
            request_id = writer.next_request_id();
        }

        s.pending_queries.insert(request_id.clone(), tx);

        let msg = Message::Query {
            request_id: request_id.clone(),
            method: method.to_string(),
            params,
        };

        let writer = s.tcp_writer.as_mut().expect("checked above");
        async_io::write_message(&mut writer.writer, &msg)
            .await
            .map_err(|e| McpError::internal_error(format!("TCP write error: {e}"), None))?;
    }
    // Lock released — wait for response

    let result = tokio::time::timeout(QUERY_TIMEOUT, rx)
        .await
        .map_err(|_| {
            McpError::internal_error(
                "Addon did not respond within 5000ms. Game may be frozen or at a breakpoint.",
                None,
            )
        })?
        .map_err(|_| {
            McpError::internal_error("TCP connection dropped while waiting for response", None)
        })?;

    // Clean up pending entry if timeout didn't do it
    match result {
        QueryResult::Ok(data) => Ok(data),
        QueryResult::Err { code, message } => Err(make_stage_error(&code, &message)),
    }
}

/// Get the current session config (immutable clone).
pub async fn get_config(state: &Arc<Mutex<SessionState>>) -> SessionConfig {
    state.lock().await.config.clone()
}

/// Map Stage error codes to McpError.
fn make_stage_error(code: &str, message: &str) -> McpError {
    match code {
        "node_not_found" => McpError::invalid_params(message.to_string(), None),
        "scene_not_loaded" => McpError::internal_error(message.to_string(), None),
        _ => McpError::internal_error(format!("{code}: {message}"), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Regression for Bug 3A: handle_connection must time out and return an error
    /// if Godot accepts the TCP connection but never sends the handshake message.
    ///
    /// This simulates the case where the GDExtension already has an active client
    /// and accepts the new TCP connection at OS level (completing the 3-way handshake)
    /// but never calls accept() and thus never sends the Stage handshake.
    ///
    /// The timeout must be ≤12s so CI doesn't hang.
    #[tokio::test]
    async fn handshake_timeout_returns_error_within_12s() {
        // Bind a real TCP listener that accepts connections but never writes data
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Accept connections in a background thread but never write anything
        std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                // Deliberately hold the stream open without writing — simulates a
                // Godot process that has another active client and won't send the
                // handshake to the queued connection.
                std::thread::sleep(Duration::from_secs(30));
            }
        });

        let state = Arc::new(Mutex::new(SessionState::default()));
        let start = std::time::Instant::now();

        // tcp_client_loop is hard to test directly (it loops forever), so test
        // handle_connection directly — it should fail with timeout ~10s.
        let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("must connect to mock listener");

        let result = handle_connection(stream, state).await;

        let elapsed = start.elapsed();

        assert!(
            result.is_err(),
            "handle_connection must return error when no handshake arrives"
        );
        assert!(
            elapsed >= Duration::from_secs(9),
            "must wait at least 9s for timeout (got {elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(12),
            "must time out within 12s (got {elapsed:?})"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("timeout") || err_msg.contains("10s"),
            "error must mention timeout: {err_msg}"
        );
    }

    /// Verify SessionState default has no active connection.
    #[tokio::test]
    async fn session_state_default_not_connected() {
        let state = SessionState::default();
        assert!(!state.connected, "default state should not be connected");
        assert!(
            state.tcp_writer.is_none(),
            "default state should have no writer"
        );
        assert!(
            state.pending_queries.is_empty(),
            "default state should have no pending queries"
        );
    }
}
