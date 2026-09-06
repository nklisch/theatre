use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use schemars::JsonSchema;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::responses::{ConfigResponse, DeltaResponse, SnapshotResponse, WatchResponse};
use crate::tcp::SessionState;

fn attach_output_schema<T: JsonSchema + 'static>(
    router: &mut ToolRouter<StageServer>,
    tool_name: &str,
) {
    if let Some(route) = router.map.get_mut(tool_name) {
        route.attr = route.attr.clone().with_output_schema::<T>();
    }
}

/// Normalize input and output schemas for strict JSON Schema MCP clients.
fn sanitize_schemas(router: &mut ToolRouter<StageServer>) {
    for route in router.map.values_mut() {
        let mut input = serde_json::Value::Object(route.attr.input_schema.as_ref().clone());
        stage_protocol::mcp_helpers::normalize_mcp_schema(&mut input);
        if let serde_json::Value::Object(map) = input {
            route.attr.input_schema = std::sync::Arc::new(map);
        }
        if let Some(ref schema) = route.attr.output_schema {
            let mut value = serde_json::Value::Object(schema.as_ref().clone());
            stage_protocol::mcp_helpers::normalize_mcp_schema(&mut value);
            if let serde_json::Value::Object(map) = value {
                route.attr.output_schema = Some(Arc::new(map));
            }
        }
    }
}

#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,
    pub tool_router: ToolRouter<Self>,
    pub(crate) connection: Arc<crate::project::ProjectConnection>,
}

impl StageServer {
    /// Build the tool router with output schemas attached.
    /// Used by both `new()` and `docs_router()`.
    pub fn router_with_schemas() -> ToolRouter<Self> {
        let mut router = Self::tool_router();
        attach_output_schema::<crate::mcp::project_select::ProjectSelectResponse>(
            &mut router,
            "project_select",
        );
        attach_output_schema::<theatre_feedback::Response>(&mut router, "feedback");
        attach_output_schema::<SnapshotResponse>(&mut router, "spatial_snapshot");
        attach_output_schema::<DeltaResponse>(&mut router, "spatial_delta");
        attach_output_schema::<WatchResponse>(&mut router, "spatial_watch");
        attach_output_schema::<ConfigResponse>(&mut router, "spatial_config");
        attach_output_schema::<crate::mcp::runtime_status::RuntimeStatusResponse>(
            &mut router,
            "runtime_status",
        );
        attach_output_schema::<crate::mcp::runtime_diagnostics::RuntimeDiagnosticsResponse>(
            &mut router,
            "runtime_diagnostics",
        );
        attach_output_schema::<crate::mcp::viewport::ViewportMetadata>(&mut router, "viewport");
        sanitize_schemas(&mut router);
        router
    }

    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self {
            state,
            tool_router: Self::router_with_schemas(),
            connection: Arc::new(crate::project::ProjectConnection::default()),
        }
    }

    /// Start the owned reconnect task. Selection later replaces this same task.
    pub async fn start_connection(&self, port: u16) -> Result<(), tokio::task::JoinError> {
        let exclusive = self.connection.operations.clone().write_owned().await;
        let _exclusive = self
            .connection
            .connect(&self.state, port, None, exclusive)
            .await?;
        Ok(())
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

impl ServerHandler for StageServer {
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        // Keep the entire operation, including local post-processing and notices,
        // on one target. project_select takes the exclusive lock itself.
        let _operation = if request.name == "project_select" {
            None
        } else {
            Some(self.connection.operations.read().await)
        };
        let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        let mut result = self.tool_router.call(call).await;
        // Selection's response already includes its new project's notice while
        // holding the exclusive lock; never attach the old queue to that result.
        if _operation.is_some() {
            let project = self.state.lock().await.project_dir.clone();
            theatre_feedback::mcp::append_notice(&mut result, &project);
        }
        result
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
        })
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        self.tool_router.get(name).cloned()
    }

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
