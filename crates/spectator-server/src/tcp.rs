use anyhow::Result;
use spectator_protocol::{
    codec::async_io,
    handshake::{HandshakeAck, HandshakeError, PROTOCOL_VERSION},
    messages::Message,
};
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

/// Handle to the TCP connection's write half, for sending queries.
pub struct TcpClientHandle {
    // Used in M1+ when MCP handlers send query messages to the addon.
    #[allow(dead_code)]
    pub writer: WriteHalf<TcpStream>,
}

/// Shared state between MCP handlers and TCP client task.
pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
}

/// Information received from the addon during handshake.
// Fields read in M1+ by MCP tool handlers.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HandshakeInfo {
    pub spectator_version: String,
    pub godot_version: String,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
    pub project_name: String,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tcp_writer: None,
            connected: false,
            session_id: None,
            handshake_info: None,
        }
    }
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

                // Clean up state on disconnect
                {
                    let mut s = state.lock().await;
                    s.tcp_writer = None;
                    s.connected = false;
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

async fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<SessionState>>,
) -> Result<()> {
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
        s.tcp_writer = Some(TcpClientHandle { writer });
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

    // Step 5: Read loop — process incoming messages until disconnect
    loop {
        match async_io::read_message::<Message>(&mut reader).await {
            Ok(msg) => {
                // M0: just log. M1+ will dispatch to handlers.
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
