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

/// Normalize generated schemas for strict JSON Schema MCP clients.
///
/// rmcp's OpenAPI-oriented generation can emit `nullable` beside a `$ref`
/// without `type`; strict clients reject that extension. Express nullability
/// as a standard union, while keeping schema resource identifiers in place.
pub fn normalize_mcp_schema(value: &mut serde_json::Value) {
    fn normalize(schema: &mut schemars::Schema) {
        if schema.as_bool() == Some(true) {
            *schema = schemars::json_schema!({});
            return;
        }
        // Traverse schema positions only, never defaults, consts or examples.
        schemars::transform::transform_subschemas(&mut normalize, schema);
        let Some(object) = schema.as_object_mut() else {
            return;
        };
        let nullable = object.remove("nullable");
        if nullable != Some(serde_json::Value::Bool(true)) {
            return;
        }
        let mut outer = serde_json::Map::new();
        for key in [
            "$schema",
            "$id",
            "$anchor",
            "$dynamicAnchor",
            "$defs",
            "definitions",
        ] {
            if let Some(value) = object.remove(key) {
                outer.insert(key.into(), value);
            }
        }
        let original = std::mem::take(object);
        outer.insert(
            "anyOf".into(),
            serde_json::json!([original, {"type":"null"}]),
        );
        *object = outer;
    }
    if let Ok(schema) = value.try_into() {
        normalize(schema);
    }
}

/// Return typed operation data as both MCP structured content and readable JSON.
pub fn structured_response<T: Serialize>(
    response: &T,
) -> Result<rmcp::model::CallToolResult, McpError> {
    serde_json::to_value(response)
        .map(rmcp::model::CallToolResult::structured)
        .map_err(|e| McpError::internal_error(format!("Response serialization error: {e}"), None))
}

/// Adapt a JSON-producing shared CLI handler at the explicit MCP boundary.
pub fn structured_json(text: String) -> Result<rmcp::model::CallToolResult, McpError> {
    serde_json::from_str(&text)
        .map(rmcp::model::CallToolResult::structured)
        .map_err(|e| McpError::internal_error(format!("Invalid handler JSON response: {e}"), None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalization_changes_schemas_not_boolean_data() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "anything": true,
                "enabled": {"type":"boolean", "default":true, "const":true, "enum":[true,false]},
                "list": {"type":"array", "items":true},
                "never": false
            },
            "examples": [{"nullable":true, "properties":{"literal":true}}],
            "anyOf": [true, {"properties":{"nested":true}}],
            "additionalProperties": false
        });
        let literals = schema["properties"]["enabled"].clone();
        let examples = schema["examples"].clone();
        normalize_mcp_schema(&mut schema);
        assert_eq!(schema["properties"]["enabled"], literals);
        assert_eq!(schema["examples"], examples);
        assert_eq!(schema["properties"]["anything"], json!({}));
        assert_eq!(schema["properties"]["list"]["items"], json!({}));
        assert_eq!(schema["anyOf"][0], json!({}));
        assert_eq!(schema["anyOf"][1]["properties"]["nested"], json!({}));
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["never"], false);
    }

    #[test]
    fn nullable_references_become_standard_unions_without_moving_definitions() {
        let mut schema = json!({
            "$defs":{"Identity":{"type":"object"}},
            "type":"object",
            "properties":{
                "identity":{"$ref":"#/$defs/Identity", "nullable":true},
                "anything":{"nullable":true},
                "label":{"type":"string", "nullable":false}
            },
            "examples":[{"nullable":true}]
        });
        normalize_mcp_schema(&mut schema);
        assert_eq!(
            schema["properties"]["identity"]["anyOf"],
            json!([
                {"$ref":"#/$defs/Identity"}, {"type":"null"}
            ])
        );
        assert_eq!(schema["$defs"]["Identity"]["type"], "object");
        assert_eq!(
            schema["properties"]["anything"],
            json!({"anyOf":[{}, {"type":"null"}]})
        );
        assert_eq!(schema["properties"]["label"], json!({"type":"string"}));
        assert_eq!(schema["examples"], json!([{"nullable":true}]));
    }

    #[test]
    fn structured_response_keeps_matching_text_and_data() {
        let data = json!({"connected":false, "ready":false});
        let result = structured_response(&data).unwrap();
        assert_eq!(result.structured_content, Some(data.clone()));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result.content[0].as_text().unwrap().text)
                .unwrap(),
            data
        );
        assert_eq!(result.is_error, Some(false));
        assert!(structured_json("not JSON".to_owned()).is_err());
    }
}
