# Spectator — Technical Specification

## System Overview

Spectator is a two-component system: a **Rust MCP server** (spectator-server) and a **Rust GDExtension + GDScript addon** (spectator-godot) running inside Godot. They communicate over TCP using a length-prefixed JSON protocol. The MCP server exposes 9 tools to AI agents via the Model Context Protocol. The addon observes the running game's scene tree and responds to queries.

```
┌──────────────┐   stdio (MCP)   ┌──────────────────┐   TCP (:9077)   ┌──────────────────┐
│  AI Agent    │ ◄─────────────► │ spectator-server  │ ──────────────► │  Godot Engine    │
│  (any MCP    │                 │                    │                 │                  │
│   client)    │                 │  Semantic Layer:   │                 │  GDExtension:    │
│              │                 │  - Spatial Index   │                 │  - Collector     │
│              │                 │  - Delta Engine    │                 │  - TCP Server    │
│              │                 │  - Token Budget    │                 │  - Recorder      │
│              │                 │  - Watch Engine    │                 │                  │
│              │                 │  - Recording Mgmt  │                 │  GDScript:       │
│              │                 │  - Bearing Calc    │                 │  - EditorPlugin  │
│              │                 │  - Clustering      │                 │  - Runtime AL    │
└──────────────┘                 └──────────────────┘                 │  - Dock UI       │
                                                                      └──────────────────┘
```

### Responsibility Split

| Concern | Where | Why |
|---|---|---|
| Scene tree traversal | GDExtension (Godot process) | Must access Godot's scene tree API directly |
| Property collection | GDExtension | Same — direct API access required |
| Physics queries (raycast) | GDExtension | Requires physics server access |
| Frame capture (recording) | GDExtension | Must run every _physics_process |
| TCP server (listen + respond) | GDExtension | Persistent listener inside Godot |
| Input handling (F8/F9/F10) | GDScript autoload | _shortcut_input, thin glue |
| Editor dock UI | GDScript + .tscn | Required — GDExtension can't be EditorPlugin base |
| Autoload registration | GDScript EditorPlugin | _enable_plugin / _disable_plugin lifecycle |
| Spatial indexing (R-tree/grid) | spectator-server | Computational, no Godot API needed |
| Bearing calculation | spectator-server | Pure math on coordinates |
| Delta computation | spectator-server | Diffing snapshots, tracking state |
| Clustering (summary view) | spectator-server | Algorithmic grouping of nodes |
| Token budget enforcement | spectator-server | Response shaping before MCP output |
| Pagination / cursor management | spectator-server | Stateful, tied to MCP session |
| Watch condition evaluation | spectator-server | Pattern matching on incoming data |
| Recording analysis (query_range, diff) | spectator-server | Temporal queries over SQLite |
| MCP tool routing | spectator-server | MCP SDK integration |
| Configuration management | spectator-server + addon | Server owns session config, addon owns defaults |

**Design principle:** The addon is deliberately **thin and dumb**. It answers "what does the engine say right now?" The server is **thick and smart** — it computes spatial relationships, manages state, shapes responses, and handles all MCP concerns. This keeps the GDExtension simple and pushes complexity into pure Rust where it's easier to test.

---

## TCP Wire Protocol

### Transport

TCP over localhost, default port 9077. The GDExtension addon listens (is the TCP server). The MCP server connects (is the TCP client). One connection at a time (1:1).

Binding: `127.0.0.1` only. No remote connections.

### Message Framing

Length-prefixed JSON. Each message is:

```
[4 bytes: payload length, big-endian u32][JSON payload, UTF-8]
```

Maximum message size: 16 MiB (safety valve). Messages exceeding this are rejected.

### Handshake

Immediately after TCP connection, the addon sends a handshake message (unsolicited):

```jsonc
{
  "type": "handshake",
  "spectator_version": "0.1.0",
  "protocol_version": 1,
  "godot_version": "4.3",
  "scene_dimensions": 3,           // 2, 3, or "mixed"
  "physics_ticks_per_sec": 60,
  "project_name": "my_game"
}
```

The server responds with an ACK:

```jsonc
{
  "type": "handshake_ack",
  "spectator_version": "0.1.0",
  "protocol_version": 1,
  "session_id": "sess_a1b2c3"
}
```

If `protocol_version` doesn't match, the server sends an error and disconnects:

```jsonc
{
  "type": "handshake_error",
  "message": "Protocol version mismatch: server supports v1, addon sent v2",
  "server_version": "0.1.0",
  "supported_protocols": [1]
}
```

### Request/Response (Server → Addon)

```jsonc
// Server sends a query
{
  "id": "req_001",
  "type": "query",
  "method": "get_visible_nodes",
  "params": {
    "perspective": "camera",
    "include_offscreen": false
  }
}

// Addon responds
{
  "id": "req_001",
  "type": "response",
  "data": {
    "nodes": [
      { "path": "enemies/scout_02", "class": "CharacterBody3D", "position": [12.4, 0.0, -8.2] }
    ]
  }
}
```

### Error Responses (Addon → Server)

```jsonc
{
  "id": "req_001",
  "type": "error",
  "code": "node_not_found",
  "message": "Node 'enemies/scout_99' does not exist in the scene tree"
}
```

### Push Events (Addon → Server, unsolicited)

For watched signals and subscriptions:

```jsonc
{
  "type": "event",
  "event": "signal_emitted",
  "node": "enemies/scout_02",
  "signal": "health_changed",
  "args": [15],
  "frame": 2900
}
```

### Addon Query Methods

The server queries the addon using a flat method namespace. The addon is responsible for executing these against the Godot scene tree and returning raw data. The server handles all semantic processing.

| Method | Parameters | Returns |
|---|---|---|
| `get_scene_tree` | `depth`, `root`, `include` | Tree structure (paths, classes, groups, optionally scripts) |
| `get_visible_nodes` | `perspective` | Nodes in camera frustum/viewport rect |
| `get_near` | `position`, `radius`, `groups`, `class_filter` | Nodes within radius |
| `get_node_state` | `path`, `properties` | Property values for a node |
| `get_node_transform` | `path` | Transform, velocity, physics state |
| `get_node_resources` | `path` | Mesh, materials, animations, collision shapes |
| `get_children` | `path` | Immediate children (name, class) |
| `get_ancestors` | `path` | Parent chain to root |
| `find_nodes` | `by`, `value` | Matching nodes (path, class, groups) |
| `raycast` | `from`, `to`, `collision_mask` | Hit result or clear |
| `get_nav_path` | `from`, `to` | Navigation path points and distance |
| `get_signals` | `path` | Connected signals and recent emissions |
| `get_frame_info` | — | Current frame, delta, engine time |
| `get_dimensions` | — | 2D, 3D, or mixed |
| `teleport_node` | `path`, `position`, `rotation_deg` | Ack + previous position |
| `set_node_property` | `path`, `property`, `value` | Ack + previous value |
| `call_node_method` | `path`, `method`, `args` | Return value |
| `emit_node_signal` | `path`, `signal`, `args` | Ack |
| `pause_tree` | `paused` | Ack + current state |
| `advance_physics` | `frames` | Ack + new frame number |
| `spawn_node` | `scene_path`, `parent`, `name`, `position` | New node path |
| `remove_node` | `path` | Ack |
| `subscribe_signal` | `path`, `signal` | Ack (events pushed async) |
| `unsubscribe_signal` | `path`, `signal` | Ack |
| `recording_start` | `config` | Recording ID |
| `recording_stop` | — | Metadata (frames, duration, nodes) |
| `recording_frame` | `index` | Frame snapshot data |
| `recording_query` | `from`, `to`, `condition` | Matching frames |
| `recording_marker` | `action`, `label`, `frame` | Marker data |
| `recording_list` | — | Saved recordings metadata |
| `recording_delete` | `id` | Ack |
| `eval_expression` | `expression`, `node_context` | Result value |

---

## Connection Lifecycle

```
1. Godot game starts → addon autoload initializes
   → GDExtension SpectatorTCPServer.start(port) called
   → TCP server listens on 127.0.0.1:9077

2. AI client spawns spectator-server → MCP server starts
   → Connects to 127.0.0.1:9077
   → Receives handshake from addon
   → Sends handshake ACK
   → Session active

3. Session active:
   → Agent makes MCP tool calls
   → Server translates to TCP queries
   → Addon responds with raw data
   → Server processes and returns MCP responses
   → Addon pushes events for watched signals

4. Game stops (Play → Stop in editor):
   → TCP connection drops
   → Server enters "reconnecting" state
   → All MCP tool calls return { error: { code: "not_connected" } }
   → Server retries connection every 2 seconds
   → Session state (watches, config) preserved in memory

5. Game starts again (Play):
   → Addon reopens TCP listener
   → Server reconnects
   → New handshake exchange
   → Server re-sends watches and config
   → Session resumes

6. MCP session ends (Claude Code closes):
   → Server process exits
   → TCP connection drops
   → Addon continues listening (ready for next server)
   → No game state affected
```

---

## Spatial Index

### 3D: R-Tree (rstar)

The server maintains an R-tree spatial index built from entity positions received from the addon. This enables efficient nearest-neighbor and radius queries without re-querying the addon.

The index is rebuilt on every snapshot response (the server already has all positions). For delta responses, the index is updated incrementally (moved nodes re-inserted).

**Performance:** rstar handles 10,000 entities with sub-millisecond query times. Spectator scenes rarely exceed 500 dynamic entities.

### 2D: Grid Hash

For 2D scenes, a flat grid hash with configurable cell size (default 32 units). Faster than R-tree for uniform 2D distributions and simpler to implement.

### Index Invalidation

The spatial index is invalidated when:
- A new snapshot is taken (full rebuild)
- The game scene changes (full rebuild)
- Reconnection occurs (full rebuild)

Between snapshots, the index is updated via delta data (moved/entered/exited nodes).

---

## Delta Engine

The delta engine tracks state changes between queries. It maintains:

1. **Last snapshot**: Complete entity state from the most recent snapshot
2. **Per-entity history**: Position + state at the time of the last query
3. **Event buffer**: Signal emissions, node enters/exits since last query

When `spatial_delta` is called:

```
1. Query addon for current state of tracked nodes
2. Diff against stored last-snapshot state
3. Categorize changes: moved, state_changed, entered, exited, signals_emitted
4. Apply watch conditions (fire triggers if conditions met)
5. Apply token budget (truncate if needed)
6. Update stored state for next delta
7. Return diff to agent
```

### Change Detection Thresholds

To avoid noise, small changes are suppressed:
- Position: movement < 0.01 units is ignored
- Rotation: change < 0.1 degrees is ignored
- Floating-point state properties: change < 0.001 is ignored (configurable)

---

## Token Budget System

### Estimation

Token count is estimated from the JSON response size. Approximate formula:

```
tokens ≈ json_bytes / 4
```

This is a rough estimate (actual tokenization varies by model), but sufficient for budget management. The goal is order-of-magnitude control, not exact token counting.

### Budget Application

1. Entities are sorted by distance from perspective (nearest first)
2. Each entity is serialized at the requested detail level
3. Token estimate is accumulated
4. When budget is reached, remaining entities are truncated
5. Pagination metadata is added if truncated

### Detail Level Token Estimates

| Entity Type | Summary | Standard | Full |
|---|---|---|---|
| Dynamic entity | ~15 tokens (in cluster) | ~80 tokens | ~200 tokens |
| Static entity | ~5 tokens (in category count) | ~5 tokens (in count) | ~30 tokens |
| Perspective header | ~40 tokens | ~40 tokens | ~40 tokens |
| Pagination block | ~30 tokens | ~30 tokens | ~30 tokens |

### Default Budgets

| Tool | Detail | Default | Hard Cap |
|---|---|---|---|
| spatial_snapshot | summary | 500 | 5000 |
| spatial_snapshot | standard | 1500 | 5000 |
| spatial_snapshot | full | 3000 | 5000 |
| spatial_delta | — | 1000 | 5000 |
| spatial_query | — | 500 | 2000 |
| spatial_inspect | — | 1500 | 3000 |
| spatial_watch | — | 200 | 500 |
| spatial_config | — | 200 | 200 |
| scene_tree | — | 1500 | 5000 |
| recording | — | 1500 | 5000 |

---

## Bearing System

All relative positions are computed by the server from raw transforms provided by the addon.

### Cardinal Bearings (8-direction)

Relative to the perspective entity's facing direction (forward vector):

```
              ahead (0°)
         ahead_left    ahead_right
       (315°)              (45°)
  left (270°)          right (90°)
       behind_left    behind_right
       (225°)             (135°)
              behind (180°)
```

Each bearing maps to a 45° arc centered on its direction. "ahead" = facing direction ± 22.5°.

### Bearing Computation

```
1. Get perspective entity's forward vector (from rotation/facing)
2. Compute vector from perspective to target
3. Project both onto the XZ plane (3D) or XY plane (2D)
4. Compute signed angle between forward and to-target vectors
5. Map to 8-direction cardinal
6. Also report exact degrees (0° = ahead, clockwise)
```

### Elevation (3D only)

```
above_Nm    — target > 2m higher (N = rounded meters)
level       — within ± 2m
below_Nm    — target > 2m lower
```

Threshold (2m) is configurable via spatial_config.

### Relative Position Block

```jsonc
{
  "dist": 7.2,              // straight-line distance in world units
  "bearing": "ahead_left",  // 8-direction cardinal
  "bearing_deg": 322,       // exact degrees (0 = ahead, clockwise)
  "elevation": "level",     // 3D only
  "occluded": false          // camera line-of-sight check
}
```

---

## Clustering Engine

Used for summary-tier snapshots. Groups entities into labeled clusters to reduce token usage.

### Group-Based Clustering (Default)

Entities are grouped by their Godot group membership. An entity in multiple groups is assigned to the first matching group in priority order (configurable). Ungrouped entities go into an "other" cluster.

### Class-Based Clustering

Entities are grouped by their Godot class name (CharacterBody3D, Area3D, RigidBody3D, etc.).

### Proximity-Based Clustering

Entities are grouped by spatial proximity using a simple algorithm:
1. Sort entities by distance from perspective
2. Seed clusters from the nearest entities
3. Assign each remaining entity to the nearest cluster center within a merge threshold
4. Clusters exceeding a size limit are split

### Cluster Summary Generation

Each cluster produces:
```jsonc
{
  "label": "enemies",
  "count": 3,
  "nearest": { "node": "enemy/scout_02", "dist": 7.2, "bearing": "ahead_left" },
  "farthest_dist": 22.1,
  "summary": "2 idle, 1 patrol"
}
```

The `summary` field is generated by examining common state properties across cluster members. If all members have an `alert_level` or `state` property, the server counts distinct values and produces a natural-language summary.

---

## Recording System

### Storage: SQLite

Recordings are stored as SQLite databases. Each recording is a single `.sqlite` file in `user://spectator_recordings/`. The MCP server manages the SQLite connection (reads for analysis queries). The addon captures frame data and sends it to the server for storage, or writes directly during recording.

### Schema

```sql
-- Recording metadata
CREATE TABLE recording (
    id TEXT PRIMARY KEY,
    name TEXT,
    started_at_frame INTEGER,
    ended_at_frame INTEGER,
    started_at_ms INTEGER,
    ended_at_ms INTEGER,
    scene_dimensions INTEGER,       -- 2 or 3
    physics_ticks_per_sec INTEGER,
    capture_config TEXT              -- JSON: nodes, groups, interval, etc.
);

-- Frame snapshots (one row per captured frame)
CREATE TABLE frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    data BLOB                       -- MessagePack-encoded snapshot (compact)
);

-- Events (signals, collisions, area enter/exit)
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER,
    event_type TEXT,                 -- signal, collision, area_enter, area_exit, node_added, node_removed
    node_path TEXT,
    data TEXT,                       -- JSON: signal name, args, related nodes
    FOREIGN KEY (frame) REFERENCES frames(frame)
);

-- Markers (human, agent, system)
CREATE TABLE markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER,
    timestamp_ms INTEGER,
    source TEXT,                     -- human, agent, system
    label TEXT,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);

-- Indexes for temporal queries
CREATE INDEX idx_events_frame ON events(frame);
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_node ON events(node_path);
CREATE INDEX idx_markers_frame ON markers(frame);
```

### Frame Data Encoding

Frame snapshot data is stored as MessagePack (not JSON) for compactness. A typical frame with 50 tracked nodes produces ~2-5 KB of MessagePack data. At 60fps over 30 seconds (1800 frames), that's 3.6-9 MB — well within SQLite's capabilities.

### Write Strategy

During recording:
1. Addon captures frame data every N physics ticks (configurable, default 1)
2. Frame data is serialized to MessagePack in the GDExtension
3. Sent to the MCP server via TCP push message (or buffered locally)
4. Server batches inserts into SQLite using WAL mode (non-blocking reads during writes)
5. Events and markers are inserted as they occur

### Query Strategy

Recording queries (`snapshot_at`, `query_range`, `diff_frames`, `find_event`) are SQL queries against the SQLite database. The server deserializes frame data from MessagePack on demand.

```sql
-- snapshot_at: get frame data at a specific frame
SELECT data FROM frames WHERE frame = ?;

-- query_range with proximity condition: scan frames in range
SELECT frame, timestamp_ms, data FROM frames
WHERE frame BETWEEN ? AND ?
ORDER BY frame;
-- (proximity check done in Rust after deserializing each frame)

-- find_event: search events
SELECT * FROM events
WHERE event_type = ? AND node_path LIKE ?
ORDER BY frame;
```

For complex conditions (proximity, velocity spike), the server scans the frame range and evaluates conditions in Rust — SQL is used for range selection, Rust for spatial logic.

---

## Node Classification

The server classifies nodes as **static** or **dynamic** to optimize data transmission and token usage.

### Classification Rules

1. **Explicit static patterns**: Nodes matching `spatial_config.static_patterns` globs are static
2. **Class-based heuristics**: `StaticBody3D`, `StaticBody2D`, `CSGShape3D`, `MeshInstance3D` (without script) are static candidates
3. **Observation-based**: Nodes that haven't moved or changed state across multiple snapshots are promoted to static
4. **Override**: Any node in a tracked group or with active watches is always dynamic

### Static Node Optimization

Static nodes are:
- Collected once (first snapshot or on scene change)
- Cached on the server
- Not re-transmitted in subsequent responses (unless changed)
- Summarized as counts + categories in summary/standard detail
- Fully listed only at "full" detail level
- Tracked for changes (the `static_changed` flag in delta responses)

---

## 2D / 3D Adaptation

### Detection

On handshake, the addon reports `scene_dimensions`. The server configures its spatial index and bearing system accordingly.

### Differences

| Aspect | 3D | 2D |
|---|---|---|
| Position format | `[x, y, z]` | `[x, y]` |
| Rotation | `rot_y` (yaw degrees) | `rot` (angle degrees) |
| Full rotation | Euler `[x, y, z]` | Single angle |
| Bearing elevation | `"above"` / `"level"` / `"below"` | Not present |
| Spatial index | R-tree (rstar) | Grid hash |
| Frustum check | Camera3D frustum | Camera2D viewport rect |
| Raycast | PhysicsRayQueryParameters3D | PhysicsRayQueryParameters2D |
| Physics | `on_floor`, `floor_normal` vector | `on_floor`, `on_wall`, `on_ceiling` |
| Transform | `Transform3D` (origin + basis) | `Transform2D` (origin + angle) |

### What Doesn't Change

The MCP tool interfaces, parameter names, and response structure keys are identical. The differences are in the *values* within those structures (array lengths, field presence). An agent doesn't need to know whether it's debugging 2D or 3D — the tools work the same way.

---

## Performance Targets

| Metric | Target | Rationale |
|---|---|---|
| MCP tool call round-trip | < 100ms | Agent shouldn't wait noticeably |
| TCP query round-trip | < 50ms | Localhost, minimal serialization |
| Scene tree collection (100 nodes) | < 2ms | GDExtension, main thread budget |
| Scene tree collection (500 nodes) | < 10ms | GDExtension, still within frame |
| Spatial index rebuild | < 1ms | rstar, typical scene sizes |
| Recording frame capture | < 1ms | Must fit in _physics_process |
| Recording SQLite write (batch) | < 5ms | WAL mode, batch 60 frames |
| Token budget estimation | < 0.1ms | Simple arithmetic |

At 60fps, the physics tick budget is ~16ms. Spectator's per-frame overhead should stay under 3ms (collection + recording + TCP poll), leaving 13ms+ for the actual game.

---

## Security Considerations

### Local-Only by Default

TCP binds to `127.0.0.1`. No remote connections without explicit configuration change.

### No Authentication

For local development debugging, no auth is needed. The attack surface is: a local process on the same machine can connect to the addon's TCP port and query/modify the game state. This is acceptable — any local process already has full access to the machine.

### Action Safety

`spatial_action` operations are debugging tools, not gameplay automation:
- `call_method` and `eval_expression` can execute arbitrary code — same trust level as the Godot editor's expression evaluator
- `set_property` can change any node property — same trust level as the Inspector panel
- `remove_node` can delete nodes — same trust level as the Scene panel

These are all things the developer can already do in the editor. Spectator just makes them available to the agent.

### Recording Privacy

Recordings capture game state (node paths, property values, positions). They are stored locally. No data is sent to external services. The recordings are as private as the game project itself.
