# Design: Milestone 1 — First Useful Tool (`spatial_snapshot`)

## Overview

M1 delivers the first end-to-end useful tool: an agent calls `spatial_snapshot` via MCP and gets real spatial data back from a running Godot game. This requires:

1. **GDExtension collector** — traverses the scene tree, collects node positions, properties, groups
2. **TCP query/response flow** — server sends queries, addon dispatches to collector, returns data
3. **Core spatial logic** — bearing calculation, token budget, clustering
4. **MCP tool** — `spatial_snapshot` with summary/standard/full detail tiers, filtering, pagination

**Exit Criteria:** Agent calls `spatial_snapshot(detail: "summary")` → gets clustered overview of a running 3D Godot scene with correct bearings, distances, and groups. Agent drills into a cluster with `expand`. Agent gets `standard` detail with per-entity data. Token budget is respected. Pagination works for large scenes.

**Depends on:** M0 (TCP handshake, wire protocol, crate structure — all complete)

---

## Implementation Units

### Unit 1: Core Spatial Types (`spectator-core`)

**File:** `crates/spectator-core/src/types.rs`

These are the domain types used throughout the server for spatial computation. They are pure Rust — no Godot, no MCP.

```rust
use serde::{Deserialize, Serialize};

/// A 3D position in world space. (2D uses [x, y] — handled in M9.)
pub type Position3 = [f64; 3];

/// Rotation in degrees (yaw for 3D standard output).
pub type RotationDeg = f64;

/// Velocity vector.
pub type Velocity3 = [f64; 3];

/// 8-direction cardinal bearing relative to a perspective's facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinal {
    Ahead,
    AheadRight,
    Right,
    BehindRight,
    Behind,
    BehindLeft,
    Left,
    AheadLeft,
}

/// Elevation classification (3D only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Elevation {
    /// Target is within ±threshold meters (default 2m).
    Level,
    /// Target is above by N meters (rounded).
    #[serde(rename = "above")]
    Above(f64),
    /// Target is below by N meters (rounded).
    #[serde(rename = "below")]
    Below(f64),
}

/// Relative spatial position from a perspective to a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativePosition {
    /// Straight-line distance in world units.
    pub dist: f64,
    /// 8-direction cardinal bearing.
    pub bearing: Cardinal,
    /// Exact bearing in degrees (0 = ahead, clockwise).
    pub bearing_deg: f64,
    /// Elevation classification (3D only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<Elevation>,
    /// Whether line-of-sight is blocked (from camera perspective).
    pub occluded: bool,
}

/// Perspective from which spatial data is computed.
#[derive(Debug, Clone)]
pub struct Perspective {
    pub position: Position3,
    /// Forward direction vector (unit vector on XZ plane for 3D).
    pub forward: [f64; 3],
    /// Facing as cardinal label.
    pub facing: Cardinal,
    /// Facing as degrees from north (0=north/+Z, clockwise).
    pub facing_deg: f64,
}

/// Raw data for a single entity received from the addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEntityData {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub rotation_deg: [f64; 3],
    pub velocity: Velocity3,
    pub groups: Vec<String>,
    pub state: serde_json::Map<String, serde_json::Value>,
    pub visible: bool,
    pub is_static: bool,
    #[serde(default)]
    pub children: Vec<ChildInfo>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub signals_recent: Vec<RecentSignal>,
    #[serde(default)]
    pub signals_connected: Vec<String>,
    #[serde(default)]
    pub physics: Option<PhysicsData>,
    #[serde(default)]
    pub transform: Option<TransformData>,
}

/// Minimal child info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildInfo {
    pub name: String,
    pub class: String,
}

/// Recent signal emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSignal {
    pub signal: String,
    pub frame: u64,
}

/// Physics state of a CharacterBody3D or RigidBody3D.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsData {
    pub velocity: Velocity3,
    pub on_floor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_normal: Option<[f64; 3]>,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

/// Full transform data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformData {
    pub origin: Position3,
    pub basis: [[f64; 3]; 3],
    pub scale: [f64; 3],
}

/// Frame metadata from the addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfo {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub delta: f64,
}
```

**File:** `crates/spectator-core/src/lib.rs`

```rust
pub mod bearing;
pub mod budget;
pub mod cluster;
pub mod types;
```

**Acceptance Criteria:**
- [ ] All types compile and serialize/deserialize round-trip correctly
- [ ] `Cardinal` serializes to lowercase snake_case: `"ahead_left"`, `"behind_right"`
- [ ] `Elevation` serializes as `"level"`, `{"above": 5.0}`, `{"below": 3.0}`
- [ ] `Option` fields are omitted from JSON when `None`

---

### Unit 2: Bearing Calculation (`spectator-core`)

**File:** `crates/spectator-core/src/bearing.rs`

```rust
use crate::types::{Cardinal, Elevation, Perspective, Position3, RelativePosition};

/// Elevation threshold in meters (above/below).
const ELEVATION_THRESHOLD: f64 = 2.0;

/// Compute the relative position of `target` from `perspective`.
///
/// `occluded` is passed in from the addon's camera visibility check.
pub fn relative_position(
    perspective: &Perspective,
    target: Position3,
    occluded: bool,
) -> RelativePosition {
    // ...
}

/// Compute straight-line distance between two 3D points.
pub fn distance(a: Position3, b: Position3) -> f64 {
    // ...
}

/// Compute bearing angle in degrees from `perspective` forward to `target`.
/// 0 = ahead (aligned with forward), clockwise.
///
/// Projects both vectors onto the XZ plane (Y-up, Godot convention).
pub fn bearing_deg(perspective: &Perspective, target: Position3) -> f64 {
    // 1. Compute direction vector from perspective to target on XZ plane
    // 2. Compute forward vector on XZ plane
    // 3. atan2 to get signed angle
    // 4. Convert to 0-360 clockwise from ahead
}

/// Map a bearing in degrees (0=ahead, clockwise) to an 8-direction cardinal.
/// Each direction covers a 45-degree arc centered on it.
pub fn to_cardinal(degrees: f64) -> Cardinal {
    // 0-22.5 and 337.5-360 → Ahead
    // 22.5-67.5 → AheadRight
    // 67.5-112.5 → Right
    // 112.5-157.5 → BehindRight
    // 157.5-202.5 → Behind
    // 202.5-247.5 → BehindLeft
    // 247.5-292.5 → Left
    // 292.5-337.5 → AheadLeft
}

/// Compute elevation classification.
pub fn elevation(perspective_y: f64, target_y: f64) -> Elevation {
    let diff = target_y - perspective_y;
    if diff.abs() <= ELEVATION_THRESHOLD {
        Elevation::Level
    } else if diff > 0.0 {
        Elevation::Above(diff.round())
    } else {
        Elevation::Below((-diff).round())
    }
}

/// Build a Perspective from a position and yaw rotation in degrees.
/// Godot convention: 0° = facing -Z, 90° = facing -X (Y-up, right-hand).
pub fn perspective_from_yaw(position: Position3, yaw_deg: f64) -> Perspective {
    // Convert yaw to forward vector on XZ plane
    // Also compute cardinal facing and facing_deg
}

/// Global compass bearing of a forward vector.
/// Returns degrees: 0=north(+Z in Godot... or -Z). We use Godot convention.
pub fn compass_bearing(forward: [f64; 3]) -> (Cardinal, f64) {
    // ...
}
```

**Implementation Notes:**
- Godot uses a Y-up, right-handed coordinate system. Forward is typically -Z.
- `yaw_deg` in Godot: 0° faces -Z, positive rotates counterclockwise when viewed from above (right-hand rule around Y).
- Bearing is relative to the perspective's forward: 0° = aligned with forward, increases clockwise from above.
- The XZ plane projection ignores vertical (Y) differences for bearing calculation.

**Acceptance Criteria:**
- [ ] `distance([0,0,0], [3,4,0])` returns `5.0`
- [ ] `to_cardinal(0.0)` returns `Ahead`, `to_cardinal(90.0)` returns `Right`, `to_cardinal(180.0)` returns `Behind`
- [ ] `to_cardinal(45.0)` returns `AheadRight` (center of arc)
- [ ] `to_cardinal(22.4)` returns `Ahead` (edge of arc)
- [ ] `to_cardinal(22.6)` returns `AheadRight` (just past edge)
- [ ] `elevation(0.0, 5.0)` returns `Above(5.0)`
- [ ] `elevation(0.0, 1.5)` returns `Level`
- [ ] `elevation(10.0, 3.0)` returns `Below(7.0)`
- [ ] Perspective with yaw=0° (facing -Z in Godot) and target directly ahead (-Z) produces bearing_deg ~0°
- [ ] All bearing tests use Godot's coordinate convention (Y-up, -Z forward at yaw=0)

---

### Unit 3: Token Budget (`spectator-core`)

**File:** `crates/spectator-core/src/budget.rs`

```rust
use serde::{Deserialize, Serialize};

/// Token budget accounting for a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Approximate tokens used in this response.
    pub used: u32,
    /// Effective budget for this call.
    pub limit: u32,
    /// Server-enforced maximum.
    pub hard_cap: u32,
}

/// Default token budgets per detail tier.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotBudgetDefaults;

impl SnapshotBudgetDefaults {
    pub const SUMMARY: u32 = 500;
    pub const STANDARD: u32 = 1500;
    pub const FULL: u32 = 3000;
    pub const HARD_CAP: u32 = 5000;
}

/// Estimate token count from JSON byte size.
/// Rough approximation: 1 token ≈ 4 bytes of JSON.
pub fn estimate_tokens(json_bytes: usize) -> u32 {
    (json_bytes / 4) as u32
}

/// Resolve the effective budget given an optional user-requested budget,
/// a detail tier default, and the hard cap.
pub fn resolve_budget(requested: Option<u32>, tier_default: u32, hard_cap: u32) -> u32 {
    let effective = requested.unwrap_or(tier_default);
    effective.min(hard_cap)
}

/// Budget enforcer that tracks cumulative token usage and signals truncation.
pub struct BudgetEnforcer {
    limit: u32,
    hard_cap: u32,
    used_bytes: usize,
}

impl BudgetEnforcer {
    pub fn new(limit: u32, hard_cap: u32) -> Self {
        Self { limit, hard_cap, used_bytes: 0 }
    }

    /// Check if adding `bytes` would exceed the budget.
    /// Returns true if the item fits within budget.
    pub fn try_add(&mut self, bytes: usize) -> bool {
        let projected = estimate_tokens(self.used_bytes + bytes);
        if projected > self.limit {
            return false;
        }
        self.used_bytes += bytes;
        true
    }

    /// Build the budget report.
    pub fn report(&self) -> BudgetReport {
        BudgetReport {
            used: estimate_tokens(self.used_bytes),
            limit: self.limit,
            hard_cap: self.hard_cap,
        }
    }
}
```

**Acceptance Criteria:**
- [ ] `estimate_tokens(400)` returns `100`
- [ ] `resolve_budget(None, 1500, 5000)` returns `1500`
- [ ] `resolve_budget(Some(3000), 1500, 5000)` returns `3000`
- [ ] `resolve_budget(Some(8000), 1500, 5000)` returns `5000` (clamped to hard cap)
- [ ] `BudgetEnforcer` allows items until budget is exceeded, then `try_add` returns `false`

---

### Unit 4: Clustering Engine (`spectator-core`)

**File:** `crates/spectator-core/src/cluster.rs`

```rust
use crate::types::{Cardinal, RawEntityData, RelativePosition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A cluster of entities for summary-tier output.
#[derive(Debug, Clone, Serialize)]
pub struct Cluster {
    pub label: String,
    pub count: usize,
    pub nearest: Option<ClusterNearest>,
    pub farthest_dist: f64,
    /// Natural-language summary of cluster members' states.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Note for static clusters (e.g., "unchanged").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Nearest entity in a cluster.
#[derive(Debug, Clone, Serialize)]
pub struct ClusterNearest {
    pub node: String,
    pub dist: f64,
    pub bearing: Cardinal,
}

/// Clustering strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterStrategy {
    Group,
    Class,
    Proximity,
    None,
}

impl Default for ClusterStrategy {
    fn default() -> Self { Self::Group }
}

/// Cluster entities by group membership (default strategy).
///
/// Each entity is assigned to its first group. Entities with no groups
/// go into an "other" cluster. Static entities go into a "static_geometry"
/// cluster.
///
/// `entities` must already have `rel` data computed (distances, bearings).
pub fn cluster_by_group(
    entities: &[(RawEntityData, RelativePosition)],
) -> Vec<Cluster> {
    // 1. Separate static vs dynamic
    // 2. Group dynamic entities by first group name
    // 3. Ungrouped → "other"
    // 4. For each group: find nearest/farthest, generate summary
    // 5. Add static cluster with count + "unchanged" note
    // ...
}

/// Generate a natural-language summary for a cluster.
///
/// Examines common state properties (e.g., "state", "alert_level")
/// and counts distinct values. Example output: "2 idle, 1 patrol".
pub fn generate_cluster_summary(
    entities: &[&RawEntityData],
) -> Option<String> {
    // Look for common state keys across all entities
    // Count distinct values for the most common key
    // Format as "N value1, M value2"
    // ...
}
```

**Implementation Notes:**
- For group-based clustering, use the first group in each entity's `groups` list.
- Static entities are always clustered separately as "static_geometry".
- The summary generator looks for properties named `state`, `alert_level`, `status`, or `mode` — these are common game state names. If none found, returns `None`.

**Acceptance Criteria:**
- [ ] 3 entities in group "enemies" → one cluster with label "enemies", count 3
- [ ] Entity with no groups → goes into "other" cluster
- [ ] Static entities → separate "static_geometry" cluster with note "unchanged"
- [ ] `nearest` is the closest entity by distance, `farthest_dist` is the max distance
- [ ] `generate_cluster_summary` for 3 entities with `state: "idle", "idle", "patrol"` returns `Some("2 idle, 1 patrol")`

---

### Unit 5: TCP Query/Response Infrastructure

This unit adds request/response dispatch to both the server and addon. M0 only did handshake — now we need the server to send queries and receive responses.

#### 5a: Protocol Query Types (`spectator-protocol`)

**File:** `crates/spectator-protocol/src/messages.rs` (modify existing)

Add structured query method types. The existing `Query` variant uses `method: String` and `params: Value` which is sufficient — we don't need to change the enum. But we add helper types for common query parameters and response shapes.

**File:** `crates/spectator-protocol/src/query.rs` (new)

```rust
use serde::{Deserialize, Serialize};

/// Parameters for the `get_snapshot_data` query method.
/// Sent by the server to the addon to collect scene data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSnapshotDataParams {
    /// Camera/node/point perspective.
    pub perspective: PerspectiveParam,
    /// Max radius from focal point.
    pub radius: f64,
    /// Whether to include offscreen nodes.
    pub include_offscreen: bool,
    /// Group filter (empty = all groups).
    #[serde(default)]
    pub groups: Vec<String>,
    /// Class filter (empty = all classes).
    #[serde(default)]
    pub class_filter: Vec<String>,
    /// What detail to collect.
    pub detail: DetailLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PerspectiveParam {
    Camera,
    Node { path: String },
    Point { position: Vec<f64> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Summary,
    Standard,
    Full,
}

/// Response data from `get_snapshot_data`.
/// This is the raw data the addon sends back — the server does all processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResponse {
    /// Current frame number.
    pub frame: u64,
    /// Timestamp in ms since game start.
    pub timestamp_ms: u64,
    /// Perspective position and rotation.
    pub perspective: PerspectiveData,
    /// All collected entities (sorted by distance is NOT the addon's job).
    pub entities: Vec<EntityData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveData {
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub forward: Vec<f64>,
}

/// Raw entity data sent by the addon.
/// Simpler than core::RawEntityData — just engine data, no spatial reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub path: String,
    pub class: String,
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub velocity: Vec<f64>,
    pub groups: Vec<String>,
    pub visible: bool,
    /// Exported variable state.
    pub state: serde_json::Map<String, serde_json::Value>,
    // -- standard+ fields --
    #[serde(default)]
    pub signals_recent: Vec<RecentSignalData>,
    // -- full fields --
    #[serde(default)]
    pub children: Vec<ChildData>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub signals_connected: Vec<String>,
    #[serde(default)]
    pub physics: Option<PhysicsEntityData>,
    #[serde(default)]
    pub transform: Option<TransformEntityData>,
    #[serde(default)]
    pub all_exported_vars: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSignalData {
    pub signal: String,
    pub frame: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildData {
    pub name: String,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsEntityData {
    pub velocity: Vec<f64>,
    pub on_floor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_normal: Option<Vec<f64>>,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformEntityData {
    pub origin: Vec<f64>,
    pub basis: Vec<Vec<f64>>,
    pub scale: Vec<f64>,
}

/// Parameters for `get_frame_info` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFrameInfoParams {}

/// Response for `get_frame_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfoResponse {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub delta: f64,
}
```

**File:** `crates/spectator-protocol/src/lib.rs` (modify)

```rust
pub mod codec;
pub mod handshake;
pub mod messages;
pub mod query;
```

**Acceptance Criteria:**
- [ ] `GetSnapshotDataParams` serializes with `perspective` as tagged enum
- [ ] `SnapshotResponse` deserializes correctly with all entity fields
- [ ] Round-trip serialization of `EntityData` with optional fields

#### 5b: Server TCP Query Dispatch (`spectator-server`)

**File:** `crates/spectator-server/src/tcp.rs` (modify existing)

Add ability for MCP handlers to send queries and receive responses through the TCP connection. The key change: the server needs a request/response correlation mechanism.

```rust
use std::collections::HashMap;
use tokio::sync::oneshot;

/// Handle to the TCP connection's write half, for sending queries.
pub struct TcpClientHandle {
    pub writer: WriteHalf<TcpStream>,
    next_id: u64,
}

impl TcpClientHandle {
    pub fn next_request_id(&mut self) -> String {
        self.next_id += 1;
        format!("req_{}", self.next_id)
    }
}

/// Shared state between MCP handlers and TCP client task.
pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
    /// Pending query response channels: request_id → sender.
    pub pending_queries: HashMap<String, oneshot::Sender<QueryResult>>,
}

/// Result of a TCP query to the addon.
pub enum QueryResult {
    /// Successful response data.
    Ok(serde_json::Value),
    /// Error from addon.
    Err { code: String, message: String },
}
```

**File:** `crates/spectator-server/src/tcp.rs` — modify `handle_connection` read loop:

```rust
// Step 5: Read loop — dispatch incoming messages
loop {
    match async_io::read_message::<Message>(&mut reader).await {
        Ok(Message::Response { id, data }) => {
            let mut s = state.lock().await;
            if let Some(sender) = s.pending_queries.remove(&id) {
                let _ = sender.send(QueryResult::Ok(data));
            }
        }
        Ok(Message::Error { id, code, message }) => {
            let mut s = state.lock().await;
            if let Some(sender) = s.pending_queries.remove(&id) {
                let _ = sender.send(QueryResult::Err { code, message });
            }
        }
        Ok(msg) => {
            tracing::debug!("Received message from addon: {:?}", msg);
        }
        Err(e) => {
            tracing::debug!("Read error (likely disconnect): {}", e);
            break;
        }
    }
}
```

**File:** `crates/spectator-server/src/tcp.rs` — add `query_addon` helper:

```rust
/// Send a query to the addon and wait for the response.
///
/// `state` is locked briefly to send the query and register the pending
/// response channel, then released before awaiting the response.
pub async fn query_addon(
    state: &Arc<Mutex<SessionState>>,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let (tx, rx) = oneshot::channel();
    let request_id;

    {
        let mut s = state.lock().await;
        let writer = s.tcp_writer.as_mut().ok_or_else(|| {
            McpError::internal_error(
                "Not connected to Godot addon. Is the game running?",
                None,
            )
        })?;

        request_id = writer.next_request_id();
        s.pending_queries.insert(request_id.clone(), tx);

        let msg = Message::Query {
            id: request_id.clone(),
            method: method.to_string(),
            params,
        };

        async_io::write_message(&mut writer.writer, &msg)
            .await
            .map_err(|e| McpError::internal_error(format!("TCP write error: {e}"), None))?;
    }
    // Lock released — wait for response

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        rx,
    )
    .await
    .map_err(|_| McpError::internal_error(
        "Addon did not respond within 5000ms. Game may be frozen or at a breakpoint.",
        None,
    ))?
    .map_err(|_| McpError::internal_error("TCP connection dropped while waiting for response", None))?;

    match result {
        QueryResult::Ok(data) => Ok(data),
        QueryResult::Err { code, message } => {
            Err(make_spectator_error(&code, &message))
        }
    }
}

/// Map Spectator error codes to McpError.
fn make_spectator_error(code: &str, message: &str) -> McpError {
    match code {
        "node_not_found" => McpError::invalid_params(message, None),
        "scene_not_loaded" => McpError::internal_error(message, None),
        _ => McpError::internal_error(format!("{code}: {message}"), None),
    }
}
```

**Acceptance Criteria:**
- [ ] `query_addon` sends a Query message over TCP and returns the Response data
- [ ] If addon is not connected, returns `McpError::internal_error` with helpful message
- [ ] If addon returns an Error message, it's mapped to an McpError
- [ ] 5-second timeout on addon responses
- [ ] Pending query is cleaned up on timeout, error, and success
- [ ] Multiple concurrent queries (different request IDs) resolve independently

#### 5c: Addon Query Handler (`spectator-godot`)

**File:** `crates/spectator-godot/src/query_handler.rs` (new)

Handles incoming Query messages by dispatching to the collector and sending responses.

```rust
use godot::prelude::*;
use spectator_protocol::{
    codec,
    messages::Message,
    query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam},
};
use std::net::TcpStream;

use crate::collector::SpectatorCollector;

/// Dispatch an incoming query to the appropriate handler.
/// Returns the response Message to send back.
pub fn handle_query(
    id: String,
    method: &str,
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Message {
    let result = match method {
        "get_snapshot_data" => handle_get_snapshot_data(params, collector),
        "get_frame_info" => handle_get_frame_info(collector),
        _ => Err(QueryError {
            code: "method_not_found".to_string(),
            message: format!("Unknown query method: {method}"),
        }),
    };

    match result {
        Ok(data) => Message::Response { id, data },
        Err(e) => Message::Error {
            id,
            code: e.code,
            message: e.message,
        },
    }
}

struct QueryError {
    code: String,
    message: String,
}

fn handle_get_snapshot_data(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSnapshotDataParams = serde_json::from_value(params)
        .map_err(|e| QueryError {
            code: "invalid_params".to_string(),
            message: format!("Invalid params: {e}"),
        })?;

    let data = collector.collect_snapshot(&params);
    serde_json::to_value(&data).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_frame_info(
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let info = collector.get_frame_info();
    serde_json::to_value(&info).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}
```

**File:** `crates/spectator-godot/src/tcp_server.rs` (modify existing)

Integrate the query handler into the message handling loop.

Changes to `SpectatorTCPServer`:
1. Add a `collector: Option<Gd<SpectatorCollector>>` field
2. Add a `#[func] set_collector(&mut self, collector: Gd<SpectatorCollector>)` method
3. Modify `handle_message` to dispatch Query messages to `query_handler::handle_query`
4. Send the response back over TCP

```rust
// In handle_message:
Message::Query { id, method, params } => {
    if let Some(ref collector) = self.collector {
        let response = query_handler::handle_query(
            id,
            &method,
            params,
            &collector.bind(),
        );
        self.send_response(response);
    } else {
        self.send_response(Message::Error {
            id,
            code: "scene_not_loaded".to_string(),
            message: "Collector not available".to_string(),
        });
    }
}
```

Add `send_response` method:
```rust
fn send_response(&mut self, msg: Message) {
    if let Some(stream) = &mut self.client {
        stream.set_nonblocking(false).ok();
        if let Err(e) = codec::write_message(stream, &msg) {
            godot_error!("[Spectator] Failed to send response: {}", e);
            self.disconnect_client();
            return;
        }
        if let Some(stream) = &self.client {
            stream.set_nonblocking(true).ok();
        }
    }
}
```

**File:** `addons/spectator/runtime.gd` (modify existing)

Wire the collector into the TCP server:

```gdscript
extends Node

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector

func _ready() -> void:
    if not ClassDB.class_exists(&"SpectatorTCPServer"):
        push_error("[Spectator] GDExtension not loaded")
        return

    collector = SpectatorCollector.new()
    add_child(collector)

    tcp_server = SpectatorTCPServer.new()
    add_child(tcp_server)
    tcp_server.set_collector(collector)

    var port: int = ProjectSettings.get_setting("spectator/connection/port", 9077)
    tcp_server.start(port)

func _physics_process(_delta: float) -> void:
    if tcp_server:
        tcp_server.poll()

func _exit_tree() -> void:
    if tcp_server:
        tcp_server.stop()
```

**Acceptance Criteria:**
- [ ] Server sends `Query { method: "get_snapshot_data", ... }` over TCP
- [ ] Addon receives query, dispatches to collector, returns `Response { data: ... }`
- [ ] Unknown method returns `Error { code: "method_not_found" }`
- [ ] Invalid params returns `Error { code: "invalid_params" }`
- [ ] Missing collector returns `Error { code: "scene_not_loaded" }`
- [ ] `runtime.gd` creates collector and wires it to TCP server

---

### Unit 6: Scene Collector (`spectator-godot`)

**File:** `crates/spectator-godot/src/collector.rs` (new)

The core data collection class. Traverses the Godot scene tree and returns raw entity data.

```rust
use godot::prelude::*;
use godot::classes::{
    Camera3D, CharacterBody3D, Engine, Node3D, PhysicsBody3D, RigidBody3D,
    StaticBody3D,
};
use spectator_protocol::query::{
    ChildData, DetailLevel, EntityData, FrameInfoResponse, GetSnapshotDataParams,
    PerspectiveData, PerspectiveParam, PhysicsEntityData, RecentSignalData,
    SnapshotResponse, TransformEntityData,
};

/// Static class heuristic: these classes are treated as static by default.
const STATIC_CLASSES: &[&str] = &[
    "StaticBody3D",
    "StaticBody2D",
    "CSGShape3D",
    "CSGBox3D",
    "CSGCylinder3D",
    "CSGMesh3D",
    "CSGPolygon3D",
    "CSGSphere3D",
    "CSGTorus3D",
    "CSGCombiner3D",
    "MeshInstance3D",  // only if no script attached
    "GridMap",
    "WorldEnvironment",
    "DirectionalLight3D",
    "OmniLight3D",
    "SpotLight3D",
];

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorCollector {
    base: Base<Node>,
}

#[godot_api]
impl INode for SpectatorCollector {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl SpectatorCollector {
    /// Collect a full snapshot of the scene for the server.
    /// Called from query_handler when the server sends get_snapshot_data.
    #[func]
    pub fn collect_snapshot_dict(&self, params_json: GString) -> Dictionary {
        // GDScript-callable wrapper, delegates to collect_snapshot
        // (used for testing; query_handler calls collect_snapshot directly)
        Dictionary::new()
    }
}

impl SpectatorCollector {
    /// Collect scene snapshot data based on the provided parameters.
    pub fn collect_snapshot(&self, params: &GetSnapshotDataParams) -> SnapshotResponse {
        let tree = match self.base().get_tree() {
            Some(t) => t,
            None => return SnapshotResponse::empty(),
        };
        let root = match tree.get_current_scene() {
            Some(r) => r,
            None => return SnapshotResponse::empty(),
        };

        // Get perspective data
        let perspective = self.resolve_perspective(&params.perspective);

        // Get frame info
        let frame_info = self.get_frame_info();

        // Collect entities
        let mut entities = Vec::new();
        self.collect_entities_recursive(
            &root,
            &perspective,
            params,
            &mut entities,
        );

        SnapshotResponse {
            frame: frame_info.frame,
            timestamp_ms: frame_info.timestamp_ms,
            perspective,
            entities,
        }
    }

    /// Resolve perspective from camera, node, or point.
    fn resolve_perspective(&self, param: &PerspectiveParam) -> PerspectiveData {
        match param {
            PerspectiveParam::Camera => {
                // Find the active Camera3D in the viewport
                let viewport = self.base().get_viewport();
                if let Some(vp) = viewport {
                    if let Some(camera) = vp.get_camera_3d() {
                        let pos = camera.get_global_position();
                        let rot = camera.get_global_rotation_degrees();
                        let forward = camera.get_global_transform().basis.col_c();
                        return PerspectiveData {
                            position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                            rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
                            forward: vec![forward.x as f64, forward.y as f64, forward.z as f64],
                        };
                    }
                }
                // Fallback: origin facing -Z
                PerspectiveData {
                    position: vec![0.0, 0.0, 0.0],
                    rotation_deg: vec![0.0, 0.0, 0.0],
                    forward: vec![0.0, 0.0, -1.0],
                }
            }
            PerspectiveParam::Node { path } => {
                if let Some(node) = self.base().try_get_node_as::<Node3D>(path) {
                    let pos = node.get_global_position();
                    let rot = node.get_global_rotation_degrees();
                    let forward = node.get_global_transform().basis.col_c();
                    PerspectiveData {
                        position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                        rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
                        forward: vec![forward.x as f64, forward.y as f64, forward.z as f64],
                    }
                } else {
                    PerspectiveData {
                        position: vec![0.0, 0.0, 0.0],
                        rotation_deg: vec![0.0, 0.0, 0.0],
                        forward: vec![0.0, 0.0, -1.0],
                    }
                }
            }
            PerspectiveParam::Point { position } => {
                PerspectiveData {
                    position: position.clone(),
                    rotation_deg: vec![0.0, 0.0, 0.0],
                    forward: vec![0.0, 0.0, -1.0], // Default: north-aligned
                }
            }
        }
    }

    /// Recursively collect entity data from the scene tree.
    fn collect_entities_recursive(
        &self,
        node: &Gd<Node>,
        perspective: &PerspectiveData,
        params: &GetSnapshotDataParams,
        entities: &mut Vec<EntityData>,
    ) {
        // Skip Spectator's own nodes
        if self.is_spectator_node(node) {
            return;
        }

        // Only collect Node3D (spatial) nodes
        if let Some(node3d) = node.try_cast::<Node3D>() {
            let should_collect = self.should_collect(&node3d, params);

            if should_collect {
                let entity = self.collect_single_entity(&node3d, params);
                entities.push(entity);
            }
        }

        // Recurse into children
        let count = node.get_child_count();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                self.collect_entities_recursive(&child, perspective, params, entities);
            }
        }
    }

    /// Check if a node should be collected based on filters.
    fn should_collect(&self, node: &Gd<Node3D>, params: &GetSnapshotDataParams) -> bool {
        let class_name = node.get_class().to_string();

        // Apply class filter
        if !params.class_filter.is_empty() {
            if !params.class_filter.iter().any(|f| class_name == *f) {
                return false;
            }
        }

        // Apply group filter
        if !params.groups.is_empty() {
            let node_ref: Gd<Node> = node.clone().upcast();
            let has_matching_group = params.groups.iter().any(|g| {
                node_ref.is_in_group(g)
            });
            if !has_matching_group {
                return false;
            }
        }

        // Radius filter — compute distance from perspective
        if params.radius > 0.0 {
            let pos = node.get_global_position();
            let p = &params.perspective;
            let persp_pos = match p {
                PerspectiveParam::Camera | PerspectiveParam::Node { .. } => {
                    // We resolve this from the PerspectiveData, but we
                    // don't have it here. We need to pass it in.
                    // For now, distance check is done server-side.
                    return true;
                }
                PerspectiveParam::Point { position } => position,
            };
            // Note: distance filtering primarily handled server-side
            // after positions are collected. Addon sends all matching entities.
        }

        true
    }

    /// Collect data for a single entity.
    fn collect_single_entity(
        &self,
        node: &Gd<Node3D>,
        params: &GetSnapshotDataParams,
    ) -> EntityData {
        let pos = node.get_global_position();
        let rot = node.get_global_rotation_degrees();
        let class_name = node.get_class().to_string();

        let node_ref: Gd<Node> = node.clone().upcast();

        // Collect velocity (if available)
        let velocity = self.get_velocity(node);

        // Collect groups
        let groups = self.get_groups(&node_ref);

        // Check visibility (camera frustum check)
        let visible = node.is_visible_in_tree();

        // Collect exported state
        let state = self.get_exported_state(&node_ref);

        // Determine static classification
        let is_static_class = STATIC_CLASSES.iter().any(|c| class_name == *c);
        // MeshInstance3D is static only if it has no script
        let has_script = node_ref.get_script().is_nil().not();
        let is_static = is_static_class && !(class_name == "MeshInstance3D" && has_script);

        let mut entity = EntityData {
            path: self.get_relative_path(&node_ref),
            class: class_name,
            position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
            rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
            velocity,
            groups,
            visible,
            state,
            signals_recent: Vec::new(),
            children: Vec::new(),
            script: None,
            signals_connected: Vec::new(),
            physics: None,
            transform: None,
            all_exported_vars: None,
        };

        // Standard+ fields
        if params.detail != DetailLevel::Summary {
            // signals_recent collected when we have signal tracking (M4)
        }

        // Full fields
        if params.detail == DetailLevel::Full {
            entity.children = self.get_children(&node_ref);
            entity.script = self.get_script_path(&node_ref);
            entity.signals_connected = self.get_connected_signals(&node_ref);
            entity.physics = self.get_physics_data(node);
            entity.transform = Some(self.get_transform_data(node));
            entity.all_exported_vars = Some(self.get_all_exported_vars(&node_ref));
        }

        entity
    }

    /// Get the velocity of a node, if it's a physics body.
    fn get_velocity(&self, node: &Gd<Node3D>) -> Vec<f64> {
        if let Some(body) = node.try_cast::<CharacterBody3D>() {
            let v = body.get_velocity();
            return vec![v.x as f64, v.y as f64, v.z as f64];
        }
        if let Some(body) = node.try_cast::<RigidBody3D>() {
            let v = body.get_linear_velocity();
            return vec![v.x as f64, v.y as f64, v.z as f64];
        }
        vec![0.0, 0.0, 0.0]
    }

    /// Get all groups a node belongs to.
    fn get_groups(&self, node: &Gd<Node>) -> Vec<String> {
        let groups = node.get_groups();
        let mut result = Vec::new();
        for i in 0..groups.len() {
            let group = groups.get(i).to_string();
            // Filter out internal Godot groups (starting with _)
            if !group.starts_with('_') {
                result.push(group);
            }
        }
        result
    }

    /// Get exported variable state (user-defined @export vars).
    fn get_exported_state(&self, node: &Gd<Node>) -> serde_json::Map<String, serde_json::Value> {
        let mut state = serde_json::Map::new();
        let properties = node.get_property_list();

        for i in 0..properties.len() {
            let prop = properties.get(i);
            let usage = prop.get("usage").unwrap_or(Variant::from(0)).to::<i64>();
            let name = prop.get("name").unwrap_or(Variant::from("")).to::<GString>().to_string();

            // PROPERTY_USAGE_SCRIPT_VARIABLE (1 << 12 = 4096) = script-defined
            // PROPERTY_USAGE_EDITOR (1 << 2 = 4) = shown in editor (exported)
            // We want properties that are both script-defined AND editor-visible
            if usage & (4096 | 4) == (4096 | 4) {
                let value = node.get(name.clone());
                if let Some(json_value) = variant_to_json(&value) {
                    state.insert(name, json_value);
                }
            }
        }

        state
    }

    /// Get immediate children info.
    fn get_children(&self, node: &Gd<Node>) -> Vec<ChildData> {
        let count = node.get_child_count();
        let mut children = Vec::new();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                children.push(ChildData {
                    name: child.get_name().to_string(),
                    class: child.get_class().to_string(),
                });
            }
        }
        children
    }

    /// Get script path if a script is attached.
    fn get_script_path(&self, node: &Gd<Node>) -> Option<String> {
        let script = node.get_script();
        if script.is_nil() {
            return None;
        }
        // Try to get resource path
        let path_var = script.call("get_path", &[]);
        let path = path_var.to::<GString>().to_string();
        if path.is_empty() { None } else { Some(path) }
    }

    /// Get connected signal names.
    fn get_connected_signals(&self, node: &Gd<Node>) -> Vec<String> {
        let signals = node.get_signal_list();
        let mut result = Vec::new();
        for i in 0..signals.len() {
            let sig = signals.get(i);
            let name = sig.get("name").unwrap_or(Variant::from("")).to::<GString>().to_string();
            let connections = node.get_signal_connection_list(name.clone());
            if connections.len() > 0 {
                result.push(name);
            }
        }
        result
    }

    /// Get physics data for CharacterBody3D.
    fn get_physics_data(&self, node: &Gd<Node3D>) -> Option<PhysicsEntityData> {
        if let Some(body) = node.try_cast::<CharacterBody3D>() {
            let v = body.get_velocity();
            let on_floor = body.is_on_floor();
            let floor_normal = if on_floor {
                let n = body.get_floor_normal();
                Some(vec![n.x as f64, n.y as f64, n.z as f64])
            } else {
                None
            };
            // Access collision layer/mask via PhysicsBody3D
            let phys: Gd<PhysicsBody3D> = body.upcast();
            let layer = phys.get_collision_layer();
            let mask = phys.get_collision_mask();
            return Some(PhysicsEntityData {
                velocity: vec![v.x as f64, v.y as f64, v.z as f64],
                on_floor,
                floor_normal,
                collision_layer: layer,
                collision_mask: mask,
            });
        }
        None
    }

    /// Get full transform data.
    fn get_transform_data(&self, node: &Gd<Node3D>) -> TransformEntityData {
        let t = node.get_global_transform();
        let origin = t.origin;
        let basis = t.basis;
        let scale = node.get_scale();
        TransformEntityData {
            origin: vec![origin.x as f64, origin.y as f64, origin.z as f64],
            basis: vec![
                vec![basis.col_a().x as f64, basis.col_a().y as f64, basis.col_a().z as f64],
                vec![basis.col_b().x as f64, basis.col_b().y as f64, basis.col_b().z as f64],
                vec![basis.col_c().x as f64, basis.col_c().y as f64, basis.col_c().z as f64],
            ],
            scale: vec![scale.x as f64, scale.y as f64, scale.z as f64],
        }
    }

    /// Get all exported vars (full detail — no filtering).
    fn get_all_exported_vars(&self, node: &Gd<Node>) -> serde_json::Map<String, serde_json::Value> {
        // Same as get_exported_state but returns all script vars
        self.get_exported_state(node)
    }

    /// Get the relative path of a node from the current scene root.
    fn get_relative_path(&self, node: &Gd<Node>) -> String {
        if let Some(tree) = self.base().get_tree() {
            if let Some(root) = tree.get_current_scene() {
                let path = root.get_path_to(node);
                return path.to_string();
            }
        }
        node.get_name().to_string()
    }

    /// Check if a node is part of Spectator's own infrastructure.
    fn is_spectator_node(&self, node: &Gd<Node>) -> bool {
        let name = node.get_name().to_string();
        name == "SpectatorRuntime"
            || name.starts_with("SpectatorTCPServer")
            || name.starts_with("SpectatorCollector")
            || name.starts_with("SpectatorRecorder")
            || node.is_in_group("spectator_internal")
    }

    /// Get current frame info.
    pub fn get_frame_info(&self) -> FrameInfoResponse {
        let engine = Engine::singleton();
        let frame = engine.get_physics_frames() as u64;
        // Timestamp approximation from frame count and physics tick rate
        let ticks = engine.get_physics_ticks_per_second() as u64;
        let timestamp_ms = if ticks > 0 { (frame * 1000) / ticks } else { 0 };
        let delta = 1.0 / ticks as f64;
        FrameInfoResponse {
            frame,
            timestamp_ms,
            delta,
        }
    }
}

impl SnapshotResponse {
    pub fn empty() -> Self {
        Self {
            frame: 0,
            timestamp_ms: 0,
            perspective: PerspectiveData {
                position: vec![0.0, 0.0, 0.0],
                rotation_deg: vec![0.0, 0.0, 0.0],
                forward: vec![0.0, 0.0, -1.0],
            },
            entities: Vec::new(),
        }
    }
}

/// Convert a Godot Variant to a JSON value.
/// Returns None for types we can't meaningfully represent.
fn variant_to_json(v: &Variant) -> Option<serde_json::Value> {
    use godot::global::VariantType;
    match v.get_type() {
        VariantType::NIL => Some(serde_json::Value::Null),
        VariantType::BOOL => Some(serde_json::Value::Bool(v.to::<bool>())),
        VariantType::INT => Some(serde_json::json!(v.to::<i64>())),
        VariantType::FLOAT => {
            let f = v.to::<f64>();
            serde_json::Number::from_f64(f).map(serde_json::Value::Number)
        }
        VariantType::STRING | VariantType::STRING_NAME | VariantType::NODE_PATH => {
            Some(serde_json::Value::String(v.to::<GString>().to_string()))
        }
        VariantType::VECTOR2 => {
            let vec = v.to::<Vector2>();
            Some(serde_json::json!([vec.x, vec.y]))
        }
        VariantType::VECTOR3 => {
            let vec = v.to::<Vector3>();
            Some(serde_json::json!([vec.x, vec.y, vec.z]))
        }
        VariantType::COLOR => {
            let c = v.to::<Color>();
            Some(serde_json::json!([c.r, c.g, c.b, c.a]))
        }
        VariantType::ARRAY => {
            let arr = v.to::<Array<Variant>>();
            let items: Vec<serde_json::Value> = (0..arr.len())
                .filter_map(|i| variant_to_json(&arr.get(i)))
                .collect();
            Some(serde_json::Value::Array(items))
        }
        VariantType::DICTIONARY => {
            let dict = v.to::<Dictionary>();
            let mut map = serde_json::Map::new();
            for key in dict.keys_array().iter_shared() {
                let key_str = key.to::<GString>().to_string();
                if let Some(val) = variant_to_json(&dict.get(key.clone()).unwrap_or(Variant::nil())) {
                    map.insert(key_str, val);
                }
            }
            Some(serde_json::Value::Object(map))
        }
        _ => {
            // For unhandled types, convert to string representation
            Some(serde_json::Value::String(format!("{v}")))
        }
    }
}
```

**File:** `crates/spectator-godot/src/lib.rs` (modify)

```rust
use godot::prelude::*;

mod collector;
mod query_handler;
mod tcp_server;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
```

**Acceptance Criteria:**
- [ ] `SpectatorCollector` class is registered and available in Godot
- [ ] `collect_snapshot` returns entities from the active scene
- [ ] Camera perspective uses the active Camera3D position and forward vector
- [ ] Node perspective resolves to the specified node's position
- [ ] Point perspective uses raw coordinates with north-aligned forward
- [ ] Entities include: path, class, position, rotation, velocity, groups, visible, state
- [ ] Full detail adds: children, script, signals, physics, transform, all exported vars
- [ ] Groups filter: entities not in specified groups are excluded
- [ ] Class filter: entities not matching specified classes are excluded
- [ ] Spectator's own nodes are excluded from results
- [ ] Internal groups (starting with `_`) are excluded from entity groups
- [ ] `variant_to_json` handles nil, bool, int, float, string, vector2/3, color, array, dictionary
- [ ] Static class heuristic: StaticBody3D etc. classified as static; MeshInstance3D only if no script

---

### Unit 7: MCP `spatial_snapshot` Tool (`spectator-server`)

**File:** `crates/spectator-server/src/mcp/mod.rs` (new)

```rust
mod snapshot;

use crate::server::SpectatorServer;
use rmcp::tool_box;

#[tool_box]
impl SpectatorServer {}
```

**File:** `crates/spectator-server/src/server.rs` (modify)

Remove the manual `ServerHandler` impl and let `#[tool_box]` generate tool dispatch:

```rust
use rmcp::handler::server::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tcp::SessionState;

#[derive(Clone)]
pub struct SpectatorServer {
    pub state: Arc<Mutex<SessionState>>,
}

impl SpectatorServer {
    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self { state }
    }
}

impl ServerHandler for SpectatorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "spectator-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
```

**File:** `crates/spectator-server/src/mcp/snapshot.rs` (new)

```rust
use crate::server::SpectatorServer;
use crate::tcp::query_addon;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use spectator_core::{
    bearing,
    budget::{BudgetEnforcer, BudgetReport, SnapshotBudgetDefaults, resolve_budget},
    cluster::{self, Cluster, ClusterStrategy},
    types::{Perspective, Position3, RelativePosition},
};
use spectator_protocol::query::{
    DetailLevel, EntityData, GetSnapshotDataParams, PerspectiveParam, SnapshotResponse,
};

/// Parameters for the spatial_snapshot MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialSnapshotParams {
    /// Perspective type: "camera", "node", or "point". Default: "camera".
    #[serde(default = "default_perspective")]
    pub perspective: String,

    /// Node path, required when perspective is "node".
    pub focal_node: Option<String>,

    /// World position [x, y, z], required when perspective is "point".
    pub focal_point: Option<Vec<f64>>,

    /// Max distance from perspective to include. Default: 50.0.
    #[serde(default = "default_radius")]
    pub radius: f64,

    /// Detail tier: "summary", "standard", or "full". Default: "standard".
    #[serde(default = "default_detail")]
    pub detail: String,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by node class.
    pub class_filter: Option<Vec<String>>,

    /// Include nodes outside camera frustum. Default: false.
    #[serde(default)]
    pub include_offscreen: bool,

    /// Soft token budget override.
    pub token_budget: Option<u32>,

    /// Pagination cursor from a previous truncated response.
    pub cursor: Option<String>,

    /// Expand a cluster from a previous summary response.
    pub expand: Option<String>,
}

fn default_perspective() -> String { "camera".to_string() }
fn default_radius() -> f64 { 50.0 }
fn default_detail() -> String { "standard".to_string() }

/// Processed entity for MCP output.
#[derive(Debug, Serialize)]
struct OutputEntity {
    path: String,
    class: String,
    rel: RelativePosition,
    abs: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rot_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    velocity: Option<Vec<f64>>,
    groups: Vec<String>,
    state: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    signals_recent: Vec<SignalEntry>,
    // Full-only fields
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    physics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signals_connected: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    all_exported_vars: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct SignalEntry {
    signal: String,
    frame: u64,
}

#[derive(Debug, Serialize)]
struct PaginationBlock {
    truncated: bool,
    showing: usize,
    total: usize,
    cursor: String,
    omitted_nearest_dist: f64,
}

#[derive(Debug, Serialize)]
struct StaticSummary {
    count: usize,
    categories: serde_json::Map<String, serde_json::Value>,
}

impl SpectatorServer {
    #[tool(description = "Get a spatial snapshot of the current scene from a perspective. Use detail 'summary' for a cheap overview (~200 tokens), 'standard' for per-entity data (~400-800 tokens), or 'full' for everything including transforms, physics, and children (~1000+ tokens). Start with summary, then drill down.")]
    pub async fn spatial_snapshot(
        &self,
        #[tool(aggr)] params: SpatialSnapshotParams,
    ) -> Result<String, McpError> {
        // 1. Parse detail level
        let detail = parse_detail(&params.detail)?;

        // 2. Build perspective param for addon query
        let perspective_param = build_perspective_param(&params)?;

        // 3. Query addon for raw data
        let query_params = GetSnapshotDataParams {
            perspective: perspective_param,
            radius: params.radius,
            include_offscreen: params.include_offscreen,
            groups: params.groups.clone().unwrap_or_default(),
            class_filter: params.class_filter.clone().unwrap_or_default(),
            detail,
        };

        let raw_data: SnapshotResponse = {
            let data = query_addon(
                &self.state,
                "get_snapshot_data",
                serde_json::to_value(&query_params).map_err(|e| {
                    McpError::internal_error(format!("Param serialization error: {e}"), None)
                })?,
            )
            .await?;
            serde_json::from_value(data).map_err(|e| {
                McpError::internal_error(format!("Response deserialization error: {e}"), None)
            })?
        };

        // 4. Build perspective for spatial calculations
        let persp = build_perspective(&raw_data.perspective);

        // 5. Compute relative positions and filter by radius/visibility
        let mut entities_with_rel: Vec<(EntityData, RelativePosition)> = raw_data
            .entities
            .into_iter()
            .filter_map(|e| {
                let pos: Position3 = [
                    e.position[0],
                    e.position[1],
                    e.position[2],
                ];
                let rel = bearing::relative_position(&persp, pos, !e.visible);
                // Filter by radius
                if rel.dist > params.radius {
                    return None;
                }
                // Filter offscreen
                if !params.include_offscreen && !e.visible {
                    return None;
                }
                Some((e, rel))
            })
            .collect();

        // 6. Sort by distance (nearest first)
        entities_with_rel.sort_by(|a, b| {
            a.1.dist.partial_cmp(&b.1.dist).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 7. Resolve budget
        let tier_default = match detail {
            DetailLevel::Summary => SnapshotBudgetDefaults::SUMMARY,
            DetailLevel::Standard => SnapshotBudgetDefaults::STANDARD,
            DetailLevel::Full => SnapshotBudgetDefaults::FULL,
        };
        let hard_cap = SnapshotBudgetDefaults::HARD_CAP;
        let budget_limit = resolve_budget(params.token_budget, tier_default, hard_cap);

        // 8. Handle expand (drill into a cluster from summary)
        if let Some(ref cluster_label) = params.expand {
            return self.handle_expand(
                &entities_with_rel,
                cluster_label,
                &persp,
                &raw_data,
                budget_limit,
                hard_cap,
            );
        }

        // 9. Build response based on detail level
        let response = match detail {
            DetailLevel::Summary => {
                build_summary_response(
                    &raw_data, &entities_with_rel, &persp, budget_limit, hard_cap,
                )
            }
            DetailLevel::Standard => {
                build_standard_response(
                    &raw_data, &entities_with_rel, &persp, budget_limit, hard_cap,
                )
            }
            DetailLevel::Full => {
                build_full_response(
                    &raw_data, &entities_with_rel, &persp, budget_limit, hard_cap,
                )
            }
        };

        serde_json::to_string(&response).map_err(|e| {
            McpError::internal_error(format!("Response serialization error: {e}"), None)
        })
    }

    fn handle_expand(
        &self,
        entities: &[(EntityData, RelativePosition)],
        cluster_label: &str,
        perspective: &Perspective,
        raw: &SnapshotResponse,
        budget_limit: u32,
        hard_cap: u32,
    ) -> Result<String, McpError> {
        // Filter entities belonging to the specified cluster (by first group)
        let matching: Vec<&(EntityData, RelativePosition)> = entities
            .iter()
            .filter(|(e, _)| {
                e.groups.first().map(|g| g.as_str()) == Some(cluster_label)
                    || (cluster_label == "other" && e.groups.is_empty())
            })
            .collect();

        if matching.is_empty() {
            return Err(McpError::invalid_params(
                format!("No cluster named '{cluster_label}' found. Use spatial_snapshot(detail: 'summary') to see available clusters."),
                None,
            ));
        }

        // Build standard-detail response for these entities
        let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
        let mut output_entities = Vec::new();

        for (entity, rel) in &matching {
            let out = build_output_entity(entity, rel, false);
            let bytes = serde_json::to_vec(&out).unwrap_or_default();
            if !enforcer.try_add(bytes.len()) {
                break;
            }
            output_entities.push(out);
        }

        let response = serde_json::json!({
            "frame": raw.frame,
            "timestamp_ms": raw.timestamp_ms,
            "expand": cluster_label,
            "entities": output_entities,
            "budget": enforcer.report(),
        });

        serde_json::to_string(&response).map_err(|e| {
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })
    }
}

fn parse_detail(s: &str) -> Result<DetailLevel, McpError> {
    match s {
        "summary" => Ok(DetailLevel::Summary),
        "standard" => Ok(DetailLevel::Standard),
        "full" => Ok(DetailLevel::Full),
        _ => Err(McpError::invalid_params(
            format!("Invalid detail level '{s}'. Must be 'summary', 'standard', or 'full'."),
            None,
        )),
    }
}

fn build_perspective_param(params: &SpatialSnapshotParams) -> Result<PerspectiveParam, McpError> {
    match params.perspective.as_str() {
        "camera" => Ok(PerspectiveParam::Camera),
        "node" => {
            let path = params.focal_node.as_ref().ok_or_else(|| {
                McpError::invalid_params(
                    "focal_node is required when perspective is 'node'",
                    None,
                )
            })?;
            Ok(PerspectiveParam::Node { path: path.clone() })
        }
        "point" => {
            let pos = params.focal_point.as_ref().ok_or_else(|| {
                McpError::invalid_params(
                    "focal_point is required when perspective is 'point'",
                    None,
                )
            })?;
            Ok(PerspectiveParam::Point { position: pos.clone() })
        }
        other => Err(McpError::invalid_params(
            format!("Invalid perspective '{other}'. Must be 'camera', 'node', or 'point'."),
            None,
        )),
    }
}

fn build_perspective(data: &spectator_protocol::query::PerspectiveData) -> Perspective {
    let position: Position3 = [data.position[0], data.position[1], data.position[2]];
    let forward = [data.forward[0], data.forward[1], data.forward[2]];
    let (facing, facing_deg) = bearing::compass_bearing(forward);
    Perspective {
        position,
        forward,
        facing,
        facing_deg,
    }
}

fn build_output_entity(
    entity: &EntityData,
    rel: &RelativePosition,
    full: bool,
) -> OutputEntity {
    let velocity = if entity.velocity.iter().any(|v| v.abs() > 0.01) {
        Some(entity.velocity.clone())
    } else {
        None
    };

    OutputEntity {
        path: entity.path.clone(),
        class: entity.class.clone(),
        rel: rel.clone(),
        abs: entity.position.clone(),
        rot_y: Some(entity.rotation_deg.get(1).copied().unwrap_or(0.0)),
        velocity,
        groups: entity.groups.clone(),
        state: entity.state.clone(),
        signals_recent: entity.signals_recent.iter().map(|s| SignalEntry {
            signal: s.signal.clone(),
            frame: s.frame,
        }).collect(),
        transform: if full { entity.transform.as_ref().map(|t| serde_json::to_value(t).unwrap_or_default()) } else { None },
        physics: if full { entity.physics.as_ref().map(|p| serde_json::to_value(p).unwrap_or_default()) } else { None },
        children: if full { Some(serde_json::to_value(&entity.children).unwrap_or_default()) } else { None },
        script: if full { entity.script.clone() } else { None },
        signals_connected: if full { Some(entity.signals_connected.clone()) } else { None },
        all_exported_vars: if full { entity.all_exported_vars.clone() } else { None },
    }
}

fn build_summary_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
) -> serde_json::Value {
    // Convert to core types for clustering
    let clusters = cluster::cluster_by_group(entities);

    let total = entities.len();
    let visible = entities.iter().filter(|(e, _)| e.visible).count();

    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);

    // Estimate overhead
    let overhead = 200; // perspective + frame + metadata
    enforcer.try_add(overhead);

    // Add clusters within budget
    let mut output_clusters = Vec::new();
    for c in &clusters {
        let bytes = serde_json::to_vec(c).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            break;
        }
        output_clusters.push(c);
    }

    serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "perspective": {
            "position": raw.perspective.position,
            "facing": perspective.facing,
            "facing_deg": perspective.facing_deg,
        },
        "clusters": output_clusters,
        "total_nodes_tracked": total,
        "total_nodes_visible": visible,
        "budget": enforcer.report(),
    })
}

fn build_standard_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
) -> serde_json::Value {
    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
    enforcer.try_add(200); // overhead

    let mut dynamic_entities = Vec::new();
    let mut static_count = 0;
    let mut static_categories: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let total = entities.len();

    for (entity, rel) in entities {
        // Check if static
        if is_static_class(&entity.class) {
            static_count += 1;
            let cat = classify_static_category(&entity.class);
            let counter = static_categories
                .entry(cat)
                .or_insert(serde_json::json!(0));
            if let Some(n) = counter.as_u64() {
                *counter = serde_json::json!(n + 1);
            }
            continue;
        }

        let out = build_output_entity(entity, rel, false);
        let bytes = serde_json::to_vec(&out).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            // Truncated — add pagination
            let pagination = PaginationBlock {
                truncated: true,
                showing: dynamic_entities.len(),
                total,
                cursor: format!("snap_{}_p{}", raw.frame, dynamic_entities.len()),
                omitted_nearest_dist: rel.dist,
            };
            return serde_json::json!({
                "frame": raw.frame,
                "timestamp_ms": raw.timestamp_ms,
                "perspective": {
                    "position": raw.perspective.position,
                    "facing": perspective.facing,
                    "facing_deg": perspective.facing_deg,
                },
                "entities": dynamic_entities,
                "static_summary": { "count": static_count, "categories": static_categories },
                "pagination": pagination,
                "budget": enforcer.report(),
            });
        }
        dynamic_entities.push(out);
    }

    serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "perspective": {
            "position": raw.perspective.position,
            "facing": perspective.facing,
            "facing_deg": perspective.facing_deg,
        },
        "entities": dynamic_entities,
        "static_summary": { "count": static_count, "categories": static_categories },
        "budget": enforcer.report(),
    })
}

fn build_full_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
) -> serde_json::Value {
    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
    enforcer.try_add(200);

    let mut dynamic_entities = Vec::new();
    let mut static_nodes = Vec::new();
    let total = entities.len();

    for (entity, rel) in entities {
        if is_static_class(&entity.class) {
            let node = serde_json::json!({
                "path": entity.path,
                "class": entity.class,
                "pos": entity.position,
            });
            let bytes = serde_json::to_vec(&node).unwrap_or_default();
            if enforcer.try_add(bytes.len()) {
                static_nodes.push(node);
            }
            continue;
        }

        let out = build_output_entity(entity, rel, true);
        let bytes = serde_json::to_vec(&out).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            let pagination = PaginationBlock {
                truncated: true,
                showing: dynamic_entities.len(),
                total,
                cursor: format!("snap_{}_p{}", raw.frame, dynamic_entities.len()),
                omitted_nearest_dist: rel.dist,
            };
            return serde_json::json!({
                "frame": raw.frame,
                "timestamp_ms": raw.timestamp_ms,
                "perspective": {
                    "position": raw.perspective.position,
                    "facing": perspective.facing,
                    "facing_deg": perspective.facing_deg,
                },
                "entities": dynamic_entities,
                "static_nodes": static_nodes,
                "pagination": pagination,
                "budget": enforcer.report(),
            });
        }
        dynamic_entities.push(out);
    }

    serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "perspective": {
            "position": raw.perspective.position,
            "facing": perspective.facing,
            "facing_deg": perspective.facing_deg,
        },
        "entities": dynamic_entities,
        "static_nodes": static_nodes,
        "budget": enforcer.report(),
    })
}

fn is_static_class(class: &str) -> bool {
    matches!(
        class,
        "StaticBody3D" | "StaticBody2D" | "CSGShape3D" | "CSGBox3D"
        | "CSGCylinder3D" | "CSGMesh3D" | "CSGPolygon3D" | "CSGSphere3D"
        | "CSGTorus3D" | "CSGCombiner3D" | "GridMap"
        | "WorldEnvironment" | "DirectionalLight3D" | "OmniLight3D" | "SpotLight3D"
    )
}

fn classify_static_category(class: &str) -> String {
    match class {
        "StaticBody3D" | "StaticBody2D" => "collision".to_string(),
        c if c.starts_with("CSG") => "csg".to_string(),
        "GridMap" => "gridmap".to_string(),
        "WorldEnvironment" => "environment".to_string(),
        c if c.contains("Light") => "lights".to_string(),
        _ => "other".to_string(),
    }
}
```

**File:** `crates/spectator-server/src/main.rs` (modify — add `mod mcp`)

```rust
mod mcp;
mod server;
mod tcp;
// ... rest unchanged
```

**Acceptance Criteria:**
- [ ] Agent calling `spatial_snapshot(detail: "summary")` via MCP receives a JSON response with clusters, frame, perspective, budget
- [ ] Agent calling `spatial_snapshot(detail: "standard")` receives per-entity data with `rel` (bearing, distance, elevation), `abs` position, groups, state
- [ ] Agent calling `spatial_snapshot(detail: "full")` receives entities with transforms, physics, children, script, all vars, plus static_nodes listing
- [ ] `perspective: "camera"` uses active Camera3D
- [ ] `perspective: "node"` with `focal_node` uses that node's position/orientation
- [ ] `perspective: "point"` with `focal_point` uses raw coordinates
- [ ] `groups` filter limits results to matching group
- [ ] `class_filter` limits results to matching class
- [ ] `include_offscreen: false` excludes non-visible nodes
- [ ] `radius` excludes entities beyond the radius
- [ ] Entities sorted by distance (nearest first)
- [ ] Token budget respected — response truncated when budget exceeded
- [ ] Pagination block present when truncated, with cursor
- [ ] `expand: "enemies"` drills into a cluster by label
- [ ] Error for invalid detail, invalid perspective, missing required params
- [ ] `not_connected` error when addon is not connected

---

### Unit 8: MCP Configuration (`.mcp.json`)

**File:** `.mcp.json` (project root — example for Claude Code)

```json
{
  "mcpServers": {
    "spectator": {
      "command": "cargo",
      "args": ["run", "-p", "spectator-server"],
      "env": {
        "SPECTATOR_PORT": "9077"
      }
    }
  }
}
```

**Acceptance Criteria:**
- [ ] Claude Code loads spectator-server as an MCP server using this config
- [ ] `spatial_snapshot` tool appears in the agent's tool list
- [ ] Agent can call the tool and receive a response (when game is running)

---

## Implementation Order

1. **Unit 1: Core types** — no dependencies, pure Rust types
2. **Unit 2: Bearing calculation** — depends on Unit 1 types
3. **Unit 3: Token budget** — depends on nothing, pure logic
4. **Unit 4: Clustering engine** — depends on Units 1, 2
5. **Unit 5a: Protocol query types** — extends spectator-protocol
6. **Unit 5b: Server TCP query dispatch** — depends on 5a, modifies server tcp.rs
7. **Unit 5c: Addon query handler** — depends on 5a, modifies addon
8. **Unit 6: Scene collector** — depends on 5a (protocol types), core GDExtension work
9. **Unit 7: MCP snapshot tool** — depends on all above, final integration
10. **Unit 8: MCP config** — trivial, do last

Units 1-4 can be done in parallel (all in spectator-core, independent of each other).
Units 5a-5c are sequential (protocol first, then server dispatch, then addon handler).
Unit 6 depends on 5a (needs protocol types).
Unit 7 ties everything together.

---

## Testing

### Unit Tests: `crates/spectator-core/src/bearing.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_3d() { ... }

    #[test]
    fn cardinal_boundaries() {
        // Test all 8 directions at exact centers and boundaries
    }

    #[test]
    fn elevation_above_below_level() { ... }

    #[test]
    fn bearing_ahead_when_aligned() { ... }

    #[test]
    fn bearing_right_when_perpendicular() { ... }

    #[test]
    fn godot_coordinate_convention() {
        // Verify Y-up, -Z forward at yaw=0
    }
}
```

### Unit Tests: `crates/spectator-core/src/budget.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn estimate_tokens_from_bytes() { ... }

    #[test]
    fn resolve_budget_defaults() { ... }

    #[test]
    fn resolve_budget_clamped() { ... }

    #[test]
    fn enforcer_tracks_budget() { ... }

    #[test]
    fn enforcer_rejects_when_exceeded() { ... }
}
```

### Unit Tests: `crates/spectator-core/src/cluster.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn cluster_by_group_basic() { ... }

    #[test]
    fn ungrouped_entities_in_other() { ... }

    #[test]
    fn static_entities_separate_cluster() { ... }

    #[test]
    fn cluster_summary_generation() { ... }
}
```

### Unit Tests: `crates/spectator-protocol/src/query.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn snapshot_params_round_trip() { ... }

    #[test]
    fn perspective_param_tagged_enum() { ... }

    #[test]
    fn entity_data_optional_fields() { ... }
}
```

### Integration Test: `crates/spectator-server/tests/snapshot_integration.rs`

Mock the TCP connection (no real Godot) and verify the full MCP → TCP → response pipeline:

```rust
// Simulate addon responses for get_snapshot_data
// Verify MCP response shape matches CONTRACT.md
// Test budget truncation
// Test error handling (not connected, invalid params)
```

---

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify MCP tool is registered (manual — requires AI client)
# 1. Start Godot with addon enabled
# 2. Press Play
# 3. Start spectator-server (or use .mcp.json)
# 4. Call spatial_snapshot(detail: "summary")
# 5. Verify response has: frame, perspective, clusters, budget
# 6. Call spatial_snapshot(detail: "standard")
# 7. Verify response has: entities with rel (dist, bearing), abs position
# 8. Call spatial_snapshot(expand: "enemies")
# 9. Verify response has: entities from only that cluster
```
