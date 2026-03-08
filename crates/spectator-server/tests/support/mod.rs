pub mod fixtures;
pub mod harness;
pub mod mock_addon;

#[cfg(feature = "e2e-tests")]
pub mod godot_process;
#[cfg(feature = "e2e-tests")]
pub mod e2e_harness;

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
        "recording" => {
            let p = from_value(params)?;
            server.recording(Parameters(p)).await
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown tool: {name}"),
            None,
        )),
    }
}
