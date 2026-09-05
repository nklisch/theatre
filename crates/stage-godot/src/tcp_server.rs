use godot::classes::Object;
use godot::obj::Gd;
use godot::prelude::*;
use stage_protocol::query::{
    ActionResponse, INTERACTION_SEQUENCE_ENGINE_DEADLINE_SECS, InteractionSequenceStep,
};
use stage_protocol::{
    codec,
    connection_state::{ConnectionAction, ConnectionState},
    handshake::Handshake,
    messages::Message,
};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use crate::collector::StageCollector;
use crate::recorder::StageRecorder;

const MAX_CLIENTS: usize = 8;

struct ClientSlot {
    stream: TcpStream,
    handshake_complete: bool,
    last_activity_at: Option<std::time::Instant>,
}

const INTERACTION_SEQUENCE_DEADLINE: Duration =
    Duration::from_secs(INTERACTION_SEQUENCE_ENGINE_DEADLINE_SECS);

/// The one deferred action allowed to own physics-frame advancement.
enum PendingAction {
    AdvanceFrames {
        slot_idx: usize,
    },
    InteractionSequence {
        slot_idx: usize,
        request_id: String,
        steps: Vec<InteractionSequenceStep>,
        next_step: usize,
        owned_inputs: HashSet<String>,
        started_at: Instant,
        total_frames: u32,
    },
}

impl PendingAction {
    fn slot_idx(&self) -> usize {
        match self {
            Self::AdvanceFrames { slot_idx } | Self::InteractionSequence { slot_idx, .. } => {
                *slot_idx
            }
        }
    }

    fn is_sequence(&self) -> bool {
        matches!(self, Self::InteractionSequence { .. })
    }
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
    /// The single client-owned deferred action using the shared frame counter.
    pending_action: Option<PendingAction>,
    collector: Option<Gd<StageCollector>>,
    recorder: Option<Gd<StageRecorder>>,
    runtime_logger: Option<Gd<Object>>,
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
            pending_action: None,
            collector: None,
            recorder: None,
            runtime_logger: None,
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

    /// Share the engine-owned run identity with human feedback capture.
    #[func]
    pub fn get_run_id(&self) -> GString {
        GString::from(crate::runtime_identity::identity().run_id.as_str())
    }

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

    /// Wire the process-local GDScript Logger into the main-thread query bridge.
    #[func]
    pub fn set_runtime_logger(&mut self, logger: Gd<Object>) {
        self.runtime_logger = Some(logger);
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
        self.cancel_pending_action();
        self.conn_state.on_disconnect();
        godot_print!("[Stage] TCP server stopped");
    }

    /// Returns true if at least one client has completed the handshake.
    #[func]
    pub fn has_stage_connection(&self) -> bool {
        self.any_connected()
    }

    /// Poll for new connections and incoming messages. Call every _physics_process.
    #[func]
    pub fn poll(&mut self) {
        // Phase 1: deferred action progress and completion.
        if self.check_pending_deadline() {
            return;
        }
        if let Some((slot_idx, message)) = self.check_frame_advance() {
            self.send_response_to_slot(slot_idx, message);
            return;
        }
        if self.is_advancing() {
            // Ordinary frame advance preserves its historical exclusive polling.
            // Sequences must continue reading sockets so owner loss cannot leave
            // synthetic input held until all requested frames elapse.
            if self
                .pending_action
                .as_ref()
                .is_some_and(PendingAction::is_sequence)
            {
                self.try_accept();
                self.poll_sequence_connections();
            }
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
            crate::runtime_identity::identity().clone(),
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
                if method == "execute_action" && self.pending_action.is_some() {
                    self.send_response_to_slot(
                        slot_idx,
                        Message::Error {
                            request_id,
                            code: "action_in_progress".into(),
                            message: "Another action owns physics-frame advancement; retry after it completes.".into(),
                        },
                    );
                    return;
                }
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
                        self.runtime_logger.as_ref(),
                    );
                    match response {
                        Some(msg) => self.send_response_to_slot(slot_idx, msg),
                        // Deferred action: transfer validated state into the one
                        // TCP-owned lifecycle before returning to the poll loop.
                        None => self.sync_deferred_action_from_collector(slot_idx),
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
        // Owner loss cancels the whole deferred lifecycle. For sequences this
        // also releases only sequence-owned presses and restores pause.
        if self.pending_action.as_ref().map(PendingAction::slot_idx) == Some(slot_idx) {
            self.cancel_pending_action();
            godot_print!(
                "[Stage] Slot {} disconnected during deferred action — action cancelled",
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

    fn poll_sequence_connections(&mut self) {
        // Probe the owner first so traffic from another client cannot starve
        // disconnect detection and prolong synthetic input ownership.
        let owner = self.pending_action.as_ref().map(PendingAction::slot_idx);
        let mut query_processed = owner.is_some_and(|slot_idx| self.try_read_query(slot_idx));
        if self.pending_action.is_none() {
            return;
        }
        for slot_idx in 0..self.clients.len() {
            if Some(slot_idx) == owner || self.clients[slot_idx].is_none() {
                continue;
            }
            let handshake_complete = self.clients[slot_idx]
                .as_ref()
                .is_some_and(|slot| slot.handshake_complete);
            if !handshake_complete {
                self.try_read_handshake(slot_idx);
            } else if !query_processed && self.try_read_query(slot_idx) {
                query_processed = true;
            }
            // Valid sequence playback intentionally suppresses ordinary idle
            // expiry, but socket reads above still detect connection loss.
        }
    }

    fn check_pending_deadline(&mut self) -> bool {
        let expired = matches!(
            self.pending_action.as_ref(),
            Some(PendingAction::InteractionSequence { started_at, .. })
                if started_at.elapsed() >= INTERACTION_SEQUENCE_DEADLINE
        );
        if !expired {
            return false;
        }
        let Some(PendingAction::InteractionSequence {
            slot_idx,
            request_id,
            mut owned_inputs,
            ..
        }) = self.pending_action.take()
        else {
            return false;
        };
        crate::action_handler::release_sequence_inputs(&mut owned_inputs);
        self.pause_scene_and_cancel_advance();
        self.send_response_to_slot(
            slot_idx,
            Message::Error {
                request_id,
                code: "sequence_timeout".into(),
                message: "Interaction sequence exceeded the 30-second engine deadline; owned inputs were released and the scene was paused.".into(),
            },
        );
        true
    }

    /// Tick the shared advance counter and either continue the next sequence
    /// step or finish the deferred response.
    fn check_frame_advance(&mut self) -> Option<(usize, Message)> {
        let current_frame = self
            .collector
            .as_ref()
            .map(|c| c.bind().get_frame_info().frame)
            .unwrap_or(0);
        let ConnectionAction::AdvanceComplete { response_id, frame } =
            self.conn_state.tick_advance(current_frame)
        else {
            return None;
        };
        let pending = self.pending_action.take()?;
        match pending {
            PendingAction::AdvanceFrames { slot_idx } => {
                self.pause_scene_and_cancel_advance();
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
                Some((
                    slot_idx,
                    Message::Response {
                        request_id: response_id,
                        data,
                    },
                ))
            }
            PendingAction::InteractionSequence {
                slot_idx,
                request_id,
                steps,
                mut next_step,
                mut owned_inputs,
                started_at,
                total_frames,
            } => {
                if next_step < steps.len() {
                    let step = &steps[next_step];
                    crate::action_handler::apply_sequence_step(step, &mut owned_inputs);
                    let started = self
                        .conn_state
                        .begin_advance(step.frames, request_id.clone());
                    debug_assert!(started, "completed step must leave frame counter idle");
                    next_step += 1;
                    self.pending_action = Some(PendingAction::InteractionSequence {
                        slot_idx,
                        request_id,
                        steps,
                        next_step,
                        owned_inputs,
                        started_at,
                        total_frames,
                    });
                    None
                } else {
                    crate::action_handler::release_sequence_inputs(&mut owned_inputs);
                    self.pause_scene_and_cancel_advance();
                    let response = ActionResponse {
                        action: "interaction_sequence".into(),
                        result: "ok".into(),
                        details: serde_json::Map::from_iter([
                            ("steps_completed".into(), serde_json::json!(steps.len())),
                            ("frames_advanced".into(), serde_json::json!(total_frames)),
                            ("new_frame".into(), serde_json::json!(frame)),
                        ]),
                        frame,
                    };
                    let data = serde_json::to_value(&response).unwrap_or(serde_json::Value::Null);
                    Some((slot_idx, Message::Response { request_id, data }))
                }
            }
        }
    }

    fn sync_deferred_action_from_collector(&mut self, slot_idx: usize) {
        let action = self.collector.as_ref().and_then(|collector| {
            let bound = collector.bind();
            bound.deferred_action.borrow_mut().take()
        });
        let Some(action) = action else {
            godot_error!("[Stage] Deferred action response had no lifecycle state");
            return;
        };
        match action {
            crate::action_handler::DeferredAction::AdvanceFrames { frames, request_id } => {
                if self.conn_state.begin_advance(frames, request_id) {
                    self.pending_action = Some(PendingAction::AdvanceFrames { slot_idx });
                    self.set_scene_paused(false);
                }
            }
            crate::action_handler::DeferredAction::InteractionSequence { steps, request_id } => {
                let total_frames = steps.iter().map(|step| step.frames).sum();
                let mut owned_inputs = HashSet::new();
                crate::action_handler::apply_sequence_step(&steps[0], &mut owned_inputs);
                if self
                    .conn_state
                    .begin_advance(steps[0].frames, request_id.clone())
                {
                    self.pending_action = Some(PendingAction::InteractionSequence {
                        slot_idx,
                        request_id,
                        steps,
                        next_step: 1,
                        owned_inputs,
                        started_at: Instant::now(),
                        total_frames,
                    });
                    self.set_scene_paused(false);
                } else {
                    crate::action_handler::release_sequence_inputs(&mut owned_inputs);
                    self.set_scene_paused(true);
                }
            }
        }
    }

    fn cancel_pending_action(&mut self) {
        // Stopping an idle listener must not change the game's pause state.
        let Some(action) = self.pending_action.take() else {
            return;
        };
        if let PendingAction::InteractionSequence {
            mut owned_inputs, ..
        } = action
        {
            crate::action_handler::release_sequence_inputs(&mut owned_inputs);
        }
        self.pause_scene_and_cancel_advance();
    }

    fn pause_scene_and_cancel_advance(&mut self) {
        self.conn_state.cancel_advance();
        self.set_scene_paused(true);
    }

    fn set_scene_paused(&self, paused: bool) {
        if let Some(mut tree) = self.base().get_tree_or_null() {
            tree.set_pause(paused);
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
        let Some(tree) = self.base().get_tree_or_null() else {
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
