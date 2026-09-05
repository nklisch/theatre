# Pattern: Director Tool Macro

Most ordinary Director MCP handlers that dispatch a Godot operation use the
`director_tool!` macro. It combines parameter serialization, normal backend
routing, typed response validation, and matching JSON text/structured MCP content.

## Rationale

Scene, node, resource, and similar authoring tools share one operation path. The
macro keeps that repeated boundary code in one place while preserving a typed
response for each operation. It is a convenience for the common path, not a
universal Director-handler abstraction.

## Examples

### Ordinary backend-dispatched tool

**File**: `crates/director/src/mcp/mod.rs`

```rust
pub async fn scene_create(
    &self,
    Parameters(params): Parameters<SceneCreateParams>,
) -> Result<CallToolResult, McpError> {
    director_tool!(self, params, "scene_create", SceneCreateResponse)
}
```

### Macro definition

**File**: `crates/director/src/mcp/mod.rs`

```rust
macro_rules! director_tool {
    ($self:expr, $params:expr, $op:expr, $resp:ty) => {{
        let op_params = serialize_params(&$params)?;
        let data = run_operation(&$self.backend, &$params.project_path, $op, &op_params).await?;
        let typed: $resp = deserialize_response(data)?;
        structured_response(&typed)
    }};
}
```

For this common path, `run_operation` validates the project, resolves Godot, and
routes through editor, daemon, then one-shot fallback. The operation result's
`data` is validated against the selected Rust response type.

### Optional fields

Director parameter structs use `#[serde_with::skip_serializing_none]` when they
contain optional fields. Omitting `None` matters because a GDScript
`Dictionary.get(key, default)` does not use its default when the key is present
with a null value.

```rust
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeAddParams {
    pub project_path: String,
    pub scene_path: String,
    #[serde(default = "default_root")]
    pub parent_path: String,
    pub node_type: String,
    pub node_name: String,
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}
```

## When to Use

Use `director_tool!` for an ordinary structured-response Director operation that uses
the normal editor → daemon → one-shot backend order and maps directly to one
typed response.

The operation string must match the GDScript dispatcher, and the response type
must implement `Deserialize + Serialize`.

## When Not to Use

Use a dedicated handler when the tool's execution or response differs from the
common path:

- `project_reload` controls daemon lifecycle and performs its own validation run.
- `editor_status` post-processes editor log output.
- `editor_run` is editor-only and must not fall back to headless execution.
- `feedback` reads project-local evidence without launching Godot and returns
  `CallToolResult` so retrieved images remain MCP image content.

Stage tools use their own engine-query and response-shaping paths.

## Common Violations

- Describing the macro as required for every Director tool.
- Using normal backend fallback for an editor-only operation.
- Routing feedback through Godot instead of the project-local queue.
- Omitting `project_path` from a common-path parameter type.
- Serializing optional fields as null when omission is required by the GDScript
  defaulting behavior.
