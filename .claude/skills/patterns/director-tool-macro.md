# Pattern: Director Tool Macro

Every Director MCP tool handler is a one-liner using the `director_tool!` macro, which wraps `serialize_params` → `run_operation` → `serialize_response` in a single expression.

## Rationale

Director has 38+ tools that all follow the same dispatch path: serialize params to JSON, route through the backend (editor/daemon/one-shot), unwrap the result, serialize to string. The macro collapses this to a single call per tool, keeping boilerplate in one place.

Unlike Stage tools, Director tools:
- Have no activity logging (no addon-side instrumentation)
- Have no budget/token injection (`finalize_response` not used)
- Always include `project_path: String` as the first field in every params struct
- Route through `run_operation` which handles editor → daemon → one-shot fallback

## Examples

### Example 1: Minimal tool (scene_create)
**File**: `crates/director/src/mcp/mod.rs:88-93`
```rust
pub async fn scene_create(
    &self,
    Parameters(params): Parameters<SceneCreateParams>,
) -> Result<String, McpError> {
    director_tool!(self, params, "scene_create", SceneCreateResponse)
}
```

### Example 2: Macro definition
**File**: `crates/director/src/mcp/mod.rs:48-54`
```rust
macro_rules! director_tool {
    ($self:expr, $params:expr, $op:expr, $resp:ty) => {{
        let op_params = serialize_params(&$params)?;
        let data = run_operation(&$self.backend, &$params.project_path, $op, &op_params).await?;
        let typed: $resp = deserialize_response(data)?;
        serialize_response(&typed)
    }};
}
```

### Example 3: Typical params struct layout
**File**: `crates/director/src/mcp/node.rs:5-16`
```rust
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeAddParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,          // Always first; required by macro ($params.project_path)

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
- Every new Director MCP tool: place the handler body as `director_tool!(self, params, "op_name", ResponseType)`
- The operation name string (`"op_name"`) must match the GDScript `match` arm in `operations.gd`
- The response type (`ResponseType`) must be a struct that implements `Deserialize + Serialize`

## When NOT to Use
- Stage tools — they use `query_addon` + `finalize_response` + `log_activity` instead
- Any tool that needs custom response transformation before returning — extract the logic into a helper and call `director_tool!` inside it, or skip the macro

## Common Violations
- Adding activity logging to Director tools (Director has no addon-side logging)
- Calling `finalize_response` on Director responses (no budget block in Director protocol)
- Omitting `project_path` from params struct (macro accesses `$params.project_path` directly)
