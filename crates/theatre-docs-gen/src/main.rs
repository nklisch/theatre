use serde::Serialize;
use serde_json::Value;

/// One tool's complete documentation metadata.
#[derive(Serialize)]
struct ToolDoc {
    /// "spectator" or "director"
    server: &'static str,
    /// MCP tool name (e.g. "spatial_snapshot", "scene_create")
    name: String,
    /// Human-readable description from #[tool(description)]
    description: String,
    /// JSON Schema for input parameters (from schemars)
    input_schema: Value,
    /// JSON Schema for output (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    output_schema: Option<Value>,
}

fn main() {
    let spectator_router = spectator_server::server::SpectatorServer::router_with_schemas();
    let director_router = director::server::DirectorServer::new().tool_router;

    let mut tools: Vec<ToolDoc> = Vec::new();

    for tool in spectator_router.list_all() {
        tools.push(ToolDoc {
            server: "spectator",
            name: tool.name.to_string(),
            description: tool.description.as_deref().unwrap_or_default().to_string(),
            input_schema: tool.schema_as_json_value(),
            output_schema: tool
                .output_schema
                .map(|s| Value::Object(s.as_ref().clone())),
        });
    }

    for tool in director_router.list_all() {
        tools.push(ToolDoc {
            server: "director",
            name: tool.name.to_string(),
            description: tool.description.as_deref().unwrap_or_default().to_string(),
            input_schema: tool.schema_as_json_value(),
            output_schema: tool
                .output_schema
                .map(|s| Value::Object(s.as_ref().clone())),
        });
    }

    println!("{}", serde_json::to_string_pretty(&tools).unwrap());
}
