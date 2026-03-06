use spectator_protocol::{
    messages::Message,
    query::{GetNodeInspectParams, GetSceneTreeParams, GetSnapshotDataParams},
};

use crate::collector::SpectatorCollector;

/// Dispatch an incoming query to the appropriate handler.
/// Returns the response Message to send back.
pub fn handle_query(
    id: String,
    method: &str,
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Message {
    let result = match method {
        "get_snapshot_data" => handle_get_snapshot_data(params, collector),
        "get_frame_info" => handle_get_frame_info(collector),
        "get_node_inspect" => handle_get_node_inspect(params, collector),
        "get_scene_tree" => handle_get_scene_tree(params, collector),
        _ => Err(QueryError {
            code: "method_not_found".to_string(),
            message: format!("Unknown query method: {method}"),
        }),
    };

    match result {
        Ok(data) => Message::Response { id, data },
        Err(e) => Message::Error {
            id,
            code: e.code,
            message: e.message,
        },
    }
}

struct QueryError {
    code: String,
    message: String,
}

fn handle_get_snapshot_data(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSnapshotDataParams = serde_json::from_value(params).map_err(|e| QueryError {
        code: "invalid_params".to_string(),
        message: format!("Invalid params: {e}"),
    })?;

    let data = collector.collect_snapshot(&params);
    serde_json::to_value(&data).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_frame_info(collector: &SpectatorCollector) -> Result<serde_json::Value, QueryError> {
    let info = collector.get_frame_info();
    serde_json::to_value(&info).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_node_inspect(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetNodeInspectParams = serde_json::from_value(params).map_err(|e| QueryError {
        code: "invalid_params".to_string(),
        message: format!("Invalid params: {e}"),
    })?;

    let data = collector.inspect_node(&params).map_err(|e| QueryError {
        code: "node_not_found".to_string(),
        message: e,
    })?;

    serde_json::to_value(&data).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_scene_tree(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSceneTreeParams = serde_json::from_value(params).map_err(|e| QueryError {
        code: "invalid_params".to_string(),
        message: format!("Invalid params: {e}"),
    })?;

    collector.query_scene_tree(&params).map_err(|e| QueryError {
        code: "node_not_found".to_string(),
        message: e,
    })
}
