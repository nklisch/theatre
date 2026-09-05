use crate::tcp::SessionState;
use rmcp::model::{CallToolResult, ErrorData};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle(
    operation: theatre_feedback::Operation,
    state: &Arc<Mutex<SessionState>>,
) -> Result<CallToolResult, ErrorData> {
    let project = state.lock().await.project_dir.clone();
    theatre_feedback::mcp::execute(&project, operation)
}
