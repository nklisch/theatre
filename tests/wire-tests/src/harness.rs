use spectator_protocol::{
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
    child: Child,
    pub port: u16,
    stream: TcpStream,
    pub handshake: Handshake,
}

impl GodotFixture {
    /// Launch Godot headless with the test project, connect, and complete the handshake.
    ///
    /// Set `GODOT_BIN` env var to override the default `godot` binary name.
    /// Set `SPECTATOR_PORT` is passed to Godot automatically via this method.
    pub fn start(scene: &str) -> anyhow::Result<Self> {
        let port = portpicker::pick_unused_port()
            .ok_or_else(|| anyhow::anyhow!("no free port available"))?;

        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        let project_dir = Self::project_dir();

        let child = Command::new(&godot_bin)
            .args([
                "--headless",
                "--fixed-fps",
                "60",
                "--path",
                &project_dir.to_string_lossy(),
                scene,
            ])
            .env("SPECTATOR_PORT", port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to launch Godot ({godot_bin}): {e}"))?;

        // Wait for the GDExtension to start listening
        let mut stream = Self::wait_for_connection(port, Duration::from_secs(15))
            .map_err(|e| anyhow::anyhow!("Godot did not open port {port} in time: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;

        // Read the handshake message
        let msg = codec::read_message::<Message>(&mut stream)?;
        let handshake = match msg {
            Message::Handshake(h) => h,
            other => anyhow::bail!("Expected Handshake, got {:?}", other),
        };

        // Send HandshakeAck to complete the connection
        let ack = Message::HandshakeAck(HandshakeAck {
            spectator_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            session_id: "wire-test-session".into(),
        });
        codec::write_message(&mut stream, &ack)?;

        Ok(Self { child, port, stream, handshake })
    }

    /// Send a query and wait for the matching response.
    pub fn query(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<QueryResult> {
        let id = uuid_simple();
        let msg = Message::Query {
            id: id.clone(),
            method: method.into(),
            params,
        };
        codec::write_message(&mut self.stream, &msg)?;

        let response = codec::read_message::<Message>(&mut self.stream)?;
        match response {
            Message::Response { id: rid, data } if rid == id => Ok(QueryResult::Ok(data)),
            Message::Error { id: rid, code, message } if rid == id => {
                Ok(QueryResult::Err { code, message })
            }
            other => anyhow::bail!("Unexpected response: {:?}", other),
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
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../godot-project")
            .canonicalize()
            .expect("tests/godot-project dir must exist")
    }
}

impl Drop for GodotFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
pub fn find_entity<'a>(
    data: &'a serde_json::Value,
    name: &str,
) -> &'a serde_json::Value {
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
