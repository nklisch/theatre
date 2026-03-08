/// Test harness: connects a real SpectatorServer to a MockAddon.
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use spectator_server::{
    server::SpectatorServer,
    tcp::{SessionState, tcp_client_loop},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

use super::mock_addon::{MockAddon, QueryHandler};

pub struct TestHarness {
    pub server: SpectatorServer,
    pub mock: MockAddon,
    pub state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,
}

impl TestHarness {
    /// Create a harness with a standard 3D handshake.
    pub async fn new(handler: QueryHandler) -> Self {
        let mock = MockAddon::start(handler).await;
        Self::connect(mock).await
    }

    /// Create a harness with a 2D handshake.
    pub async fn new_2d(handler: QueryHandler) -> Self {
        let mock = MockAddon::start_2d(handler).await;
        Self::connect(mock).await
    }

    pub async fn new_with_mock(mock: MockAddon) -> Self {
        Self::connect(mock).await
    }

    async fn connect(mock: MockAddon) -> Self {
        let state = Arc::new(Mutex::new(SessionState::default()));
        let tcp_state = state.clone();
        let port = mock.port();

        let tcp_task = tokio::spawn(async move {
            tcp_client_loop(tcp_state, port).await;
        });

        wait_for_connected(&state).await;

        let server = SpectatorServer::new(state.clone());
        Self {
            server,
            mock,
            state,
            _tcp_task: tcp_task,
        }
    }

    /// Call a tool by name with JSON params, return parsed JSON.
    pub async fn call_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let s = self.call_tool_raw(name, params).await?;
        Ok(serde_json::from_str(&s).unwrap())
    }

    /// Call a tool and return the raw JSON string.
    pub async fn call_tool_raw(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<String, McpError> {
        match name {
            "spatial_snapshot" => {
                let p = from_value(params)?;
                self.server.spatial_snapshot(Parameters(p)).await
            }
            "spatial_inspect" => {
                let p = from_value(params)?;
                self.server.spatial_inspect(Parameters(p)).await
            }
            "scene_tree" => {
                let p = from_value(params)?;
                self.server.scene_tree(Parameters(p)).await
            }
            "spatial_action" => {
                let p = from_value(params)?;
                self.server.spatial_action(Parameters(p)).await
            }
            "spatial_query" => {
                let p = from_value(params)?;
                self.server.spatial_query(Parameters(p)).await
            }
            "spatial_delta" => {
                let p = from_value(params)?;
                self.server.spatial_delta(Parameters(p)).await
            }
            "spatial_watch" => {
                let p = from_value(params)?;
                self.server.spatial_watch(Parameters(p)).await
            }
            "spatial_config" => {
                let p = from_value(params)?;
                self.server.spatial_config(Parameters(p)).await
            }
            "recording" => {
                let p = from_value(params)?;
                self.server.recording(Parameters(p)).await
            }
            _ => Err(McpError::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            )),
        }
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        self._tcp_task.abort();
    }
}

fn from_value<T: for<'de> serde::Deserialize<'de>>(v: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(v).map_err(|e| McpError::invalid_params(e.to_string(), None))
}

async fn wait_for_connected(state: &Arc<Mutex<SessionState>>) {
    for _ in 0..100 {
        if state.lock().await.connected {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("Timed out waiting for TCP connection to mock addon");
}
