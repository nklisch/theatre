/// Mock TCP addon — acts as the Godot addon's TCP server side.
///
/// Listens on an ephemeral port, completes the Spectator handshake, and
/// dispatches query responses from a handler function supplied by the test.
use spectator_protocol::{
    codec::async_io,
    handshake::{Handshake, PROTOCOL_VERSION},
    messages::Message,
};
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

/// Handler called for each query: `(method, params) → Ok(data) | Err((code, msg))`.
pub type QueryHandler =
    Arc<dyn Fn(&str, &serde_json::Value) -> Result<serde_json::Value, (String, String)> + Send + Sync>;

pub struct MockAddon {
    pub port: u16,
    // Dropping this triggers shutdown of the listener task.
    _shutdown_tx: oneshot::Sender<()>,
    pub event_tx: mpsc::Sender<Message>,
    join_handle: JoinHandle<()>,
}

impl MockAddon {
    /// Start a mock addon on an ephemeral port with a standard 3D handshake.
    pub async fn start(handler: QueryHandler) -> Self {
        Self::start_with_handshake(default_handshake_3d(), handler).await
    }

    pub async fn start_2d(handler: QueryHandler) -> Self {
        Self::start_with_handshake(default_handshake_2d(), handler).await
    }

    pub async fn start_with_handshake(handshake: Handshake, handler: QueryHandler) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel::<Message>(32);

        let join_handle = tokio::spawn(run_mock(listener, handshake, handler, event_rx, shutdown_rx));

        Self {
            port,
            _shutdown_tx: shutdown_tx,
            event_tx,
            join_handle,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Push an unsolicited event to the connected server.
    pub async fn push_event(&self, event: &str, data: serde_json::Value) {
        let msg = Message::Event {
            event: event.to_string(),
            data,
        };
        let _ = self.event_tx.send(msg).await;
    }

    pub async fn shutdown(self) {
        drop(self._shutdown_tx);
        let _ = self.join_handle.await;
    }
}

async fn run_mock(
    listener: TcpListener,
    handshake: Handshake,
    handler: QueryHandler,
    event_rx: mpsc::Receiver<Message>,
    shutdown_rx: oneshot::Receiver<()>,
) {
    tokio::select! {
        result = listener.accept() => {
            if let Ok((stream, _)) = result {
                run_mock_connection(stream, handshake, handler, event_rx).await;
            }
        }
        _ = shutdown_rx => {}
    }
}

async fn run_mock_connection(
    stream: tokio::net::TcpStream,
    handshake: Handshake,
    handler: QueryHandler,
    mut event_rx: mpsc::Receiver<Message>,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);

    // Send handshake to the server
    async_io::write_message(&mut writer, &Message::Handshake(handshake))
        .await
        .expect("mock: failed to send handshake");

    // Read ACK (or HandshakeError if version mismatch)
    let _ack: Message = match async_io::read_message(&mut reader).await {
        Ok(m) => m,
        Err(_) => return,
    };

    // Query/event loop
    loop {
        tokio::select! {
            result = async_io::read_message::<Message>(&mut reader) => {
                match result {
                    Ok(Message::Query { id, method, params }) => {
                        let response = match handler(&method, &params) {
                            Ok(data) => Message::Response { id, data },
                            Err((code, message)) => Message::Error { id, code, message },
                        };
                        if async_io::write_message(&mut writer, &response).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            Some(event) = event_rx.recv() => {
                if async_io::write_message(&mut writer, &event).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// A mock that rejects the handshake with a version mismatch.
pub async fn start_wrong_version_mock() -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let jh = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let (_, mut writer) = tokio::io::split(stream);
            // Send handshake with wrong protocol version
            let bad_handshake = Handshake {
                spectator_version: "0.0.0".into(),
                protocol_version: PROTOCOL_VERSION + 99,
                godot_version: "4.3".into(),
                scene_dimensions: 3,
                physics_ticks_per_sec: 60,
                project_name: "Bad".into(),
            };
            let _ = async_io::write_message(&mut writer, &Message::Handshake(bad_handshake)).await;
            // Don't read the error response — just drop
        }
    });

    (port, jh)
}

pub fn default_handshake_3d() -> Handshake {
    Handshake::new("4.3".into(), 3, 60, "IntegrationTest".into())
}

pub fn default_handshake_2d() -> Handshake {
    Handshake::new("4.3".into(), 2, 60, "IntegrationTest2D".into())
}
