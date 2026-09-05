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

stage_protocol::impl_mcp_internal!(DaemonError, EditorError);

impl From<OperationError> for McpError {
    fn from(error: OperationError) -> Self {
        let message = error.to_string();
        let data = match error {
            OperationError::OperationFailed(result) => Some(serde_json::json!({
                "operation": result.operation,
                "context": result.context,
                "data": result.data,
                "persistence": result.persistence,
            })),
            _ => None,
        };
        McpError::internal_error(message, data)
    }
}
