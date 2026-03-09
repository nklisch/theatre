# Pattern: ClipSession Resource

All clip analysis operations open a recording via `ClipSession::open()` and
finalize with `session.finalize()`. This bundles DB connection, metadata, and
storage path into one struct so every handler has a consistent setup path.

## Rationale

Clip operations require: resolving the storage path (cached), opening the
SQLite DB read-only, and reading recording metadata. `ClipSession` provides a
single async entry point for all of this, with uniform error propagation.
`finalize()` injects `clip_context` and calls the standard `finalize_response`
in one step.

## Structure

```
1. ClipSession::open(state, clip_id) — resolve path, open DB, read metadata
2. Validate frame ranges via session.meta.validate_frame(f)
3. Call analysis function using session.db
4. session.finalize(&mut response, budget, hard_cap) — inject clip_context + budget
5. Return
```

## Examples

### Example 1: ClipSession definition
**File**: `crates/spectator-server/src/clip_analysis.rs:161`
```rust
pub struct ClipSession {
    pub db: Connection,
    pub meta: ClipMeta,
    pub storage_path: String,
    pub clip_id: String,
}

impl ClipSession {
    pub async fn open(
        state: &Arc<Mutex<SessionState>>,
        clip_id: Option<&str>,  // None → most recent clip
    ) -> Result<Self, McpError> {
        let storage_path = resolve_clip_storage_path(state).await?;
        let clip_id = match clip_id {
            Some(id) => id.to_string(),
            None => most_recent_clip(&storage_path).ok_or_else(|| {
                McpError::invalid_params("No clip_id specified and no clips found", None)
            })?,
        };
        let db = open_clip_db(&storage_path, &clip_id)?;
        let meta = read_recording_meta(&db)?;
        Ok(Self { db, meta, storage_path, clip_id })
    }

    pub fn finalize(
        &self,
        response: &mut serde_json::Value,
        budget_limit: u32,
        hard_cap: u32,
    ) -> Result<String, McpError> {
        if let Some(obj) = response.as_object_mut() {
            obj.insert("clip_context".into(), self.meta.to_context());  // Always injected
        }
        crate::mcp::finalize_response(response, budget_limit, hard_cap)
    }
}
```

### Example 2: snapshot_at handler using ClipSession
**File**: `crates/spectator-server/src/mcp/clips.rs`
```rust
"snapshot_at" => {
    let session = ClipSession::open(&self.state, params.clip_id.as_deref()).await?;
    let frame = resolve_frame(&session, params.at_frame, params.at_time_ms, "snapshot_at")?;
    let entities = clip_analysis::read_frame(&session.db, frame)?;
    let mut response = json!({
        "frame": frame,
        "result": build_frame_snapshot(&entities, &session.meta),
    });
    session.finalize(&mut response, budget_limit, hard_cap)
}
```

### Example 3: trajectory handler using ClipSession
**File**: `crates/spectator-server/src/mcp/clips.rs`
```rust
"trajectory" => {
    let session = ClipSession::open(&self.state, params.clip_id.as_deref()).await?;
    let (from, to) = resolve_frame_range(&session, params.from_frame, params.to_frame, "trajectory")?;
    let traj = clip_analysis::build_trajectory(&session.db, from, to, &params.node)?;
    let mut response = json!({ "result": traj, "from_frame": from, "to_frame": to });
    session.finalize(&mut response, budget_limit, hard_cap)
}
```

## Frame Resolution Helpers

Two private helpers in `clips.rs` normalize frame-or-time inputs:

```rust
fn resolve_frame(
    session: &ClipSession,
    at_frame: Option<u64>,
    at_time_ms: Option<u64>,
    action: &str,
) -> Result<u64, McpError> { ... }

fn resolve_frame_range(
    session: &ClipSession,
    from: Option<u64>,
    to: Option<u64>,
    action: &str,
) -> Result<(u64, u64), McpError> { ... }
```

## Storage Path Caching

`resolve_clip_storage_path` queries the addon once, then caches in `SessionState.clip_storage_path`.

**File**: `crates/spectator-server/src/clip_analysis.rs:20`

## When to Use

- Any clips handler that reads frame data from a recording DB
- Use `clip_id: Option<&str>` → `ClipSession::open` so `None` defaults to most recent

## When NOT to Use

- Clip list/delete/marker/status operations that don't read frame data (use `resolve_clip_storage_path` directly)
- Do not open `ClipSession` for write operations — DB is opened read-only

## Common Violations

- Opening the DB directly with `open_clip_db` instead of using `ClipSession::open` (misses metadata)
- Forgetting to call `session.finalize()` (omits `clip_context` from response)
- Calling `finalize_response` directly when `session.finalize()` is available
