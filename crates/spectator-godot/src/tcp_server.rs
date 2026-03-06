use godot::prelude::*;
use spectator_protocol::{codec, handshake::Handshake, messages::Message};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    client: Option<TcpStream>,
    port: i32,
    handshake_completed: bool,
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
        }
    }
}

#[godot_api]
impl SpectatorTCPServer {
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
        // Accept new connections if we don't have one
        if self.client.is_none() {
            self.try_accept();
        }

        // Read incoming messages from connected client
        if self.client.is_some() {
            self.try_read();
        }
    }
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
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // No pending connection — normal for non-blocking
            }
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
            // Temporarily set blocking for the handshake write
            stream.set_nonblocking(false).ok();
            match codec::write_message(stream, &msg) {
                Ok(()) => {
                    godot_print!("[Spectator] Handshake sent");
                }
                Err(e) => {
                    godot_error!("[Spectator] Failed to send handshake: {}", e);
                    self.disconnect_client();
                    return;
                }
            }
            if let Some(stream) = &self.client {
                stream.set_nonblocking(true).ok();
            }
        }
    }

    fn try_read(&mut self) {
        let stream = match &mut self.client {
            Some(s) => s,
            None => return,
        };

        // Temporarily set blocking with a very short timeout for reads
        stream.set_nonblocking(false).ok();
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(1)))
            .ok();

        match codec::read_message::<Message>(stream) {
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
                return;
            }
            Err(e) => {
                godot_error!("[Spectator] Read error: {}", e);
                self.disconnect_client();
                return;
            }
        }

        // Restore non-blocking
        if let Some(stream) = &self.client {
            stream.set_nonblocking(true).ok();
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
            _ => {
                // M0: ignore other message types
                godot_print!("[Spectator] Received message (unhandled in M0)");
            }
        }
    }

    fn disconnect_client(&mut self) {
        self.client = None;
        self.handshake_completed = false;
    }

    fn get_godot_version(&self) -> String {
        let info = godot::classes::Engine::singleton().get_version_info();
        let major = info.get("major").unwrap_or(Variant::from(0)).to::<i32>();
        let minor = info.get("minor").unwrap_or(Variant::from(0)).to::<i32>();
        format!("{}.{}", major, minor)
    }

    fn detect_scene_dimensions(&self) -> u32 {
        // M0: default to 3. Full detection in M9 (2D support).
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
