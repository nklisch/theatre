//! Shared MCP serde helpers for stage-server and director.
//!
//! Enabled with the `mcp` feature flag (requires `rmcp`).

use rmcp::model::ErrorData as McpError;
use serde::{Deserialize, Serialize};

/// Implement `From<$ty> for McpError` mapping all variants to `internal_error`.
#[macro_export]
macro_rules! impl_mcp_internal {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for ::rmcp::model::ErrorData {
                fn from(e: $ty) -> Self {
                    ::rmcp::model::ErrorData::internal_error(e.to_string(), None)
                }
            }
        )+
    };
}

/// Serialize a params struct to a JSON Value for forwarding to the addon.
pub fn serialize_params<T: Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params)
        .map_err(|e| McpError::internal_error(format!("Param serialization error: {e}"), None))
}

/// Deserialize a JSON Value from the addon into a typed response struct.
pub fn deserialize_response<T: for<'de> Deserialize<'de>>(
    data: serde_json::Value,
) -> Result<T, McpError> {
    serde_json::from_value(data)
        .map_err(|e| McpError::internal_error(format!("Response deserialization error: {e}"), None))
}

/// Serialize a response struct to a JSON string for returning to the MCP client.
pub fn serialize_response<T: Serialize>(response: &T) -> Result<String, McpError> {
    serde_json::to_string(response)
        .map_err(|e| McpError::internal_error(format!("Response serialization error: {e}"), None))
}

/// Replace bare `true` values in a JSON Schema tree with `{}`.
///
/// schemars v1 emits `true` for `serde_json::Value` fields (the JSON Schema
/// "accept anything" shorthand). Some MCP clients (notably Claude Code) reject
/// bare booleans during schema validation. `{}` is the equivalent object form.
pub fn replace_bool_schemas(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Bool(true) => {
            *value = serde_json::Value::Object(serde_json::Map::new());
        }
        serde_json::Value::Object(map) => {
            for v in map.values_mut() {
                replace_bool_schemas(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                replace_bool_schemas(v);
            }
        }
        _ => {}
    }
}
