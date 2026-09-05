use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_limit() -> u32 {
    25
}

/// One focused category of ClassDB metadata.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EngineApiCategory {
    #[default]
    Summary,
    Properties,
    Methods,
    Signals,
    Enums,
}

/// Parameters for `engine_api`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineApiParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Built-in ClassDB class to inspect, such as Node2D or StandardMaterial3D.
    pub class_name: String,

    /// Metadata category to return. Summary is the bounded default.
    #[serde(default)]
    pub category: EngineApiCategory,

    /// Exact property, method, signal, or enum name to select.
    #[serde(default)]
    pub member: Option<String>,

    /// Zero-based offset into the selected category after deterministic sorting.
    #[serde(default)]
    pub offset: u32,

    /// Maximum members to return. Must be between 1 and 100.
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: u32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub status: String,
    pub build: String,
    pub hash: String,
    pub string: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineClassIdentity {
    pub class_name: String,
    pub parent_class: String,
    pub instantiable: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineApiCounts {
    pub properties: u32,
    pub methods: u32,
    pub signals: u32,
    pub enums: u32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineApiPage {
    pub offset: u32,
    pub limit: u32,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DefaultRepresentation {
    Json,
    Serialized,
    Text,
    Unavailable,
}

/// A ClassDB property or method default with an explicit representation contract.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineDefaultValue {
    pub native_type: String,
    pub representation: DefaultRepresentation,
    /// JSON-native or existing Director-serialized value. Null for text/unavailable defaults.
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineApiArgument {
    pub name: String,
    pub value_type: u32,
    pub type_name: String,
    pub class_name: String,
    pub hint: u32,
    pub hint_string: String,
    pub usage: u32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineEnumValue {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineApiMember {
    Property {
        name: String,
        declared_by: String,
        value_type: u32,
        type_name: String,
        class_name: String,
        hint: u32,
        hint_string: String,
        usage: u32,
        default_value: EngineDefaultValue,
    },
    Method {
        name: String,
        declared_by: String,
        flags: u32,
        arguments: Vec<EngineApiArgument>,
        return_value: EngineApiArgument,
        default_arguments: Vec<EngineDefaultValue>,
    },
    Signal {
        name: String,
        declared_by: String,
        arguments: Vec<EngineApiArgument>,
    },
    Enum {
        name: String,
        declared_by: String,
        bitfield: bool,
        values: Vec<EngineEnumValue>,
    },
}

/// Focused ClassDB metadata for one installed-engine class.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EngineApiResponse {
    pub engine_version: EngineVersion,
    pub class: EngineClassIdentity,
    pub category: EngineApiCategory,
    pub counts: EngineApiCounts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<EngineApiMember>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<EngineApiPage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_default_and_limit_is_bounded_in_schema() {
        let params: EngineApiParams = serde_json::from_value(serde_json::json!({
            "project_path": "/project",
            "class_name": "Node2D"
        }))
        .unwrap();

        assert!(matches!(params.category, EngineApiCategory::Summary));
        assert_eq!(params.offset, 0);
        assert_eq!(params.limit, 25);

        let schema = schemars::schema_for!(EngineApiParams);
        let json = serde_json::to_value(schema).unwrap();
        assert_eq!(json["properties"]["limit"]["minimum"], 1);
        assert_eq!(json["properties"]["limit"]["maximum"], 100);
    }
}
