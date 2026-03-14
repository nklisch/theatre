#![allow(dead_code)]
/// Manages a headless Godot process for E2E tests.
use std::fs::File;
use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::net::TcpStream;
use tokio::time::{Duration, sleep};

pub struct GodotProcess {
    child: Child,
    port: u16,
    stderr_log: PathBuf,
}

impl GodotProcess {
    /// Launch Godot headless with the test project and a specific scene.
    ///
    /// Binds to an ephemeral port (OS-assigned via port 0 trick).
    /// Waits up to E2E_TIMEOUT_SECS seconds (default 15) for the TCP listener.
    /// Captures stderr to a temp file for debugging on failure.
    pub async fn start(scene: &str) -> anyhow::Result<Self> {
        // Ephemeral port allocation: bind to :0, get the assigned port, close.
        // Small TOCTOU window is acceptable in test environments.
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            listener.local_addr()?.port()
        };

        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".to_string());

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_dir = manifest_dir
            .join("..") // crates/
            .join("..") // repo root
            .join("tests")
            .join("godot-project")
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Cannot find godot-project dir: {e}"))?;

        let stderr_log = std::env::temp_dir().join(format!("stage_godot_{port}.stderr"));

        let stderr_file = File::create(&stderr_log)?;

        let child = Command::new(&godot_bin)
            .args(["--headless", "--fixed-fps", "60", "--path"])
            .arg(&project_dir)
            .arg(scene)
            .env("THEATRE_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(stderr_file)
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to spawn Godot ({godot_bin}): {e}\n\
                     Set GODOT_BIN env var to the path of your Godot binary."
                )
            })?;

        let timeout_secs: u64 = std::env::var("E2E_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15);

        let process = Self {
            child,
            port,
            stderr_log,
        };

        // Wait for addon TCP listener to become connectable.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            match TcpStream::connect(format!("127.0.0.1:{port}")).await {
                Ok(_) => break,
                Err(_) => {
                    if tokio::time::Instant::now() >= deadline {
                        let stderr = process.stderr_output();
                        anyhow::bail!(
                            "Timed out after {timeout_secs}s waiting for Godot TCP listener \
                             on port {port}.\n\nGodot stderr:\n{stderr}"
                        );
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Ok(process)
    }

    /// Launch with the 3D test scene.
    pub async fn start_3d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_3d.tscn").await
    }

    /// Launch with the 2D test scene.
    pub async fn start_2d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_2d.tscn").await
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Read captured stderr output (Godot debug output).
    pub fn stderr_output(&self) -> String {
        let mut buf = String::new();
        if let Ok(mut f) = File::open(&self.stderr_log) {
            let _ = f.read_to_string(&mut buf);
        }
        buf
    }
}

impl Drop for GodotProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
