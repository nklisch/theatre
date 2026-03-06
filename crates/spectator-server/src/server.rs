use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tcp::SessionState;

#[derive(Clone)]
pub struct SpectatorServer {
    pub state: Arc<Mutex<SessionState>>,
    pub tool_router: ToolRouter<Self>,
}

impl SpectatorServer {
    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for SpectatorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "spectator-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
