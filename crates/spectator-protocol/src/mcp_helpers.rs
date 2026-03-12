//! Shared MCP serde helpers for spectator-server and director.
//!
//! Enabled with the `mcp` feature flag (requires `rmcp`).

use rmcp::model::ErrorData as McpError;
use serde::{Deserialize, Serialize};

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
