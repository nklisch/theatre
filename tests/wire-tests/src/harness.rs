use stage_protocol::{
    codec,
    handshake::{Handshake, HandshakeAck, PROTOCOL_VERSION},
    messages::Message,
};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A live Godot process + connected TCP stream, with handshake completed.
///
/// Dropped via `impl Drop` which kills the Godot process.
pub struct GodotFixture {
    child: Option<Child>,
    pub port: u16,
    stream: TcpStream,
    pub handshake: Handshake,
}

impl GodotFixture {
    /// Launch Godot headless with the test project, connect, and complete the handshake.
    ///
    /// Set `GODOT_BIN` env var to override the default `godot` binary name.
    /// `THEATRE_PORT` is passed to Godot automatically via this method.
    pub fn start(scene: &str) -> anyhow::Result<Self> {
        Self::start_with_timing(scene, true)
    }

    /// Launch without `--fixed-fps` so wall-clock lifecycle tests run in real time.
    pub fn start_realtime(scene: &str) -> anyhow::Result<Self> {
        Self::start_with_timing(scene, false)
    }

    fn start_with_timing(scene: &str, fixed_fps: bool) -> anyhow::Result<Self> {
        let port = portpicker::pick_unused_port()
            .ok_or_else(|| anyhow::anyhow!("no free port available"))?;

        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        let project_dir = Self::project_dir();
        let mut command = Command::new(&godot_bin);
        command.arg("--headless");
        if fixed_fps {
            command.args(["--fixed-fps", "60"]);
        }
        let mut child = command
            .args(["--path", &project_dir.to_string_lossy(), scene])
            .env("THEATRE_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to launch Godot ({godot_bin}): {e}"))?;

        // Wait for the GDExtension to start listening
        let mut stream = match Self::wait_for_connection(port, Duration::from_secs(15)) {
            Ok(stream) => stream,
            Err(error) => {
                terminate_process_tree(&mut child);
                anyhow::bail!("Godot did not open port {port} in time: {error}");
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;

        // Read the handshake message
        let msg = codec::read_message::<Message>(&mut stream)?;
        let handshake = match msg {
            Message::Handshake(h) => h,
            other => anyhow::bail!("Expected Handshake, got {:?}", other),
        };

        // Send HandshakeAck to complete the connection
        let ack = Message::HandshakeAck(HandshakeAck {
            stage_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            session_id: "wire-test-session".into(),
        });
        codec::write_message(&mut stream, &ack)?;

        Ok(Self {
            child: Some(child),
            port,
            stream,
            handshake,
        })
    }

    /// Override the response timeout for a deliberately long-running query.
    pub fn set_read_timeout(&self, timeout: Duration) -> anyhow::Result<()> {
        self.stream.set_read_timeout(Some(timeout))?;
        Ok(())
    }

    /// Send a query without waiting, for connection-loss lifecycle tests.
    pub fn send_query_without_waiting(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        codec::write_message(
            &mut self.stream,
            &Message::Query {
                request_id: uuid_simple(),
                method: method.into(),
                params,
            },
        )?;
        Ok(())
    }

    /// Send a query and wait for the matching response.
    pub fn query(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<QueryResult> {
        let id = uuid_simple();
        let msg = Message::Query {
            request_id: id.clone(),
            method: method.into(),
            params,
        };
        codec::write_message(&mut self.stream, &msg)?;

        let response = codec::read_message::<Message>(&mut self.stream)?;
        match response {
            Message::Response {
                request_id: rid,
                data,
            } if rid == id => Ok(QueryResult::Ok(data)),
            Message::Error {
                request_id: rid,
                code,
                message,
            } if rid == id => Ok(QueryResult::Err { code, message }),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Issue one query from an additional handshaked client.
    pub fn query_from_additional_client(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<QueryResult> {
        let mut stream = Self::wait_for_connection(self.port, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        match codec::read_message::<Message>(&mut stream)? {
            Message::Handshake(_) => {}
            other => anyhow::bail!("Expected Handshake, got {other:?}"),
        }
        codec::write_message(
            &mut stream,
            &Message::HandshakeAck(HandshakeAck {
                stage_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: PROTOCOL_VERSION,
                session_id: "wire-test-additional-client".into(),
            }),
        )?;
        let id = uuid_simple();
        codec::write_message(
            &mut stream,
            &Message::Query {
                request_id: id.clone(),
                method: method.into(),
                params,
            },
        )?;
        match codec::read_message::<Message>(&mut stream)? {
            Message::Response { request_id, data } if request_id == id => Ok(QueryResult::Ok(data)),
            Message::Error {
                request_id,
                code,
                message,
            } if request_id == id => Ok(QueryResult::Err { code, message }),
            other => anyhow::bail!("Unexpected response: {other:?}"),
        }
    }

    fn wait_for_connection(port: u16, timeout: Duration) -> anyhow::Result<TcpStream> {
        let deadline = Instant::now() + timeout;
        loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => return Ok(stream),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => anyhow::bail!("Godot not listening on {port}: {e}"),
            }
        }
    }

    fn project_dir() -> std::path::PathBuf {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../godot-project")
            .canonicalize()
            .expect("tests/godot-project dir must exist");
        #[cfg(windows)]
        if let Some(path) = path.to_string_lossy().strip_prefix(r"\\?\") {
            return std::path::PathBuf::from(path);
        }
        path
    }
}

impl GodotFixture {
    /// Close the TCP connection without killing the Godot process.
    /// Returns the port and child process so tests can reconnect to the same process.
    /// Drop the returned `Child` when done to kill Godot.
    pub fn disconnect_keep_alive(mut self) -> (u16, OwnedGodotChild) {
        let port = self.port;
        let child = self.child.take().expect("child already taken");
        // self.stream is dropped here, closing the TCP connection (sends FIN to Godot)
        (port, OwnedGodotChild(Some(child)))
    }

    /// Reconnect to an already-running Godot process after a deliberate client drop.
    pub fn reconnect(port: u16, mut owned: OwnedGodotChild) -> anyhow::Result<Self> {
        let mut stream = Self::wait_for_connection(port, Duration::from_secs(15))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let handshake = match codec::read_message::<Message>(&mut stream)? {
            Message::Handshake(handshake) => handshake,
            other => anyhow::bail!("Expected Handshake, got {other:?}"),
        };
        codec::write_message(
            &mut stream,
            &Message::HandshakeAck(HandshakeAck {
                stage_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: PROTOCOL_VERSION,
                session_id: "wire-test-reconnect".into(),
            }),
        )?;
        Ok(Self {
            child: owned.0.take(),
            port,
            stream,
            handshake,
        })
    }
}

impl Drop for GodotFixture {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_process_tree(&mut child);
        }
    }
}

/// Keeps a disconnected fixture's Godot process alive for reconnect tests,
/// then guarantees the same process-tree cleanup as the fixture itself.
pub struct OwnedGodotChild(Option<Child>);

impl Drop for OwnedGodotChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            terminate_process_tree(&mut child);
        }
    }
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    let _ = child.kill();
    let _ = child.wait();
}

/// Result of a query — either response data or an error.
pub enum QueryResult {
    Ok(serde_json::Value),
    Err { code: String, message: String },
}

impl QueryResult {
    pub fn unwrap_data(self) -> serde_json::Value {
        match self {
            Self::Ok(data) => data,
            Self::Err { code, message } => {
                panic!("Expected Ok, got error: {code}: {message}")
            }
        }
    }

    pub fn unwrap_err(self) -> (String, String) {
        match self {
            Self::Err { code, message } => (code, message),
            Self::Ok(data) => panic!("Expected error, got data: {data}"),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Self::Err { .. })
    }
}

fn uuid_simple() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Combine thread id and nanos for a cheap unique-enough ID
    let tid = std::thread::current().id();
    format!("{tid:?}-{nanos:08x}")
}

/// Assert that two f64 values are approximately equal (within 0.01).
pub fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected ~{expected}, got {actual}"
    );
}

/// Find an entity in snapshot data by name fragment. Panics if not found.
pub fn find_entity<'a>(data: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    data["entities"]
        .as_array()
        .expect("entities array missing")
        .iter()
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains(name))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("entity containing '{name}' not found in snapshot"))
}
