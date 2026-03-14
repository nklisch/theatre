use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;
use schemars::JsonSchema;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::responses::{
    ConfigResponse, DeltaResponse, SnapshotSummaryResponse, WatchAddResponse,
};
use crate::tcp::SessionState;

fn attach_output_schema<T: JsonSchema + 'static>(
    router: &mut ToolRouter<StageServer>,
    tool_name: &str,
) {
    if let Some(route) = router.map.get_mut(tool_name) {
        route.attr = route.attr.clone().with_output_schema::<T>();
    }
}

#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,
    pub tool_router: ToolRouter<Self>,
}

impl StageServer {
    /// Build the tool router with output schemas attached.
    /// Used by both `new()` and `docs_router()`.
    pub fn router_with_schemas() -> ToolRouter<Self> {
        let mut router = Self::tool_router();
        attach_output_schema::<SnapshotSummaryResponse>(&mut router, "spatial_snapshot");
        attach_output_schema::<DeltaResponse>(&mut router, "spatial_delta");
        attach_output_schema::<WatchAddResponse>(&mut router, "spatial_watch");
        attach_output_schema::<ConfigResponse>(&mut router, "spatial_config");
        router
    }

    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self {
            state,
            tool_router: Self::router_with_schemas(),
        }
    }

    /// Push an activity log event to the addon (best-effort, non-blocking).
    pub(crate) async fn log_activity(&self, entry_type: &str, summary: &str, tool: &str) {
        self.log_activity_with_meta(entry_type, summary, tool, None)
            .await;
    }

    /// Push an activity log event with optional metadata (best-effort, non-blocking).
    /// Use `meta` to include structured data — e.g. `{ "active_watches": N }` for watch events.
    pub(crate) async fn log_activity_with_meta(
        &self,
        entry_type: &str,
        summary: &str,
        tool: &str,
        meta: Option<serde_json::Value>,
    ) {
        let event = crate::activity::build_activity_message(entry_type, summary, tool, meta);
        let mut s = self.state.lock().await;
        if let Some(ref mut writer) = s.tcp_writer {
            let _ =
                stage_protocol::codec::async_io::write_message(&mut writer.writer, &event).await;
        }
    }
}

#[tool_handler]
impl ServerHandler for StageServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "stage-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
