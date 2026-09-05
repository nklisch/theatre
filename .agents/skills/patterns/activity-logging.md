# Pattern: Activity Logging Tail

Every MCP tool handler computes a human-readable summary at the top, runs its
main logic, then pushes a best-effort activity event to the addon at the tail.

## Rationale

The addon's dock panel displays a live tool-activity feed. The server sends
these events over the same TCP connection used for queries. Failures are
silently dropped — a dead TCP writer must never crash a handler.

## Structure

```
1. Compute summary string up front (params may be moved later)
2. Main handler logic (query addon, transform response, serialize)
3. self.log_activity(entry_type, &summary, "tool_name").await;
4. Return result
```

## Examples

### Example 1: spatial_snapshot (standard case)
**File**: `crates/stage-server/src/mcp/mod.rs:154`
```rust
pub async fn spatial_snapshot(
    &self,
    Parameters(params): Parameters<SpatialSnapshotParams>,
) -> Result<String, McpError> {
    let activity_summary = crate::activity::snapshot_summary(&params);  // ← up front

    // ... main logic ...

    let result = serialize_response(&response);
    self.log_activity("query", &activity_summary, "spatial_snapshot").await;  // ← tail
    result
}
```

### Example 2: spatial_inspect (same shape)
**File**: `crates/stage-server/src/mcp/mod.rs:330`
```rust
pub async fn spatial_inspect(
    &self,
    Parameters(params): Parameters<SpatialInspectParams>,
) -> Result<String, McpError> {
    let activity_summary = crate::activity::inspect_summary(&params.node);

    // ... query + finalize ...

    let result = finalize_response(&mut response, budget_limit, config.token_hard_cap);
    self.log_activity("query", &activity_summary, "spatial_inspect").await;
    result
}
```

### Example 3: spatial_watch (metadata variant)
**File**: `crates/stage-server/src/mcp/mod.rs` (watch handler)
```rust
let summary = crate::activity::watch_summary(&params);
let result = watch::handle_spatial_watch(params, &self.state).await;
let active_watches = self.state.lock().await.watch_engine.list().len() as u64;
self.log_activity_with_meta(
    "watch",
    &summary,
    "spatial_watch",
    Some(serde_json::json!({ "active_watches": active_watches })),
).await;
result
```

## Summary Builders

Each tool has a dedicated summary function in `activity.rs`:

| Tool | Function | Entry type |
|------|----------|------------|
| spatial_snapshot | `snapshot_summary(&params)` | `"query"` |
| spatial_inspect | `inspect_summary(&node)` | `"query"` |
| scene_tree | `scene_tree_summary(&params)` | `"query"` |
| spatial_action | `action_summary(&params)` | `"action"` |
| spatial_watch | `watch_summary(&params)` | `"watch"` |
| spatial_delta | `delta_summary()` | `"query"` |
| spatial_config | `config_summary(&params)` | `"config"` |
| clips | `clips_summary(&params)` | `"clips"` |

**File**: `crates/stage-server/src/activity.rs`

## log_activity implementation

**File**: `crates/stage-server/src/server.rs:25`
```rust
pub(crate) async fn log_activity(&self, entry_type: &str, summary: &str, tool: &str) {
    self.log_activity_with_meta(entry_type, summary, tool, None).await;
}

pub(crate) async fn log_activity_with_meta(..., meta: Option<serde_json::Value>) {
    let event = crate::activity::build_activity_message(entry_type, summary, tool, meta);
    let mut s = self.state.lock().await;
    if let Some(ref mut writer) = s.tcp_writer {
        let _ = async_io::write_message(&mut writer.writer, &event).await;
    }
    // Errors silently dropped — best-effort only
}
```

## When to Use

- Every MCP tool handler method in `#[tool_router] impl StageServer`
- Add a new `*_summary()` fn to `activity.rs` for each new tool

## When NOT to Use

- Internal helper functions (only tool handler entry points log activity)
- Do not await the result or propagate errors from `log_activity`

## Common Violations

- Forgetting to build `activity_summary` before params are moved into inner helpers
- Using `log_activity_with_meta` when simple `log_activity` suffices
- Logging activity before the main handler logic completes (result could be an error)
