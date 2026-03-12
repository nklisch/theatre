use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;

use crate::backend::Backend;

#[derive(Clone)]
pub struct DirectorServer {
    pub tool_router: ToolRouter<Self>,
    pub backend: Arc<Backend>,
}

impl DirectorServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            backend: Arc::new(Backend::new()),
        }
    }
}

impl Default for DirectorServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler]
impl ServerHandler for DirectorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "director".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
