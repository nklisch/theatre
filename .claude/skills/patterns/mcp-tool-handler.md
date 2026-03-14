# Pattern: MCP Tool Handler

All MCP tools follow the same structure: `#[tool_router]` impl block on `StageServer`, individual `async fn` methods with `#[tool]`, typed parameter extraction via `Parameters<T>`, `query_addon` for addon calls, and `log_activity` at the end.

## Rationale
The rmcp `#[tool_router]` macro generates the dispatch table. `Parameters<T>` deserialization handles validation. All responses are `Result<String, McpError>` — MCP SDK requirement. Activity logging at the end of every handler provides an audit trail.

## Examples

### Example 1: Minimal handler structure (spatial_delta)
**File**: `crates/stage-server/src/mcp/mod.rs:405-412`
```rust
#[tool(description = "See what changed since the last query...")]
pub async fn spatial_delta(
    &self,
    Parameters(params): Parameters<SpatialDeltaParams>,
) -> Result<String, McpError> {
    let result = delta::handle_spatial_delta(params, &self.state).await;
    self.log_activity("query", &crate::activity::delta_summary(), "spatial_delta").await;
    result
}
```

### Example 2: Handler with query_addon call and response building (spatial_inspect)
**File**: `crates/stage-server/src/mcp/mod.rs:226-262`
```rust
pub async fn spatial_inspect(
    &self,
    Parameters(params): Parameters<SpatialInspectParams>,
) -> Result<String, McpError> {
    let config = get_config(&self.state).await;
    let include = parse_include(&params.include)?;
    let query_params = GetNodeInspectParams { ... };

    let raw_data: NodeInspectResponse = {
        let data = query_addon(&self.state, "get_node_inspect", serialize_params(&query_params)?)
            .await?;
        deserialize_response(data)?
    };

    let budget_limit = resolve_budget(None, 1500, config.token_hard_cap);
    let result = finalize_response(&mut response, budget_limit, config.token_hard_cap);
    self.log_activity("query", &activity_summary, "spatial_inspect").await;
    result
}
```

### Example 3: Shared helpers used by all handlers
**File**: `crates/stage-server/src/mcp/mod.rs:32-76`
```rust
fn serialize_params<T: Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params).map_err(|e| {
        McpError::internal_error(format!("Param serialization error: {e}"), None)
    })
}

fn deserialize_response<T: for<'de> Deserialize<'de>>(data: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(data).map_err(|e| {
        McpError::internal_error(format!("Response deserialization error: {e}"), None)
    })
}

fn finalize_response(response: &mut serde_json::Value, budget_limit: u32, hard_cap: u32) -> Result<String, McpError> {
    let json_bytes = serde_json::to_vec(response).unwrap_or_default().len();
    let used = stage_core::budget::estimate_tokens(json_bytes);
    inject_budget(response, used, budget_limit, hard_cap);
    serialize_response(response)
}
```

## When to Use
- Every new MCP tool: add as `async fn` in the `#[tool_router]` impl
- All addon calls: use `query_addon` + `serialize_params` + `deserialize_response`
- All responses: call `log_activity` just before returning, use `finalize_response` for budget injection

## When NOT to Use
- Non-tool server logic (background tasks, connection management) — those go in `tcp.rs` or `server.rs`

## Common Violations
- Returning `Err(McpError::...)` directly from inside `query_addon` without the `?` — always use `?` to propagate
- Forgetting `log_activity` — every handler must log at the end
- Skipping `finalize_response` / budget injection — all responses need the `budget` block
