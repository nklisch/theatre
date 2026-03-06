use anyhow::Result;
use rmcp::model::ErrorData as McpError;
use spectator_protocol::{
    codec::async_io,
    handshake::{HandshakeAck, HandshakeError, PROTOCOL_VERSION},
    messages::Message,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
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
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tcp_writer: None,
            connected: false,
            session_id: None,
            handshake_info: None,
            pending_queries: HashMap::new(),
        }
    }
}

/// Information received from the addon during handshake.
#[derive(Debug, Clone)]
pub struct HandshakeInfo {
    pub spectator_version: String,
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
                }

                tracing::info!("Addon disconnected, will retry in 2s");
            }
            Err(e) => {
                tracing::debug!("Connection failed: {}", e);
            }
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<Mutex<SessionState>>) -> Result<()> {
    let (mut reader, writer) = tokio::io::split(stream);

    // Step 1: Read handshake from addon
    let msg: Message = async_io::read_message(&mut reader)
        .await
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
        s.handshake_info = Some(HandshakeInfo {
            spectator_version: handshake.spectator_version,
            godot_version: handshake.godot_version,
            scene_dimensions: handshake.scene_dimensions,
            physics_ticks_per_sec: handshake.physics_ticks_per_sec,
            project_name: handshake.project_name,
        });
    }

    // Step 5: Read loop — dispatch incoming messages
    loop {
        match async_io::read_message::<Message>(&mut reader).await {
            Ok(Message::Response { id, data }) => {
                let mut s = state.lock().await;
                if let Some(sender) = s.pending_queries.remove(&id) {
                    let _ = sender.send(QueryResult::Ok(data));
                }
            }
            Ok(Message::Error { id, code, message }) => {
                let mut s = state.lock().await;
                if let Some(sender) = s.pending_queries.remove(&id) {
                    let _ = sender.send(QueryResult::Err { code, message });
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
                McpError::internal_error(
                    "Not connected to Godot addon. Is the game running?",
                    None,
                )
            })?;
            request_id = writer.next_request_id();
        }

        s.pending_queries.insert(request_id.clone(), tx);

        let msg = Message::Query {
            id: request_id.clone(),
            method: method.to_string(),
            params,
        };

        let writer = s.tcp_writer.as_mut().expect("checked above");
        async_io::write_message(&mut writer.writer, &msg)
            .await
            .map_err(|e| McpError::internal_error(format!("TCP write error: {e}"), None))?;
    }
    // Lock released — wait for response

    let result = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .map_err(|_| {
            McpError::internal_error(
                "Addon did not respond within 5000ms. Game may be frozen or at a breakpoint.",
                None,
            )
        })?
        .map_err(|_| {
            McpError::internal_error(
                "TCP connection dropped while waiting for response",
                None,
            )
        })?;

    // Clean up pending entry if timeout didn't do it
    match result {
        QueryResult::Ok(data) => Ok(data),
        QueryResult::Err { code, message } => Err(make_spectator_error(&code, &message)),
    }
}

/// Map Spectator error codes to McpError.
fn make_spectator_error(code: &str, message: &str) -> McpError {
    match code {
        "node_not_found" => McpError::invalid_params(message.to_string(), None),
        "scene_not_loaded" => McpError::internal_error(message.to_string(), None),
        _ => McpError::internal_error(format!("{code}: {message}"), None),
    }
}
