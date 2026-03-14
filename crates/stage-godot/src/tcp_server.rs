use godot::obj::Gd;
use godot::prelude::*;
use stage_protocol::query::ActionResponse;
use stage_protocol::{
    codec,
    connection_state::{ConnectionAction, ConnectionState},
    handshake::Handshake,
    messages::Message,
};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};

use crate::collector::StageCollector;
use crate::recorder::StageRecorder;

const MAX_CLIENTS: usize = 8;

struct ClientSlot {
    stream: TcpStream,
    handshake_complete: bool,
    last_activity_at: Option<std::time::Instant>,
}

/// Tracks which client slot initiated the current frame advance.
struct PendingAdvance {
    slot_idx: usize,
}

#[derive(GodotClass)]
#[class(base = Node)]
pub struct StageTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    /// Sparse vec of client slots; None = empty slot.
    clients: Vec<Option<ClientSlot>>,
    port: i32,
    /// Frame-advance state machine. The connected/handshake_completed fields are
    /// unused — per-slot state in `clients` is authoritative for connection status.
    conn_state: ConnectionState,
    /// Which client slot owns the current deferred frame-advance response.
    pending_advance: Option<PendingAdvance>,
    collector: Option<Gd<StageCollector>>,
    recorder: Option<Gd<StageRecorder>>,
    /// Seconds of silence on a handshaked connection before treating it as a zombie.
    /// 0 = disabled.
    client_idle_timeout_secs: u64,
}

#[godot_api]
impl INode for StageTCPServer {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            listener: None,
            clients: Vec::new(),
            port: 9077,
            conn_state: ConnectionState::default(),
            pending_advance: None,
            collector: None,
            recorder: None,
            client_idle_timeout_secs: 10,
        }
    }
}

#[godot_api]
impl StageTCPServer {
    /// Emitted when an activity_log event is received from the server.
    /// `active_watches` is the current watch count from meta (-1 if not provided).
    #[signal]
    fn activity_received(
        entry_type: GString,
        summary: GString,
        tool_name: GString,
        active_watches: i64,
    );

    /// Wire the collector into the TCP server.
    #[func]
    pub fn set_collector(&mut self, collector: Gd<StageCollector>) {
        self.collector = Some(collector);
    }

    /// Wire the recorder into the TCP server.
    #[func]
    pub fn set_recorder(&mut self, recorder: Gd<StageRecorder>) {
        self.recorder = Some(recorder);
    }

    /// Set the client idle timeout in seconds. 0 disables the timeout. Default: 10.
    #[func]
    pub fn set_idle_timeout(&mut self, secs: i64) {
        self.client_idle_timeout_secs = secs.max(0) as u64;
    }

    /// Returns "connected" if any slot has completed handshake, "waiting" if the
    /// listener is active but no connected clients, or "stopped".
    #[func]
    pub fn get_connection_status(&self) -> GString {
        if self.any_connected() {
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
                godot_print!("[Stage] TCP server listening on {}", addr);
            }
            Err(e) => {
                godot_error!("[Stage] Failed to bind to {}: {}", addr, e);
            }
        }
    }

    /// Stop listening and close all active connections.
    #[func]
    pub fn stop(&mut self) {
        self.clients.clear();
        self.listener = None;
        self.pending_advance = None;
        self.conn_state.on_disconnect();
        godot_print!("[Stage] TCP server stopped");
    }

    /// Returns true if at least one client has completed the handshake.
    #[func]
    pub fn is_connected(&self) -> bool {
        self.any_connected()
    }

    /// Poll for new connections and incoming messages. Call every _physics_process.
    #[func]
    pub fn poll(&mut self) {
        // Phase 1: frame-advance completion
        if let Some(advance_msg) = self.check_frame_advance() {
            if let Some(pa) = self.pending_advance.take() {
                self.send_response_to_slot(pa.slot_idx, advance_msg);
            }
            return;
        }
        // Skip new queries while a frame advance is in progress
        if self.is_advancing() {
            return;
        }

        // Phase 2: accept new connections (unconditional — up to MAX_CLIENTS)
        self.try_accept();

        // Phase 3: per-slot I/O — at most one query dispatched per tick
        let mut query_processed = false;
        for slot_idx in 0..self.clients.len() {
            if self.clients[slot_idx].is_none() {
                continue;
            }
            let handshake_complete = self.clients[slot_idx]
                .as_ref()
                .map(|s| s.handshake_complete)
                .unwrap_or(false);

            if !handshake_complete {
                self.try_read_handshake(slot_idx);
            } else if !query_processed {
                if self.try_read_query(slot_idx) {
                    query_processed = true;
                } else {
                    self.check_idle_timeout(slot_idx);
                }
            } else {
                // Already processed a query this tick — still check idle timeout
                self.check_idle_timeout(slot_idx);
            }
        }
    }
}

/// Run `f` with the stream in blocking mode, then restore non-blocking.
///
/// Using a closure means the mutable borrow of the stream is contained inside
/// `f` and released before the caller handles the result — NLL then allows
/// `self.disconnect_slot()` to be called immediately after.
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
impl StageTCPServer {
    fn any_connected(&self) -> bool {
        self.clients
            .iter()
            .any(|s| s.as_ref().map(|c| c.handshake_complete).unwrap_or(false))
    }

    fn try_accept(&mut self) {
        let filled = self.clients.iter().filter(|s| s.is_some()).count();
        if filled >= MAX_CLIENTS {
            return;
        }

        let listener = match &self.listener {
            Some(l) => l,
            None => return,
        };

        match listener.accept() {
            Ok((stream, addr)) => {
                stream.set_nonblocking(true).ok();
                let slot = ClientSlot {
                    stream,
                    handshake_complete: false,
                    last_activity_at: None,
                };
                let slot_idx = match self.clients.iter().position(|s| s.is_none()) {
                    Some(i) => {
                        self.clients[i] = Some(slot);
                        i
                    }
                    None => {
                        self.clients.push(Some(slot));
                        self.clients.len() - 1
                    }
                };
                godot_print!("[Stage] Client connected from {} (slot {})", addr, slot_idx);
                self.send_handshake_to_slot(slot_idx);
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                godot_error!("[Stage] Accept error: {}", e);
            }
        }
    }

    fn send_handshake_to_slot(&mut self, slot_idx: usize) {
        let handshake = Handshake::new(
            self.get_godot_version(),
            self.detect_scene_dimensions(),
            self.get_physics_ticks(),
            self.get_project_name(),
        );
        let msg = Message::Handshake(handshake);

        let result = match self.clients.get_mut(slot_idx).and_then(|s| s.as_mut()) {
            Some(slot) => with_blocking_io(&mut slot.stream, |s| codec::write_message(s, &msg)),
            None => return,
        };

        match result {
            Ok(()) => godot_print!("[Stage] Handshake sent to slot {}", slot_idx),
            Err(e) => {
                godot_error!(
                    "[Stage] Failed to send handshake to slot {}: {}",
                    slot_idx,
                    e
                );
                self.disconnect_slot(slot_idx);
            }
        }
    }

    fn try_read_handshake(&mut self, slot_idx: usize) {
        let result = match self.clients.get_mut(slot_idx).and_then(|s| s.as_mut()) {
            Some(slot) => with_blocking_io(&mut slot.stream, |s| {
                s.set_read_timeout(Some(std::time::Duration::from_millis(1)))
                    .ok();
                codec::read_message::<Message>(s)
            }),
            None => return,
        };

        match result {
            Ok(Message::HandshakeAck(ack)) => {
                godot_print!(
                    "[Stage] Handshake ACK from slot {} — session {}",
                    slot_idx,
                    ack.session_id
                );
                if let Some(Some(slot)) = self.clients.get_mut(slot_idx) {
                    slot.handshake_complete = true;
                    slot.last_activity_at = Some(std::time::Instant::now());
                }
            }
            Ok(Message::HandshakeError(err)) => {
                godot_error!(
                    "[Stage] Handshake rejected by slot {}: {}",
                    slot_idx,
                    err.message
                );
                self.disconnect_slot(slot_idx);
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::UnexpectedEof
                    || e.kind() == ErrorKind::ConnectionReset =>
            {
                godot_print!("[Stage] Slot {} disconnected during handshake", slot_idx);
                self.disconnect_slot(slot_idx);
            }
            Err(e) => {
                godot_error!("[Stage] Handshake read error on slot {}: {}", slot_idx, e);
                self.disconnect_slot(slot_idx);
            }
            Ok(_) => {
                godot_error!(
                    "[Stage] Unexpected message before handshake on slot {}",
                    slot_idx
                );
                self.disconnect_slot(slot_idx);
            }
        }
    }

    /// Try to read one query from a post-handshake slot.
    /// Returns `true` if a query was dispatched, `false` if no data was available.
    fn try_read_query(&mut self, slot_idx: usize) -> bool {
        let result = match self.clients.get_mut(slot_idx).and_then(|s| s.as_mut()) {
            Some(slot) => with_blocking_io(&mut slot.stream, |s| {
                s.set_read_timeout(Some(std::time::Duration::from_millis(1)))
                    .ok();
                codec::read_message::<Message>(s)
            }),
            None => return false,
        };

        match result {
            Ok(msg) => {
                if let Some(Some(slot)) = self.clients.get_mut(slot_idx) {
                    slot.last_activity_at = Some(std::time::Instant::now());
                }
                self.handle_query_message(slot_idx, msg);
                true
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
            {
                false
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::UnexpectedEof
                    || e.kind() == ErrorKind::ConnectionReset =>
            {
                godot_print!("[Stage] Client in slot {} disconnected", slot_idx);
                self.disconnect_slot(slot_idx);
                false
            }
            Err(e) => {
                godot_error!("[Stage] Read error on slot {}: {}", slot_idx, e);
                self.disconnect_slot(slot_idx);
                false
            }
        }
    }

    fn handle_query_message(&mut self, slot_idx: usize, msg: Message) {
        match msg {
            Message::Query {
                request_id,
                method,
                params,
            } => {
                if method.starts_with("recording_") || method.starts_with("dashcam_") {
                    let response_msg = if let Some(ref mut recorder) = self.recorder {
                        match crate::recording_handler::handle_recording_query(
                            recorder, &method, &params,
                        ) {
                            Ok(data) => Message::Response { request_id, data },
                            Err((code, message)) => Message::Error {
                                request_id,
                                code,
                                message,
                            },
                        }
                    } else {
                        Message::Error {
                            request_id,
                            code: "internal_error".to_string(),
                            message: "Recorder not available".to_string(),
                        }
                    };
                    self.send_response_to_slot(slot_idx, response_msg);
                } else if let Some(ref collector) = self.collector {
                    let response = crate::query_handler::handle_query(
                        request_id,
                        &method,
                        params,
                        &collector.bind(),
                    );
                    match response {
                        Some(msg) => self.send_response_to_slot(slot_idx, msg),
                        // None = deferred (advance_frames): record which slot owns the response
                        None => {
                            self.pending_advance = Some(PendingAdvance { slot_idx });
                            self.sync_advance_from_collector();
                        }
                    }
                } else {
                    self.send_response_to_slot(
                        slot_idx,
                        Message::Error {
                            request_id,
                            code: "scene_not_loaded".to_string(),
                            message: "Collector not available".to_string(),
                        },
                    );
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
                godot_print!(
                    "[Stage] Received unhandled message type on slot {}",
                    slot_idx
                );
            }
        }
    }

    fn send_response_to_slot(&mut self, slot_idx: usize, msg: Message) {
        let result = match self.clients.get_mut(slot_idx).and_then(|s| s.as_mut()) {
            Some(slot) => with_blocking_io(&mut slot.stream, |s| codec::write_message(s, &msg)),
            None => return,
        };
        if let Err(e) = result {
            godot_error!(
                "[Stage] Failed to send response to slot {}: {}",
                slot_idx,
                e
            );
            self.disconnect_slot(slot_idx);
        }
    }

    fn disconnect_slot(&mut self, slot_idx: usize) {
        if slot_idx < self.clients.len() {
            self.clients[slot_idx] = None;
        }
        // If this slot owned a pending frame advance, cancel it
        if self.pending_advance.as_ref().map(|pa| pa.slot_idx) == Some(slot_idx) {
            self.pending_advance = None;
            self.conn_state.on_disconnect();
            godot_print!(
                "[Stage] Slot {} disconnected during frame advance — advance cancelled",
                slot_idx
            );
        }
    }

    fn check_idle_timeout(&mut self, slot_idx: usize) {
        if self.client_idle_timeout_secs == 0 {
            return;
        }
        let timed_out = self
            .clients
            .get(slot_idx)
            .and_then(|s| s.as_ref())
            .and_then(|slot| slot.last_activity_at)
            .map(|last| last.elapsed().as_secs() > self.client_idle_timeout_secs)
            .unwrap_or(false);
        if timed_out {
            godot_print!(
                "[Stage] Slot {} idle timeout — dropping zombie connection",
                slot_idx
            );
            self.disconnect_slot(slot_idx);
        }
    }

    fn is_advancing(&self) -> bool {
        self.conn_state.is_advancing()
    }

    /// Tick the advance counter. If the advance just completed, re-pauses the
    /// scene tree and returns the deferred response message.
    fn check_frame_advance(&mut self) -> Option<Message> {
        let current_frame = self
            .collector
            .as_ref()
            .map(|c| c.bind().get_frame_info().frame)
            .unwrap_or(0);

        match self.conn_state.tick_advance(current_frame) {
            ConnectionAction::AdvanceComplete { response_id, frame } => {
                if let Some(mut tree) = self.base().get_tree() {
                    tree.set_pause(true);
                }
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
                Some(Message::Response {
                    request_id: response_id,
                    data,
                })
            }
            _ => None,
        }
    }

    /// Sync advance state written by action_handler (into the collector) into
    /// `conn_state` so that `ConnectionState` becomes the authoritative tracker.
    /// Called after `handle_query` returns `None` (deferred response).
    fn sync_advance_from_collector(&mut self) {
        if let Some(ref collector) = self.collector {
            let bound = collector.bind();
            let mut state = bound.advance_state.borrow_mut();
            if state.remaining > 0 {
                let frames = state.remaining;
                let id = state.pending_id.take().unwrap_or_default();
                state.remaining = 0; // transferred to conn_state
                drop(state);
                self.conn_state.begin_advance(frames, id);
            }
        }
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
        let Some(tree) = self.base().get_tree() else {
            return 3;
        };
        let Some(root) = tree.get_current_scene() else {
            return 3;
        };
        let root_node: godot::obj::Gd<godot::classes::Node> = root.upcast();

        let has_2d = Self::has_node_type_recursive(&root_node, true);
        let has_3d = Self::has_node_type_recursive(&root_node, false);

        match (has_2d, has_3d) {
            (true, false) => 2,
            (false, true) => 3,
            (true, true) => 0,   // mixed
            (false, false) => 3, // default to 3D if no spatial nodes
        }
    }

    fn has_node_type_recursive(
        node: &godot::obj::Gd<godot::classes::Node>,
        check_2d: bool,
    ) -> bool {
        if check_2d {
            if node.clone().try_cast::<godot::classes::Node2D>().is_ok() {
                return true;
            }
        } else if node.clone().try_cast::<godot::classes::Node3D>().is_ok() {
            return true;
        }
        let count = node.get_child_count();
        for i in 0..count {
            if let Some(child) = node.get_child(i)
                && Self::has_node_type_recursive(&child, check_2d)
            {
                return true;
            }
        }
        false
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
