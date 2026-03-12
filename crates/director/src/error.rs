use crate::daemon::DaemonError;
use crate::editor::EditorError;
use crate::oneshot::OperationError;
use crate::resolve::ResolveError;
use rmcp::model::ErrorData as McpError;

/// Convert ResolveError to McpError for use in tool handlers.
impl From<ResolveError> for McpError {
    fn from(e: ResolveError) -> Self {
        McpError::invalid_params(e.to_string(), None)
    }
}

spectator_protocol::impl_mcp_internal!(DaemonError, EditorError, OperationError);
