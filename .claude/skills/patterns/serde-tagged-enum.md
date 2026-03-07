# Pattern: Serde Tagged Enum

Protocol enums use `#[serde(tag = "type")]` (internally tagged) or `#[serde(rename_all = "snake_case")]` for wire-format dispatch. All enum variants have explicit `#[serde(rename = "...")]` snake_case names to ensure stable JSON keys.

## Rationale
Tagged enums map cleanly to the JSON `"type"` field pattern used across the TCP protocol. `rename_all = "snake_case"` ensures Rust PascalCase enum variants serialize to snake_case without per-variant annotations. Used consistently across `spectator-protocol` and `spectator-core`.

## Examples

### Example 1: Top-level protocol dispatch (internally tagged)
**File**: `crates/spectator-protocol/src/messages.rs:4-47`
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "handshake")]
    Handshake(crate::handshake::Handshake),
    #[serde(rename = "query")]
    Query { id: String, method: String, #[serde(default)] params: serde_json::Value },
    #[serde(rename = "response")]
    Response { id: String, data: serde_json::Value },
    #[serde(rename = "error")]
    Error { id: String, code: String, message: String },
    // ...
}
```

### Example 2: Action request dispatch (internally tagged)
**File**: `crates/spectator-protocol/src/query.rs` — `ActionRequest` enum uses `#[serde(tag = "type")]` so that `{"type":"teleport","path":"...","position":[...]}` deserializes directly.

### Example 3: Internal enums with rename_all
**File**: `crates/spectator-core/src/delta.rs:44-50`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferedEventType {
    SignalEmitted,
    NodeEntered,
    NodeExited,
}
```

### Example 4: PerspectiveParam — externally-tagged with inline structs
**File**: `crates/spectator-protocol/src/query.rs` — `PerspectiveParam` uses `#[serde(tag = "type")]` for `Camera`, `Node { path }`, `Point { position }` variants.

## When to Use
- Any enum that appears in JSON and needs a discriminant field: use `#[serde(tag = "type")]`
- Enums with all-unit or single-field variants: use `#[serde(rename_all = "snake_case")]`
- New protocol message kinds: add a variant to `Message` with explicit `#[serde(rename = "...")]`

## When NOT to Use
- Enums that are never serialized — no annotation needed
- Untagged unions (e.g., `QueryOrigin` that can be a string OR array) — use `#[serde(untagged)]` instead

## Common Violations
- Adding a new enum variant without `#[serde(rename = "...")]` — Rust PascalCase becomes inconsistent in JSON
- Using `#[serde(rename_all = "snake_case")]` on a tagged enum with nested structs that have their own naming — verify round-trip tests
