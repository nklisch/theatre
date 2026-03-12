use crate::daemon::DaemonError;
use crate::editor::EditorError;
use crate::oneshot::OperationError;
use crate::resolve::ResolveError;
use rmcp::model::ErrorData as McpError;

/// Implement `From<$ty> for McpError` mapping all variants to `internal_error`.
macro_rules! impl_mcp_internal {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for McpError {
                fn from(e: $ty) -> Self {
                    McpError::internal_error(e.to_string(), None)
                }
            }
        )+
    };
}

/// Convert ResolveError to McpError for use in tool handlers.
impl From<ResolveError> for McpError {
    fn from(e: ResolveError) -> Self {
        McpError::invalid_params(e.to_string(), None)
    }
}

impl_mcp_internal!(DaemonError, EditorError, OperationError);
