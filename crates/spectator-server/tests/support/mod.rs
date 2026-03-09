pub mod fixtures;
pub mod harness;
pub mod mock_addon;

pub mod e2e_harness;
pub mod godot_process;

/// Shared tool dispatch: routes tool name + JSON params to SpectatorServer handler methods.
/// Used by both TestHarness and E2EHarness.
pub async fn dispatch_tool(
    server: &spectator_server::server::SpectatorServer,
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, rmcp::model::ErrorData> {
    let raw = dispatch_tool_raw(server, name, params).await?;
    Ok(serde_json::from_str(&raw).unwrap())
}

pub async fn dispatch_tool_raw(
    server: &spectator_server::server::SpectatorServer,
    name: &str,
    params: serde_json::Value,
) -> Result<String, rmcp::model::ErrorData> {
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::ErrorData as McpError;

    fn from_value<T: for<'de> serde::Deserialize<'de>>(
        v: serde_json::Value,
    ) -> Result<T, McpError> {
        serde_json::from_value(v).map_err(|e| McpError::invalid_params(e.to_string(), None))
    }

    match name {
        "spatial_snapshot" => {
            let p = from_value(params)?;
            server.spatial_snapshot(Parameters(p)).await
        }
        "spatial_inspect" => {
            let p = from_value(params)?;
            server.spatial_inspect(Parameters(p)).await
        }
        "scene_tree" => {
            let p = from_value(params)?;
            server.scene_tree(Parameters(p)).await
        }
        "spatial_action" => {
            let p = from_value(params)?;
            server.spatial_action(Parameters(p)).await
        }
        "spatial_query" => {
            let p = from_value(params)?;
            server.spatial_query(Parameters(p)).await
        }
        "spatial_delta" => {
            let p = from_value(params)?;
            server.spatial_delta(Parameters(p)).await
        }
        "spatial_watch" => {
            let p = from_value(params)?;
            server.spatial_watch(Parameters(p)).await
        }
        "spatial_config" => {
            let p = from_value(params)?;
            server.spatial_config(Parameters(p)).await
        }
        "clips" => {
            // clips returns CallToolResult; extract text from first content block
            let p = from_value(params)?;
            let result = server.clips(Parameters(p)).await?;
            let text = result
                .content
                .first()
                .and_then(|c| c.as_text())
                .map(|t| t.text.clone())
                .unwrap_or_default();
            Ok(text)
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown tool: {name}"),
            None,
        )),
    }
}

/// Dispatch a tool call and return the full CallToolResult.
/// For clips (which returns native CallToolResult), calls directly.
/// For all other tools, wraps the string result in a single-text-block result.
pub async fn dispatch_tool_result(
    server: &spectator_server::server::SpectatorServer,
    name: &str,
    params: serde_json::Value,
) -> Result<rmcp::model::CallToolResult, rmcp::model::ErrorData> {
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::{CallToolResult, Content, ErrorData as McpError};

    fn from_value<T: for<'de> serde::Deserialize<'de>>(
        v: serde_json::Value,
    ) -> Result<T, McpError> {
        serde_json::from_value(v).map_err(|e| McpError::invalid_params(e.to_string(), None))
    }

    if name == "clips" {
        let p = from_value(params)?;
        return server.clips(Parameters(p)).await;
    }

    // For all other tools, call them directly and wrap in CallToolResult
    let s = match name {
        "spatial_snapshot" => {
            let p = from_value(params)?;
            server.spatial_snapshot(Parameters(p)).await?
        }
        "spatial_inspect" => {
            let p = from_value(params)?;
            server.spatial_inspect(Parameters(p)).await?
        }
        "scene_tree" => {
            let p = from_value(params)?;
            server.scene_tree(Parameters(p)).await?
        }
        "spatial_action" => {
            let p = from_value(params)?;
            server.spatial_action(Parameters(p)).await?
        }
        "spatial_query" => {
            let p = from_value(params)?;
            server.spatial_query(Parameters(p)).await?
        }
        "spatial_delta" => {
            let p = from_value(params)?;
            server.spatial_delta(Parameters(p)).await?
        }
        "spatial_watch" => {
            let p = from_value(params)?;
            server.spatial_watch(Parameters(p)).await?
        }
        "spatial_config" => {
            let p = from_value(params)?;
            server.spatial_config(Parameters(p)).await?
        }
        _ => {
            return Err(McpError::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::success(vec![Content::text(s)]))
}
