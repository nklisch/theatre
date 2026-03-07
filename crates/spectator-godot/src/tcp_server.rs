use godot::obj::Gd;
use godot::prelude::*;
use spectator_protocol::{codec, handshake::Handshake, messages::Message};
use spectator_protocol::query::ActionResponse;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};

use crate::collector::SpectatorCollector;
use crate::recorder::SpectatorRecorder;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    client: Option<TcpStream>,
    port: i32,
    handshake_completed: bool,
    collector: Option<Gd<SpectatorCollector>>,
    recorder: Option<Gd<SpectatorRecorder>>,
}

#[godot_api]
impl INode for SpectatorTCPServer {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            listener: None,
            client: None,
            port: 9077,
            handshake_completed: false,
            collector: None,
            recorder: None,
        }
    }
}

#[godot_api]
impl SpectatorTCPServer {
    /// Emitted when an activity_log event is received from the server.
    /// `active_watches` is the current watch count from meta (-1 if not provided).
    #[signal]
    fn activity_received(entry_type: GString, summary: GString, tool_name: GString, active_watches: i64);

    /// Wire the collector into the TCP server.
    #[func]
    pub fn set_collector(&mut self, collector: Gd<SpectatorCollector>) {
        self.collector = Some(collector);
    }

    /// Wire the recorder into the TCP server.
    #[func]
    pub fn set_recorder(&mut self, recorder: Gd<SpectatorRecorder>) {
        self.recorder = Some(recorder);
    }

    /// Returns "connected", "waiting", or "stopped".
    #[func]
    pub fn get_connection_status(&self) -> GString {
        if self.handshake_completed {
            "connected".into()
        } else if self.listener.is_some() {
            "waiting".into()
        } else {
            "stopped".into()
        }
    }

    /// Returns the port the server is (or was) listening on.
    #[func]
    pub fn get_port(&self) -> i32 {
        self.port
    }

    /// Start listening on the given port. Binds to localhost only.
    #[func]
    pub fn start(&mut self, port: i32) {
        self.port = port;
        let addr = format!("127.0.0.1:{}", port);
        match TcpListener::bind(&addr) {
            Ok(listener) => {
                listener.set_nonblocking(true).ok();
                self.listener = Some(listener);
                godot_print!("[Spectator] TCP server listening on {}", addr);
            }
            Err(e) => {
                godot_error!("[Spectator] Failed to bind to {}: {}", addr, e);
            }
        }
    }

    /// Stop listening and close any active connection.
    #[func]
    pub fn stop(&mut self) {
        self.client = None;
        self.listener = None;
        self.handshake_completed = false;
        godot_print!("[Spectator] TCP server stopped");
    }

    /// Returns true if a client is connected and handshake is complete.
    #[func]
    pub fn is_connected(&self) -> bool {
        self.handshake_completed
    }

    /// Poll for new connections and incoming messages. Call every _physics_process.
    #[func]
    pub fn poll(&mut self) {
        // Check frame-advance state before processing new queries
        if let Some(advance_msg) = self.check_frame_advance() {
            self.send_response(advance_msg);
            return; // Don't process new queries while advancing
        }
        // Still advancing (remaining > 0 but not done yet) — skip new queries
        if self.is_advancing() {
            return;
        }

        if self.client.is_none() {
            self.try_accept();
        }

        if self.client.is_some() {
            self.try_read();
        }
    }
}

/// Run `f` with the stream in blocking mode, then restore non-blocking.
///
/// Using a closure means the mutable borrow of the stream is contained inside
/// `f` and released before the caller handles the result — NLL then allows
/// `self.disconnect_client()` to be called immediately after.
fn with_blocking_io<F, R>(stream: &mut TcpStream, f: F) -> R
where
    F: FnOnce(&mut TcpStream) -> R,
{
    stream.set_nonblocking(false).ok();
    let result = f(stream);
    stream.set_nonblocking(true).ok();
    result
}

// Private implementation methods (not exposed to GDScript)
impl SpectatorTCPServer {
    fn try_accept(&mut self) {
        let listener = match &self.listener {
            Some(l) => l,
            None => return,
        };

        match listener.accept() {
            Ok((stream, addr)) => {
                godot_print!("[Spectator] Client connected from {}", addr);
                stream.set_nonblocking(true).ok();
                self.client = Some(stream);
                self.handshake_completed = false;
                self.send_handshake();
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                godot_error!("[Spectator] Accept error: {}", e);
            }
        }
    }

    fn send_handshake(&mut self) {
        let handshake = Handshake::new(
            self.get_godot_version(),
            self.detect_scene_dimensions(),
            self.get_physics_ticks(),
            self.get_project_name(),
        );
        let msg = Message::Handshake(handshake);

        if let Some(stream) = &mut self.client {
            let result = with_blocking_io(stream, |s| codec::write_message(s, &msg));
            match result {
                Ok(()) => godot_print!("[Spectator] Handshake sent"),
                Err(e) => {
                    godot_error!("[Spectator] Failed to send handshake: {}", e);
                    self.disconnect_client();
                }
            }
        }
    }

    fn try_read(&mut self) {
        let stream = match &mut self.client {
            Some(s) => s,
            None => return,
        };

        let result = with_blocking_io(stream, |s| {
            s.set_read_timeout(Some(std::time::Duration::from_millis(1))).ok();
            codec::read_message::<Message>(s)
        });

        match result {
            Ok(msg) => {
                self.handle_message(msg);
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
            {
                // No data available — normal
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::UnexpectedEof
                    || e.kind() == ErrorKind::ConnectionReset =>
            {
                godot_print!("[Spectator] Client disconnected");
                self.disconnect_client();
            }
            Err(e) => {
                godot_error!("[Spectator] Read error: {}", e);
                self.disconnect_client();
            }
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::HandshakeAck(ack) => {
                godot_print!(
                    "[Spectator] Handshake ACK received — session {}",
                    ack.session_id
                );
                self.handshake_completed = true;
            }
            Message::HandshakeError(err) => {
                godot_error!("[Spectator] Handshake rejected: {}", err.message);
                self.disconnect_client();
            }
            Message::Query { id, method, params } => {
                if method.starts_with("recording_") {
                    let response_msg = if let Some(ref mut recorder) = self.recorder {
                        match crate::recording_handler::handle_recording_query(
                            recorder, &method, &params,
                        ) {
                            Ok(data) => Message::Response { id, data },
                            Err((code, message)) => Message::Error { id, code, message },
                        }
                    } else {
                        Message::Error {
                            id,
                            code: "internal_error".to_string(),
                            message: "Recorder not available".to_string(),
                        }
                    };
                    self.send_response(response_msg);
                } else if let Some(ref collector) = self.collector {
                    let response = crate::query_handler::handle_query(
                        id,
                        &method,
                        params,
                        &collector.bind(),
                    );
                    // None means response is deferred (e.g., advance_frames)
                    if let Some(msg) = response {
                        self.send_response(msg);
                    }
                } else {
                    self.send_response(Message::Error {
                        id,
                        code: "scene_not_loaded".to_string(),
                        message: "Collector not available".to_string(),
                    });
                }
            }
            Message::Event { event, data } if event == "activity_log" => {
                let entry_type = data
                    .get("entry_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("query")
                    .to_string();
                let summary = data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_name = data
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let active_watches: i64 = data
                    .get("meta")
                    .and_then(|m| m.get("active_watches"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);
                self.base_mut().emit_signal(
                    "activity_received",
                    &[
                        GString::from(entry_type.as_str()).to_variant(),
                        GString::from(summary.as_str()).to_variant(),
                        GString::from(tool_name.as_str()).to_variant(),
                        active_watches.to_variant(),
                    ],
                );
            }
            _ => {
                godot_print!("[Spectator] Received unhandled message type");
            }
        }
    }

    fn send_response(&mut self, msg: Message) {
        if let Some(stream) = &mut self.client {
            let result = with_blocking_io(stream, |s| codec::write_message(s, &msg));
            if let Err(e) = result {
                godot_error!("[Spectator] Failed to send response: {}", e);
                self.disconnect_client();
            }
        }
    }

    fn disconnect_client(&mut self) {
        self.client = None;
        self.handshake_completed = false;
    }

    /// Returns true if a frame-advance is currently in progress (remaining > 0).
    fn is_advancing(&self) -> bool {
        self.collector
            .as_ref()
            .map(|c| c.bind().advance_state.borrow().remaining > 0)
            .unwrap_or(false)
    }

    /// Decrement the advance counter. If it reaches zero, re-pauses the tree
    /// and returns a deferred response message. Returns None if still advancing.
    fn check_frame_advance(&mut self) -> Option<Message> {
        let collector = self.collector.as_ref()?;
        let remaining = collector.bind().advance_state.borrow().remaining;
        if remaining == 0 {
            return None;
        }

        let new_remaining = remaining - 1;
        let (pending_id, frame) = {
            let bound = collector.bind();
            let mut state = bound.advance_state.borrow_mut();
            state.remaining = new_remaining;
            let id = if new_remaining == 0 {
                state.pending_id.take()
            } else {
                None
            };
            let f = if new_remaining == 0 {
                drop(state);
                bound.get_frame_info().frame
            } else {
                0
            };
            (id, f)
        };

        if new_remaining == 0 {
            // Re-pause the scene tree
            if let Some(mut tree) = self.base().get_tree() {
                tree.set_pause(true);
            }
            // Build and return the deferred response
            if let Some(id) = pending_id {
                let response = ActionResponse {
                    action: "advance_frames".into(),
                    result: "ok".into(),
                    details: serde_json::Map::from_iter([(
                        "new_frame".into(),
                        serde_json::json!(frame),
                    )]),
                    frame,
                };
                let data = serde_json::to_value(&response).unwrap_or(serde_json::Value::Null);
                return Some(Message::Response { id, data });
            }
        }

        None
    }

    fn get_godot_version(&self) -> String {
        let info = godot::classes::Engine::singleton().get_version_info();
        let major = info
            .get("major")
            .and_then(|v| v.try_to::<i32>().ok())
            .unwrap_or(0);
        let minor = info
            .get("minor")
            .and_then(|v| v.try_to::<i32>().ok())
            .unwrap_or(0);
        format!("{}.{}", major, minor)
    }

    fn detect_scene_dimensions(&self) -> u32 {
        3
    }

    fn get_physics_ticks(&self) -> u32 {
        godot::classes::Engine::singleton().get_physics_ticks_per_second() as u32
    }

    fn get_project_name(&self) -> String {
        godot::classes::ProjectSettings::singleton()
            .get_setting("application/config/name")
            .to::<GString>()
            .to_string()
    }
}
