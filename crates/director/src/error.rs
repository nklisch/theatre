use crate::daemon::DaemonError;
use crate::oneshot::OperationError;
use crate::resolve::ResolveError;
use rmcp::model::ErrorData as McpError;

/// Convert ResolveError to McpError for use in tool handlers.
impl From<ResolveError> for McpError {
    fn from(e: ResolveError) -> Self {
        McpError::invalid_params(e.to_string(), None)
    }
}

/// Convert DaemonError to McpError for use in tool handlers.
impl From<DaemonError> for McpError {
    fn from(e: DaemonError) -> Self {
        McpError::internal_error(e.to_string(), None)
    }
}

/// Convert OperationError to McpError for use in tool handlers.
impl From<OperationError> for McpError {
    fn from(e: OperationError) -> Self {
        match &e {
            OperationError::OperationFailed { .. } => {
                McpError::internal_error(e.to_string(), None)
            }
            OperationError::SpawnFailed(_)
            | OperationError::ProcessFailed { .. }
            | OperationError::Timeout(_)
            | OperationError::ParseFailed { .. } => {
                McpError::internal_error(e.to_string(), None)
            }
        }
    }
}
