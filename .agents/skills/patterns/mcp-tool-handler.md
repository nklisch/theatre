# Pattern: Stage MCP Tool Handler

Most Stage tools that return ordinary JSON text use a `#[tool_router]` method on
`StageServer`, typed extraction through `Parameters<T>`, a focused handler, and a
`Result<String, McpError>` return. Engine-backed handlers serialize a typed query,
call `query_addon`, and deserialize the response before applying server-side
reasoning or response shaping.

## Rationale

The rmcp `#[tool_router]` macro generates dispatch and `Parameters<T>` provides the
typed input boundary. A string result is the simplest shape for text-only JSON,
but it is not an SDK requirement or a universal Theatre convention. Tools that
need multiple MCP content blocks return `CallToolResult` instead.

## Examples

### Ordinary text handler

**File**: `crates/stage-server/src/mcp/mod.rs`

```rust
#[tool(description = "See what changed since the last query...")]
pub async fn spatial_delta(
    &self,
    Parameters(params): Parameters<SpatialDeltaParams>,
) -> Result<String, McpError> {
    let result = delta::handle_spatial_delta(params, &self.state).await;
    self.log_activity("query", &crate::activity::delta_summary(), "spatial_delta")
        .await;
    result
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

Use the ordinary text-handler shape for Stage tools that return one JSON text
payload. For live engine data, use the typed protocol helpers and `query_addon`.
Apply budget finalization and activity logging when those are part of that tool's
contract and lifecycle.

Return `CallToolResult` when the tool needs mixed MCP content or shared content
handling rather than flattening images into a text-only response.

## When Not to Use

- Project-local feedback does not need a Stage engine connection.
- Viewport and image-producing clip operations need mixed content.
- Connection management and background runtime work belong in their owning
  server or transport modules, not a tool handler.

## Common Violations

- Claiming every rmcp handler must return `Result<String, McpError>`.
- Routing project-local feedback through the addon.
- Flattening viewport or feedback image content into ordinary handler text.
- Injecting a budget block into a response whose contract does not define one.
