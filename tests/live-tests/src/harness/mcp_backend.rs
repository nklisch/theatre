#![allow(dead_code)]
use super::backend::{LiveBackend, ToolResult};
use super::dispatch::dispatch_tool;
use super::godot_process::LiveGodotProcess;
use serde_json::Value;
use stage_server::server::StageServer;
use stage_server::tcp::{SessionState, tcp_client_loop};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;

pub struct McpBackend {
    pub godot: LiveGodotProcess,
    pub server: StageServer,
    pub state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,
    project_dir: PathBuf,
}

impl McpBackend {
    pub async fn start(scene: &str) -> anyhow::Result<Self> {
        let godot = LiveGodotProcess::start(scene).await?;
        let port = godot.port();

        let state = Arc::new(Mutex::new(SessionState::default()));
        let tcp_state = state.clone();

        let tcp_task = tokio::spawn(async move {
            tcp_client_loop(tcp_state, port).await;
        });

        // Wait for connection
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            if state.lock().await.connected {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("Timed out waiting for Stage TCP connection");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let server = StageServer::new(state.clone());
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_dir = manifest_dir
            .join("../../tests/godot-project")
            .canonicalize()?;

        Ok(Self {
            godot,
            server,
            state,
            _tcp_task: tcp_task,
            project_dir,
        })
    }
}

impl LiveBackend for McpBackend {
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult> {
        match dispatch_tool(&self.server, tool, params).await {
            Ok(v) => Ok(ToolResult::Ok(v)),
            Err(e) => Ok(ToolResult::Err {
                code: format!("{:?}", e.code),
                message: e.message.to_string(),
            }),
        }
    }

    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult> {
        let bin =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/director");

        let mut params = params;
        if let Value::Object(ref mut map) = params {
            map.entry("project_path").or_insert_with(|| {
                Value::String(self.project_dir.to_string_lossy().into())
            });
        }

        let output = Command::new(&bin)
            .args([operation, &params.to_string()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to spawn director binary: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "director CLI returned empty stdout (exit {exit_code})\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let json: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            anyhow::anyhow!("director output is not valid JSON: {e}\nstdout: {stdout}")
        })?;

        if exit_code == 0 {
            Ok(ToolResult::Ok(json))
        } else {
            Ok(ToolResult::Err {
                code: "director_error".to_string(),
                message: json.to_string(),
            })
        }
    }

    async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn is_stateful(&self) -> bool {
        true
    }
}

impl Drop for McpBackend {
    fn drop(&mut self) {
        self._tcp_task.abort();
        // LiveGodotProcess::drop kills godot
    }
}
