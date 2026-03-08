/// Pure state machine for the addon-side TCP connection.
/// No Godot types — just state transitions and message decisions.
///
/// This is extracted from `SpectatorTCPServer` in spectator-godot so the
/// advance-frames logic can be unit-tested without a Godot runtime.
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub handshake_completed: bool,
    pub advance_remaining: u32,
    pub advance_request_id: Option<String>,
}

/// What the caller (GDExtension) should do after a state transition.
#[derive(Debug)]
pub enum ConnectionAction {
    /// No action needed.
    None,
    /// Disconnect the client.
    Disconnect,
    /// The advance is complete — re-pause and send a response for this request ID.
    AdvanceComplete { response_id: String, frame: u64 },
}

impl ConnectionState {
    pub fn on_client_connected(&mut self) {
        self.connected = true;
        self.handshake_completed = false;
    }

    pub fn on_handshake_ack(&mut self) {
        self.handshake_completed = true;
    }

    pub fn on_disconnect(&mut self) {
        self.connected = false;
        self.handshake_completed = false;
        self.advance_remaining = 0;
        self.advance_request_id = None;
    }

    /// Begin a frame advance. Returns `true` if accepted, `false` if one is already in progress.
    pub fn begin_advance(&mut self, frames: u32, request_id: String) -> bool {
        if self.advance_remaining > 0 {
            return false;
        }
        self.advance_remaining = frames;
        self.advance_request_id = Some(request_id);
        true
    }

    /// Called each physics tick. Returns an action if the advance just completed.
    /// The `current_frame` argument is passed in by the caller (avoids Godot dependency).
    pub fn tick_advance(&mut self, current_frame: u64) -> ConnectionAction {
        if self.advance_remaining == 0 {
            return ConnectionAction::None;
        }
        self.advance_remaining -= 1;
        if self.advance_remaining == 0 {
            if let Some(id) = self.advance_request_id.take() {
                return ConnectionAction::AdvanceComplete {
                    response_id: id,
                    frame: current_frame,
                };
            }
        }
        ConnectionAction::None
    }

    pub fn is_advancing(&self) -> bool {
        self.advance_remaining > 0
    }

    pub fn is_ready(&self) -> bool {
        self.connected && self.handshake_completed && !self.is_advancing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_disconnected() {
        let state = ConnectionState::default();
        assert!(!state.connected);
        assert!(!state.handshake_completed);
        assert!(!state.is_advancing());
        assert!(!state.is_ready());
    }

    #[test]
    fn connection_lifecycle() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        assert!(state.connected);
        assert!(!state.handshake_completed);
        assert!(!state.is_ready());

        state.on_handshake_ack();
        assert!(state.handshake_completed);
        assert!(state.is_ready());

        state.on_disconnect();
        assert!(!state.connected);
        assert!(!state.is_ready());
    }

    #[test]
    fn advance_frames_lifecycle() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();

        assert!(state.begin_advance(3, "req-1".into()));
        assert!(state.is_advancing());
        assert!(!state.is_ready());

        // Can't start another advance while one is in progress
        assert!(!state.begin_advance(5, "req-2".into()));

        // Tick down
        assert!(matches!(state.tick_advance(0), ConnectionAction::None));
        assert_eq!(state.advance_remaining, 2);

        assert!(matches!(state.tick_advance(0), ConnectionAction::None));
        assert_eq!(state.advance_remaining, 1);

        // Final tick completes
        match state.tick_advance(42) {
            ConnectionAction::AdvanceComplete { response_id, frame } => {
                assert_eq!(response_id, "req-1");
                assert_eq!(frame, 42);
            }
            other => panic!("Expected AdvanceComplete, got {:?}", other),
        }

        assert!(!state.is_advancing());
        assert!(state.is_ready());
    }

    #[test]
    fn advance_single_frame() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();

        assert!(state.begin_advance(1, "req-single".into()));
        assert!(state.is_advancing());

        match state.tick_advance(10) {
            ConnectionAction::AdvanceComplete { response_id, frame } => {
                assert_eq!(response_id, "req-single");
                assert_eq!(frame, 10);
            }
            other => panic!("Expected AdvanceComplete, got {:?}", other),
        }
        assert!(!state.is_advancing());
    }

    #[test]
    fn disconnect_during_advance_clears_state() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();
        state.begin_advance(10, "req-1".into());

        state.on_disconnect();
        assert!(!state.is_advancing());
        assert_eq!(state.advance_remaining, 0);
        assert!(state.advance_request_id.is_none());
    }

    #[test]
    fn tick_when_not_advancing_is_noop() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();

        // No advance in progress
        assert!(matches!(state.tick_advance(5), ConnectionAction::None));
        assert!(state.is_ready());
    }

    #[test]
    fn begin_advance_zero_frames_completes_immediately() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();

        // Zero frames: begin_advance sets remaining=0, immediately not advancing
        assert!(state.begin_advance(0, "req-zero".into()));
        // remaining is 0 so is_advancing() is false, but request_id is set
        assert!(!state.is_advancing());
    }

    #[test]
    fn reconnect_after_disconnect_resets_state() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack();
        state.begin_advance(5, "old-req".into());
        state.on_disconnect();

        // Reconnect
        state.on_client_connected();
        assert!(!state.handshake_completed);
        assert!(!state.is_advancing());

        state.on_handshake_ack();
        assert!(state.is_ready());

        // New advance works
        assert!(state.begin_advance(2, "new-req".into()));
    }
}
