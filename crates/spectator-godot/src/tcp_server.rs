use godot::obj::Gd;
use godot::prelude::*;
use spectator_protocol::{codec, handshake::Handshake, messages::Message};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};

use crate::collector::SpectatorCollector;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    client: Option<TcpStream>,
    port: i32,
    handshake_completed: bool,
    collector: Option<Gd<SpectatorCollector>>,
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
        }
    }
}

#[godot_api]
impl SpectatorTCPServer {
    /// Wire the collector into the TCP server.
    #[func]
    pub fn set_collector(&mut self, collector: Gd<SpectatorCollector>) {
        self.collector = Some(collector);
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
        if self.client.is_none() {
            self.try_accept();
        }

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
            Message::Query { id, method, params } => {
                if let Some(ref collector) = self.collector {
                    let response = crate::query_handler::handle_query(
                        id,
                        &method,
                        params,
                        &collector.bind(),
                    );
                    self.send_response(response);
                } else {
                    self.send_response(Message::Error {
                        id,
                        code: "scene_not_loaded".to_string(),
                        message: "Collector not available".to_string(),
                    });
                }
            }
            _ => {
                godot_print!("[Spectator] Received unhandled message type");
            }
        }
    }

    fn send_response(&mut self, msg: Message) {
        if let Some(stream) = &mut self.client {
            stream.set_nonblocking(false).ok();
            if let Err(e) = codec::write_message(stream, &msg) {
                godot_error!("[Spectator] Failed to send response: {}", e);
                self.disconnect_client();
                return;
            }
            if let Some(stream) = &self.client {
                stream.set_nonblocking(true).ok();
            }
        }
    }

    fn disconnect_client(&mut self) {
        self.client = None;
        self.handshake_completed = false;
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
