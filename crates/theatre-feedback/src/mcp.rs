//! MCP rendering shared by the two servers; typed results and image blocks stay intact.
use crate::{Operation, Queue, Response};
use base64::Engine;
use rmcp::model::{CallToolResult, Content, ErrorData};
use std::path::Path;

pub fn execute(project: &Path, operation: Operation) -> Result<CallToolResult, ErrorData> {
    let response = Queue::open(project)
        .and_then(|queue| queue.execute(operation))
        .map_err(|e| match e {
            crate::Error::Invalid(_) => ErrorData::invalid_params(e.to_string(), None),
            _ => ErrorData::internal_error(e.to_string(), None),
        })?;
    let value = serde_json::to_value(&response)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    let mut content = vec![Content::text(value.to_string())];
    if let Response::Retrieve {
        image_path: Some(path),
        ..
    } = &response
    {
        let bytes = crate::read_bounded(path, crate::MAX_IMAGE_BYTES)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        content.push(Content::image(
            base64::engine::general_purpose::STANDARD.encode(bytes),
            "image/jpeg",
        ));
    }
    let mut result = CallToolResult::success(content);
    result.structured_content = Some(value);
    Ok(result)
}

pub fn append_notice(result: &mut Result<CallToolResult, ErrorData>, project: &Path) {
    let Some(notice) = crate::pending_notice(project) else {
        return;
    };
    match result {
        Ok(result) => result.content.push(Content::text(notice)),
        Err(error) => {
            // Preserve error code, message and existing data, including non-object data.
            if error.data.is_none() {
                error.data = Some(serde_json::json!({"feedback_notice": notice}));
            } else if let Some(serde_json::Value::Object(data)) = error.data.as_mut() {
                data.insert("feedback_notice".into(), notice.into());
            } else {
                error.message = format!("{}\n{}", error.message, notice).into();
            }
        }
    }
}
