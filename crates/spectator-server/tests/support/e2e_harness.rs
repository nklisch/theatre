/// Full-stack E2E harness: real Godot + real SpectatorServer.
use rmcp::model::ErrorData as McpError;
use spectator_server::{
    server::SpectatorServer,
    tcp::{SessionState, tcp_client_loop},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;

use super::godot_process::GodotProcess;
use super::harness::wait_for_connected;

pub struct E2EHarness {
    pub godot: GodotProcess,
    pub server: SpectatorServer,
    pub state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,
    trace: Vec<StepTrace>,
    scene: String,
}

struct StepTrace {
    step: usize,
    tool: String,
    params: serde_json::Value,
    result: Result<serde_json::Value, String>,
    elapsed_ms: u64,
}

impl E2EHarness {
    /// Launch Godot with the 3D test scene and connect.
    pub async fn start_3d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_3d.tscn").await
    }

    /// Launch Godot with the 2D test scene and connect.
    pub async fn start_2d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_2d.tscn").await
    }

    /// Launch Godot with a specific scene, create server, connect, handshake.
    pub async fn start(scene: &str) -> anyhow::Result<Self> {
        let godot = GodotProcess::start(scene).await?;
        let port = godot.port();

        let state = Arc::new(Mutex::new(SessionState::default()));
        let tcp_state = state.clone();

        let tcp_task = tokio::spawn(async move {
            tcp_client_loop(tcp_state, port).await;
        });

        wait_for_connected(&state).await;

        let server = SpectatorServer::new(state.clone());
        Ok(Self {
            godot,
            server,
            state,
            _tcp_task: tcp_task,
            trace: Vec::new(),
            scene: scene.to_string(),
        })
    }

    /// Call a tool, logging the step for trace output. Returns the parsed JSON result.
    pub async fn step(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let start = tokio::time::Instant::now();
        let result = super::dispatch_tool(&self.server, tool, params.clone()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        self.trace.push(StepTrace {
            step: n,
            tool: tool.to_string(),
            params,
            result: match &result {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(format!("{:?}: {}", e.code, e.message)),
            },
            elapsed_ms,
        });

        result
    }

    /// Call a tool expecting success. Panics with full journey trace on failure.
    pub async fn expect(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match self.step(n, tool, params).await {
            Ok(v) => v,
            Err(e) => {
                let trace = self.trace_dump();
                panic!(
                    "Step {n} ({tool}): expected success but got error: {:?} — {}\n\n{trace}",
                    e.code, e.message
                );
            }
        }
    }

    /// Call a tool expecting failure. Panics with trace if it succeeds.
    pub async fn expect_err(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> McpError {
        match self.step(n, tool, params).await {
            Err(e) => e,
            Ok(v) => {
                let trace = self.trace_dump();
                panic!("Step {n} ({tool}): expected error but got success: {v}\n\n{trace}");
            }
        }
    }

    /// Wait for N physics frames to elapse.
    /// At --fixed-fps 60, each frame is ~16.7ms. 50ms margin for scheduling jitter.
    pub async fn wait_frames(&mut self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        self.trace.push(StepTrace {
            step: 0,
            tool: format!("[wait {n} frames, {ms}ms]"),
            params: serde_json::Value::Null,
            result: Ok(serde_json::Value::Null),
            elapsed_ms: ms,
        });
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    /// Format the full trace for debugging. On failure, also includes Godot's stderr.
    pub fn trace_dump(&self) -> String {
        let mut out = format!(
            "E2E Journey Trace ({}, port {}):\n",
            self.scene,
            self.godot.port()
        );
        for t in &self.trace {
            if t.tool.starts_with('[') {
                // wait entry
                out.push_str(&format!("  {}\n", t.tool));
            } else {
                let status = match &t.result {
                    Ok(_) => "OK".to_string(),
                    Err(e) => format!("ERR  ← FAILED\n    Error: {e}"),
                };
                out.push_str(&format!(
                    "  Step {}: {}({}) → {} ({}ms)\n",
                    t.step,
                    t.tool,
                    serde_json::to_string(&t.params).unwrap_or_default(),
                    status,
                    t.elapsed_ms,
                ));
            }
        }

        let stderr = self.godot.stderr_output();
        if !stderr.is_empty() {
            let last_lines: Vec<&str> = stderr.lines().rev().take(20).collect();
            out.push_str("\nGodot stderr (last 20 lines):\n");
            for line in last_lines.into_iter().rev() {
                out.push_str(&format!("  {line}\n"));
            }
        }

        out
    }
}

impl Drop for E2EHarness {
    fn drop(&mut self) {
        self._tcp_task.abort();
        // GodotProcess::drop kills godot
    }
}
