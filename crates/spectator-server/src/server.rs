use rmcp::handler::server::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tcp::SessionState;

#[derive(Clone)]
pub struct SpectatorServer {
    // Used in M1+ by MCP tool handlers to query the addon.
    #[allow(dead_code)]
    pub state: Arc<Mutex<SessionState>>,
}

impl SpectatorServer {
    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self { state }
    }
}

// No tools in M0. ServerHandler defaults: list_tools returns empty, call_tool returns error.
// Tools are added in M1+ using #[tool_router] / #[tool_handler].
impl ServerHandler for SpectatorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "spectator-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}
