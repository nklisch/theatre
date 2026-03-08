use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::watch::{ConditionOperator, TrackCategory, WatchCondition};

use crate::tcp::get_config;

use super::finalize_response;

/// MCP parameters for the spatial_watch tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialWatchParams {
    /// Action: "add", "remove", "list", "clear".
    #[schemars(description = "Action: add, remove, list, clear")]
    pub action: String,

    /// For "add": watch specification.
    pub watch: Option<WatchSpec>,

    /// For "remove": watch ID to remove.
    pub watch_id: Option<String>,
}

/// Watch specification for the "add" action.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchSpec {
    /// Node path or "group:<name>".
    pub node: String,

    /// Conditions for triggering.
    #[serde(default)]
    pub conditions: Vec<WatchConditionInput>,

    /// What to track: position, state, signals, physics, all.
    #[serde(default = "default_track")]
    pub track: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchConditionInput {
    pub property: String,
    /// Operator: "lt", "gt", "eq", "changed".
    pub operator: String,
    pub value: Option<serde_json::Value>,
}

fn default_track() -> Vec<String> {
    vec!["all".to_string()]
}

fn parse_operator(s: &str) -> Result<ConditionOperator, McpError> {
    super::parse_enum_param(s, "operator", &[
        ("lt", ConditionOperator::Lt),
        ("gt", ConditionOperator::Gt),
        ("eq", ConditionOperator::Eq),
        ("changed", ConditionOperator::Changed),
    ])
}

fn parse_track(s: &str) -> Result<TrackCategory, McpError> {
    super::parse_enum_param(s, "track category", &[
        ("position", TrackCategory::Position),
        ("state", TrackCategory::State),
        ("signals", TrackCategory::Signals),
        ("physics", TrackCategory::Physics),
        ("all", TrackCategory::All),
    ])
}

fn format_conditions(conditions: &[WatchCondition]) -> String {
    if conditions.is_empty() {
        "none".to_string()
    } else {
        conditions
            .iter()
            .map(|c| {
                let val = c.value.as_ref().map(|v| v.to_string()).unwrap_or_default();
                format!("{} {:?} {}", c.property, c.operator, val)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub async fn handle_spatial_watch(
    params: SpatialWatchParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    let hard_cap = get_config(state).await.token_hard_cap;

    match params.action.as_str() {
        "add" => {
            let spec = params.watch.ok_or_else(|| {
                McpError::invalid_params("'watch' specification is required for add action", None)
            })?;

            let conditions: Vec<WatchCondition> = spec
                .conditions
                .iter()
                .map(|c| {
                    Ok(WatchCondition {
                        property: c.property.clone(),
                        operator: parse_operator(&c.operator)?,
                        value: c.value.clone(),
                    })
                })
                .collect::<Result<Vec<_>, McpError>>()?;

            let track: Vec<TrackCategory> = spec
                .track
                .iter()
                .map(|t| parse_track(t))
                .collect::<Result<Vec<_>, McpError>>()?;

            let watch = {
                let mut s = state.lock().await;
                s.watch_engine.add(spec.node, conditions, track)
            };

            let mut response = serde_json::json!({
                "watch_id": watch.id,
                "watching": watch.node,
                "conditions": format_conditions(&watch.conditions),
                "tracking": watch.track,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        "remove" => {
            let watch_id = params.watch_id.ok_or_else(|| {
                McpError::invalid_params("'watch_id' is required for remove action", None)
            })?;

            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.remove(&watch_id)
            };

            let mut response = serde_json::json!({
                "result": if removed { "ok" } else { "not_found" },
                "removed": if removed { 1 } else { 0 },
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        "list" => {
            let watches = {
                let s = state.lock().await;
                s.watch_engine
                    .list()
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "watch_id": w.id,
                            "node": w.node,
                            "conditions": format_conditions(&w.conditions),
                            "tracking": w.track,
                        })
                    })
                    .collect::<Vec<_>>()
            };

            let mut response = serde_json::json!({
                "watches": watches,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        "clear" => {
            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.clear()
            };

            let mut response = serde_json::json!({
                "result": "ok",
                "removed": removed,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        other => Err(McpError::invalid_params(
            format!("Unknown action '{other}'. Valid: add, remove, list, clear"),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_operator_valid() {
        assert!(matches!(parse_operator("lt"), Ok(ConditionOperator::Lt)));
        assert!(matches!(parse_operator("gt"), Ok(ConditionOperator::Gt)));
        assert!(matches!(parse_operator("eq"), Ok(ConditionOperator::Eq)));
        assert!(matches!(
            parse_operator("changed"),
            Ok(ConditionOperator::Changed)
        ));
    }

    #[test]
    fn parse_operator_invalid() {
        assert!(parse_operator("invalid").is_err());
    }

    #[test]
    fn parse_track_valid() {
        assert!(matches!(parse_track("all"), Ok(TrackCategory::All)));
        assert!(matches!(
            parse_track("position"),
            Ok(TrackCategory::Position)
        ));
    }

    #[test]
    fn parse_track_invalid() {
        assert!(parse_track("everything").is_err());
    }

    #[test]
    fn format_conditions_empty() {
        assert_eq!(format_conditions(&[]), "none");
    }

    #[test]
    fn format_conditions_single() {
        let conds = vec![WatchCondition {
            property: "health".to_string(),
            operator: ConditionOperator::Lt,
            value: Some(serde_json::json!(20.0)),
        }];
        let s = format_conditions(&conds);
        assert!(s.contains("health"));
        assert!(s.contains("20"));
    }
}
