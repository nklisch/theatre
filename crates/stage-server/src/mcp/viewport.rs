use std::sync::Arc;

use rmcp::model::{CallToolResult, Content, ErrorData as McpError};
use stage_protocol::mcp_helpers::{deserialize_response, serialize_params, serialize_response};
use stage_protocol::viewport::ViewportCapture;
use tokio::sync::Mutex;

use crate::tcp::{SessionState, query_addon};
pub use stage_protocol::viewport::{ViewportMetadata, ViewportParams};

pub async fn handle_viewport(
    params: ViewportParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<CallToolResult, McpError> {
    params
        .validate()
        .map_err(|message| McpError::invalid_params(message, None))?;
    let capture: ViewportCapture = deserialize_response(
        query_addon(state, "get_viewport", serialize_params(&params)?).await?,
    )?;
    let metadata = serialize_params(&capture.metadata)?;
    let mut content = vec![Content::text(serialize_response(&capture.metadata)?)];
    if let Some(image) = capture.image_base64 {
        content.push(Content::image(image, "image/jpeg"));
    }
    let mut result = CallToolResult::success(content);
    result.structured_content = Some(metadata);
    Ok(result)
}

/// Same mixed-image JSON convention as clips: metadata plus image_base64/mime_type.
pub async fn handle_viewport_cli(
    params: ViewportParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let result = handle_viewport(params, state).await?;
    let mut metadata = result
        .structured_content
        .ok_or_else(|| McpError::internal_error("Missing viewport metadata", None))?;
    for content in result.content {
        if let rmcp::model::RawContent::Image(image) = content.raw {
            metadata["image_base64"] = image.data.into();
            metadata["mime_type"] = image.mime_type.into();
        }
    }
    serialize_response(&metadata)
}
