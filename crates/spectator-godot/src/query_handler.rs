use spectator_protocol::{
    messages::Message,
    query::{
        ActionRequest, GetNodeInspectParams, GetSceneTreeParams, GetSnapshotDataParams,
        QueryOrigin, ResolveNodeResponse, SpatialQueryRequest,
    },
};

use crate::action_handler::{self, ActionResult};
use crate::collector::SpectatorCollector;

/// Dispatch an incoming query to the appropriate handler.
/// Returns Some(response) for immediate replies, or None for deferred responses
/// (e.g., advance_frames, which are sent after physics ticks complete).
pub fn handle_query(
    request_id: String,
    method: &str,
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Option<Message> {
    match method {
        "get_snapshot_data" => Some(simple_query(
            request_id,
            handle_get_snapshot_data(params, collector),
        )),
        "get_frame_info" => Some(simple_query(request_id, handle_get_frame_info(collector))),
        "get_node_inspect" => Some(simple_query(
            request_id,
            handle_get_node_inspect(params, collector),
        )),
        "get_scene_tree" => Some(simple_query(
            request_id,
            handle_get_scene_tree(params, collector),
        )),
        "execute_action" => handle_execute_action(request_id, params, collector),
        "spatial_query" => Some(simple_query(
            request_id,
            handle_spatial_query(params, collector),
        )),
        _ => Some(Message::Error {
            request_id,
            code: "method_not_found".to_string(),
            message: format!("Unknown query method: {method}"),
        }),
    }
}

struct QueryError {
    code: String,
    message: String,
}

fn simple_query(request_id: String, result: Result<serde_json::Value, QueryError>) -> Message {
    match result {
        Ok(data) => Message::Response { request_id, data },
        Err(e) => Message::Error {
            request_id,
            code: e.code,
            message: e.message,
        },
    }
}

fn parse_params<T: for<'de> serde::Deserialize<'de>>(
    value: serde_json::Value,
) -> Result<T, QueryError> {
    serde_json::from_value(value).map_err(|e| QueryError {
        code: "invalid_params".to_string(),
        message: format!("Invalid params: {e}"),
    })
}

fn to_json_value<T: serde::Serialize>(data: &T) -> Result<serde_json::Value, QueryError> {
    serde_json::to_value(data).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_snapshot_data(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSnapshotDataParams = parse_params(params)?;
    to_json_value(&collector.collect_snapshot(&params))
}

fn handle_get_frame_info(collector: &SpectatorCollector) -> Result<serde_json::Value, QueryError> {
    to_json_value(&collector.get_frame_info())
}

fn handle_get_node_inspect(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetNodeInspectParams = parse_params(params)?;
    let data = collector.inspect_node(&params).map_err(|e| QueryError {
        code: "node_not_found".to_string(),
        message: e,
    })?;
    to_json_value(&data)
}

fn handle_get_scene_tree(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSceneTreeParams = parse_params(params)?;
    collector.query_scene_tree(&params).map_err(|e| QueryError {
        code: "node_not_found".to_string(),
        message: e,
    })
}

fn handle_execute_action(
    request_id: String,
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Option<Message> {
    let request: ActionRequest = match parse_params(params) {
        Ok(r) => r,
        Err(e) => {
            return Some(Message::Error {
                request_id,
                code: e.code,
                message: e.message,
            });
        }
    };

    match action_handler::execute_action(&request, collector, &request_id) {
        Ok(ActionResult::Done(response)) => match to_json_value(&response) {
            Ok(data) => Some(Message::Response { request_id, data }),
            Err(e) => Some(Message::Error {
                request_id,
                code: e.code,
                message: e.message,
            }),
        },
        Ok(ActionResult::Pending) => {
            // advance_frames in progress — response will be sent by tcp_server.poll()
            None
        }
        Err(e) => Some(Message::Error {
            request_id,
            code: "action_failed".to_string(),
            message: e,
        }),
    }
}

fn handle_spatial_query(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let request: SpatialQueryRequest = parse_params(params)?;
    match request {
        SpatialQueryRequest::Raycast {
            from,
            to,
            collision_mask,
        } => {
            let from_pos = resolve_query_origin(&from, collector)?;
            let to_pos = resolve_query_origin(&to, collector)?;
            // Use 2D raycast when both positions are 2D (2 components)
            if from_pos.len() <= 2 && to_pos.len() <= 2 {
                let from_v2 = godot::builtin::Vector2::new(
                    from_pos[0] as f32,
                    *from_pos.get(1).unwrap_or(&0.0) as f32,
                );
                let to_v2 = godot::builtin::Vector2::new(
                    to_pos[0] as f32,
                    *to_pos.get(1).unwrap_or(&0.0) as f32,
                );
                let result = collector
                    .raycast_2d(from_v2, to_v2, collision_mask)
                    .map_err(|e| QueryError {
                        code: "query_failed".into(),
                        message: e,
                    })?;
                to_json_value(&result)
            } else {
                let from_v3 = godot::builtin::Vector3::new(
                    from_pos[0] as f32,
                    *from_pos.get(1).unwrap_or(&0.0) as f32,
                    *from_pos.get(2).unwrap_or(&0.0) as f32,
                );
                let to_v3 = godot::builtin::Vector3::new(
                    to_pos[0] as f32,
                    *to_pos.get(1).unwrap_or(&0.0) as f32,
                    *to_pos.get(2).unwrap_or(&0.0) as f32,
                );
                let result = collector
                    .raycast(from_v3, to_v3, collision_mask)
                    .map_err(|e| QueryError {
                        code: "query_failed".into(),
                        message: e,
                    })?;
                to_json_value(&result)
            }
        }
        SpatialQueryRequest::PathDistance { from, to } => {
            let from_pos = resolve_query_origin(&from, collector)?;
            let to_pos = resolve_query_origin(&to, collector)?;
            let from_v3 = godot::builtin::Vector3::new(
                from_pos[0] as f32,
                from_pos[1] as f32,
                from_pos[2] as f32,
            );
            let to_v3 =
                godot::builtin::Vector3::new(to_pos[0] as f32, to_pos[1] as f32, to_pos[2] as f32);
            let result = collector
                .get_nav_path(from_v3, to_v3)
                .map_err(|e| QueryError {
                    code: "query_failed".into(),
                    message: e,
                })?;
            to_json_value(&result)
        }
        SpatialQueryRequest::ResolveNode { path } => {
            let result = collector
                .resolve_node_position(&path)
                .map_err(|e| QueryError {
                    code: "node_not_found".into(),
                    message: e,
                })?;
            to_json_value(&result)
        }
    }
}

/// Resolve a QueryOrigin to a position array.
fn resolve_query_origin(
    origin: &QueryOrigin,
    collector: &SpectatorCollector,
) -> Result<Vec<f64>, QueryError> {
    match origin {
        QueryOrigin::Position(pos) => Ok(pos.clone()),
        QueryOrigin::Node(path) => {
            let resolved: ResolveNodeResponse =
                collector
                    .resolve_node_position(path)
                    .map_err(|e| QueryError {
                        code: "node_not_found".into(),
                        message: e,
                    })?;
            Ok(resolved.position)
        }
    }
}
