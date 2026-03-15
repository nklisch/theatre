#![allow(dead_code)]
use serde_json::Value;

/// Result of a tool invocation.
pub enum ToolResult {
    Ok(Value),
    Err { code: String, message: String },
}

impl ToolResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, ToolResult::Ok(_))
    }

    pub fn unwrap_data(self) -> Value {
        match self {
            ToolResult::Ok(v) => v,
            ToolResult::Err { code, message } => {
                panic!("Expected Ok but got Err({code}): {message}")
            }
        }
    }

    pub fn unwrap_err(self) -> (String, String) {
        match self {
            ToolResult::Err { code, message } => (code, message),
            ToolResult::Ok(v) => panic!("Expected Err but got Ok: {v}"),
        }
    }
}

/// Abstraction over CLI subprocess vs in-process MCP server.
pub trait LiveBackend: Send + Sync {
    /// Invoke a Stage tool.
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult>;

    /// Invoke a Director operation.
    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult>;

    /// Wait for N physics frames at 60 FPS.
    async fn wait_frames(&self, n: u32);

    /// Whether this backend maintains a persistent session.
    fn is_stateful(&self) -> bool;
}
