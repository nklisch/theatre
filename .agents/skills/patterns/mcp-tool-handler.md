# Pattern: Stage MCP Tool Handler

Stage MCP tools use a `#[tool_router]` method on `StageServer`, typed extraction
through `Parameters<T>`, and a focused shared handler. A shared JSON handler can
return `Result<String, McpError>` for CLI use; its MCP wrapper returns
`CallToolResult` with matching text and structured content. Engine-backed handlers serialize a typed query,
call `query_addon`, and deserialize the response before applying server-side
reasoning or response shaping.

## Rationale

The rmcp `#[tool_router]` macro generates dispatch and `Parameters<T>` provides the
typed input boundary. Declaring an output schema also requires structured
content: a JSON string alone is not that envelope. The MCP wrapper performs the
conversion explicitly, without changing the shared CLI handler's return type.

## Examples

### Structured MCP wrapper

**File**: `crates/stage-server/src/mcp/mod.rs`

```rust
#[tool(description = "See what changed since the last query...")]
pub async fn spatial_delta(
    &self,
    Parameters(params): Parameters<SpatialDeltaParams>,
) -> Result<rmcp::model::CallToolResult, McpError> {
    let result = delta::handle_spatial_delta(params, &self.state).await;
    self.log_activity("query", &crate::activity::delta_summary(), "spatial_delta")
        .await;
    result.and_then(stage_protocol::mcp_helpers::structured_json)
}
```

### Engine query and budgeted text response

**Files**: `crates/stage-server/src/mcp/mod.rs` and
`crates/stage-protocol/src/mcp_helpers.rs`

```rust
let raw: NodeInspectResponse = query_and_deserialize(
    state,
    "get_node_inspect",
    &query_params,
).await?;

let mut response = build_response(raw);
finalize_response(&mut response, budget_limit, hard_cap)
```

`finalize_response` estimates the serialized size, injects the documented budget
block, and serializes the JSON. Use it only for handlers whose response contract
is budgeted.

### Mixed-content and project-local handlers

`viewport` returns `Result<CallToolResult, McpError>` so it can include JSON
metadata and an image content block. `clips` does the same because some clip
operations return images. `feedback` also returns `CallToolResult`; it reads the
selected project's retained feedback queue directly and does not query the live
engine.

## When to Use

Use the shared JSON handler plus structured MCP wrapper for ordinary JSON
operations. For live engine data, use the typed protocol helpers and `query_addon`.
Apply budget finalization and activity logging when those are part of that tool's
contract and lifecycle.

Preserve image blocks in mixed-content `CallToolResult` responses. Do not flatten
them into handler text or treat text-only JSON as structured content.

## When Not to Use

- Project-local feedback does not need a Stage engine connection.
- Viewport and image-producing clip operations need mixed content.
- Connection management and background runtime work belong in their owning
  server or transport modules, not a tool handler.

## Common Violations

- Returning only a string from an MCP wrapper that declares an output schema.
- Routing project-local feedback through the addon.
- Flattening viewport or feedback image content into ordinary handler text.
- Injecting a budget block into a response whose contract does not define one.
