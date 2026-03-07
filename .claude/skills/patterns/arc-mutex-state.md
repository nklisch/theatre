# Pattern: Arc<Mutex<SessionState>> with Background Task

Shared server state lives in `Arc<Mutex<SessionState>>`. The background TCP task and MCP handlers share it. Locks are acquired briefly, released before `await` points to prevent deadlocks. Request-response matching uses `oneshot` channels stored in the state.

## Rationale
MCP handlers and the TCP read loop run on different tokio tasks. `Arc<Mutex<>>` is the standard shared-state primitive. Fine-grained lock scoping (acquire, mutate, drop before awaiting) avoids holding the lock across I/O.

## Examples

### Example 1: SessionState definition — all shared fields in one struct
**File**: `crates/spectator-server/src/tcp.rs:40-55`
```rust
pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub pending_queries: HashMap<String, oneshot::Sender<QueryResult>>,
    pub spatial_index: SpatialIndex,
    pub delta_engine: DeltaEngine,
    pub watch_engine: WatchEngine,
    pub config: SessionConfig,
}
```

### Example 2: Background task spawned from main with Arc clone
**File**: `crates/spectator-server/src/main.rs:53-57`
```rust
let tcp_state = Arc::clone(&state);
tokio::spawn(async move {
    tcp::tcp_client_loop(tcp_state, port).await;
});
```

### Example 3: Fine-grained lock — acquire, mutate, release, then await
**File**: `crates/spectator-server/src/tcp.rs` (query_addon pattern)
```rust
// Register oneshot sender while holding lock
let rx = {
    let mut s = state.lock().await;
    let id = s.tcp_writer.as_mut().unwrap().next_request_id();
    let (tx, rx) = oneshot::channel();
    s.pending_queries.insert(id.clone(), tx);
    // write query to tcp_writer, then drop lock
    rx
};
// Await response WITHOUT holding the lock
rx.await.map_err(|_| McpError::internal_error("disconnected", None))
```

### Example 4: Lock for read-only config snapshot
**File**: `crates/spectator-server/src/tcp.rs` — `get_config` clones config while locked:
```rust
pub async fn get_config(state: &Arc<Mutex<SessionState>>) -> SessionConfig {
    state.lock().await.config.clone()
}
```

## When to Use
- Any state shared between MCP handlers and the TCP background task
- Reading config: clone it out of the lock, don't hold the lock during computation
- Request-response matching: `oneshot::channel` + map keyed by request ID, inserted/removed under lock

## When NOT to Use
- State local to a single handler invocation — use local variables
- State that's only accessed from one task — no mutex needed

## Common Violations
- Holding the lock across an `await` — this blocks all other handlers; always release before awaiting
- Storing `Arc<Mutex<SessionState>>` in the session state itself — creates cycles
- Panicking while holding the lock — poisons the mutex; use `?` to propagate errors instead
