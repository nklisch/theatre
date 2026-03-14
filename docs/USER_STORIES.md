# Spectator — User Stories

Stories are organized by epic. Each story has a priority level:

- **P0** — Core. Spectator is not usable without this.
- **P1** — Important. Required for a complete v1 experience.
- **P2** — Valuable. Enhances the experience, can ship after initial release.

Within each epic, stories are ordered by dependency (earlier stories enable later ones).

---

## Epic 1: Connection & Setup

The foundational infrastructure — getting Spectator running and connected.

### S1.1: GDExtension Addon Loads in Godot Editor
**Priority:** P0
**As a** Godot developer,
**I want to** copy the `addons/spectator/` folder into my project and enable it in Project Settings → Plugins,
**So that** the Spectator addon initializes without errors.

**Acceptance Criteria:**
- plugin.cfg is valid and recognized by Godot 4.5+
- Enabling the plugin in Project Settings shows no errors in the Output panel
- The EditorPlugin registers the SpectatorRuntime autoload
- The GDExtension binary loads and SpectatorCollector, SpectatorTCPServer, SpectatorRecorder classes are available
- Disabling the plugin cleanly removes the autoload and frees resources

### S1.2: TCP Server Starts on Game Play
**Priority:** P0
**As a** Godot developer,
**I want** the addon to start a TCP server on a configurable port when I press Play in the editor,
**So that** the MCP server can connect to the running game.

**Acceptance Criteria:**
- TCP server listens on port 9077 (default) when the game starts running
- Port is configurable via Project Settings or config file
- If the port is already in use, the addon logs a clear error message and does not crash
- TCP server stops when the game stops
- Server binds to localhost only (127.0.0.1)

### S1.3: MCP Server Connects to Addon
**Priority:** P0
**As an** AI agent (via MCP client),
**I want** the spectator-server process to connect to the Godot addon over TCP,
**So that** I can query the running game's state.

**Acceptance Criteria:**
- spectator-server connects to localhost:9077 on startup
- TCP handshake exchanges version, Godot version, scene dimensions (2D/3D), project name
- Protocol version mismatch produces a clear error
- If addon is not available, server retries every 2 seconds
- Connection status is available via MCP (any tool call returns `not_connected` error until connected)

### S1.4: Reconnection After Disconnect
**Priority:** P1
**As an** AI agent,
**I want** the connection to recover automatically when the game is stopped and restarted,
**So that** I don't have to restart my MCP session.

**Acceptance Criteria:**
- When the game stops (TCP connection drops), the server enters "reconnecting" state
- Server retries connection every 2 seconds
- When the game starts again, connection re-establishes
- Session state (watches, config) is preserved and re-sent on reconnect
- All tool calls during disconnection return `{ error: { code: "not_connected" } }`

### S1.5: MCP Server Configuration for AI Clients
**Priority:** P0
**As a** developer,
**I want** clear instructions and config snippets for adding spectator-server to my AI client (Claude Code, Cursor, etc.),
**So that** my AI agent can discover and use Spectator's tools.

**Acceptance Criteria:**
- README includes config snippets for Claude Code (`.mcp.json` format)
- Config specifies `type: "stdio"`, command path to `spectator-server` binary
- Agent skill file (`skills/spectator.md`) is distributed and documented
- After configuration, the AI client lists all 9 Spectator tools as available

---

## Epic 2: Spatial Observation

The core value — giving the agent eyes into the game world.

### S2.1: Spatial Snapshot — Summary
**Priority:** P0
**As an** AI agent,
**I want to** get a token-efficient summary of the current scene from a spatial perspective,
**So that** I can understand what's in the game world without spending many tokens.

**Acceptance Criteria:**
- `spatial_snapshot(detail: "summary")` returns clustered overview (~200-300 tokens)
- Response includes: frame number, timestamp, perspective position/facing
- Nodes are clustered by group (default) with count, nearest node, farthest distance, and text summary
- Recent events (signals, node enters/exits) are listed
- Total tracked and visible node counts are included
- Budget accounting shows tokens used vs. limit

### S2.2: Spatial Snapshot — Standard
**Priority:** P0
**As an** AI agent,
**I want to** get individual entity data for dynamic nodes in the scene,
**So that** I can see positions, states, and spatial relationships for each node.

**Acceptance Criteria:**
- `spatial_snapshot(detail: "standard")` returns per-entity data (~400-800 tokens)
- Each entity includes: path, class, relative position (distance + bearing + elevation), absolute position, rotation, velocity (if moving), groups, state (exported vars), recent signals
- Static nodes are summarized (count + categories), not listed individually
- Entities are sorted by distance from perspective (nearest first)

### S2.3: Spatial Snapshot — Full
**Priority:** P1
**As an** AI agent,
**I want** complete data for all nodes including transforms, physics, children, scripts, and static geometry,
**So that** I can do deep investigation when needed.

**Acceptance Criteria:**
- `spatial_snapshot(detail: "full")` returns comprehensive per-entity data (~1000+ tokens)
- Adds: full transform (origin, basis, scale), physics state (velocity, on_floor, collision layers), children list, script path, all exported vars, connected signals
- Static nodes get individual listings (path, position, AABB)

### S2.4: Perspective Options
**Priority:** P1
**As an** AI agent,
**I want to** view the scene from the active camera, a specific node, or an arbitrary point,
**So that** I can understand spatial relationships from different viewpoints.

**Acceptance Criteria:**
- `perspective: "camera"` uses the active Camera3D/Camera2D position and orientation
- `perspective: "node"` with `focal_node: "enemies/scout_02"` uses that node's position and facing
- `perspective: "point"` with `focal_point: [10, 0, 5]` uses a raw world position (no facing — north-aligned)
- All bearings and distances are relative to the chosen perspective

### S2.5: Spatial Filtering
**Priority:** P1
**As an** AI agent,
**I want to** filter snapshots by group, class, radius, and visibility,
**So that** I can focus on relevant nodes and reduce token usage.

**Acceptance Criteria:**
- `groups: ["enemies"]` returns only nodes in the "enemies" group
- `class_filter: ["CharacterBody3D"]` returns only CharacterBody3D nodes
- `radius: 20.0` limits results to nodes within 20 units of perspective
- `include_offscreen: false` (default) excludes nodes outside camera frustum/viewport
- `include_offscreen: true` includes all nodes within radius regardless of visibility
- Filters combine (AND logic): `groups: ["enemies"], radius: 10` = enemies within 10 units

### S2.6: Token Budget & Pagination
**Priority:** P1
**As an** AI agent,
**I want** response size to be controlled by a token budget with pagination for large scenes,
**So that** I don't overflow my context window.

**Acceptance Criteria:**
- Each response includes a `budget` block: `{ used, limit, hard_cap }`
- `token_budget` parameter overrides the default budget (up to hard cap)
- When truncated, response includes `pagination`: `{ truncated, showing, total, cursor, omitted_nearest_dist }`
- Passing `cursor` back returns the next page with consistent frame data
- Hard cap (default 5000) is enforced regardless of requested budget
- Entities are included nearest-first, so truncation drops the farthest nodes

### S2.7: Cluster Expansion
**Priority:** P1
**As an** AI agent,
**I want to** expand a cluster from a summary response into full entity detail,
**So that** I can drill into a specific group without re-fetching the entire scene.

**Acceptance Criteria:**
- `spatial_snapshot(expand: "enemies")` returns standard/full detail for only that cluster's nodes
- Works with cluster labels from a prior summary response
- Does not re-send perspective/frame metadata already seen in the summary

---

## Epic 3: Spatial Queries

Targeted spatial questions without fetching the full scene.

### S3.1: Nearest Query
**Priority:** P0
**As an** AI agent,
**I want to** find the K nearest nodes to a point or node,
**So that** I can answer "what's near X?" efficiently.

**Acceptance Criteria:**
- `spatial_query(query_type: "nearest", from: "player", k: 5)` returns the 5 closest nodes
- Results include path, distance, bearing, class
- Supports `from` as a node path or `[x, y, z]` coordinate
- Respects `groups` and `class_filter` parameters

### S3.2: Radius Query
**Priority:** P1
**As an** AI agent,
**I want to** find all nodes within a given radius of a point or node,
**So that** I can answer "what's in this area?"

**Acceptance Criteria:**
- `spatial_query(query_type: "radius", from: "player", radius: 15.0)` returns all nodes within 15 units
- Same response format as nearest query
- Respects filters

### S3.3: Raycast Query
**Priority:** P1
**As an** AI agent,
**I want to** check line of sight between two points or nodes,
**So that** I can determine visibility and obstructions.

**Acceptance Criteria:**
- `spatial_query(query_type: "raycast", from: "enemies/scout_02", to: "player")` performs a physics raycast
- Response includes: clear (bool), blocked_by (node path), blocked_at (position), total_distance, clear_distance
- Works in both 2D and 3D (uses appropriate physics query parameters)

### S3.4: Relationship Query
**Priority:** P1
**As an** AI agent,
**I want to** understand the spatial relationship between two specific nodes,
**So that** I can reason about their positions relative to each other.

**Acceptance Criteria:**
- `spatial_query(query_type: "relationship", from: "enemies/scout_02", to: "player")` returns mutual spatial info
- Response includes: distance, bearing from A to B, bearing from B to A, elevation difference, line of sight, navmesh distance (if available), shared groups

### S3.5: Path Distance Query
**Priority:** P2
**As an** AI agent,
**I want to** know the navigation mesh path distance between two nodes,
**So that** I can understand traversal cost vs. straight-line distance.

**Acceptance Criteria:**
- `spatial_query(query_type: "path_distance", from: "enemies/scout_02", to: "player")` returns nav path info
- Response includes: nav_distance, straight_distance, path_ratio, waypoint count, traversable (bool)
- Graceful response if no NavigationServer is active ("navmesh not available")

### S3.6: Area Query
**Priority:** P2
**As an** AI agent,
**I want to** find all nodes within a bounding box or sphere,
**So that** I can examine a specific region of the scene.

**Acceptance Criteria:**
- `spatial_query(query_type: "area", from: [0, 0, 0], radius: 10)` returns nodes in a sphere
- Supports AABB specification for box queries
- Respects filters

---

## Epic 4: Deep Inspection

Single-node deep dive.

### S4.1: Full Node Inspection
**Priority:** P0
**As an** AI agent,
**I want** comprehensive data about a single node — transform, physics, state, children, signals, script, and spatial context,
**So that** I can diagnose issues with a specific node.

**Acceptance Criteria:**
- `spatial_inspect(node: "enemies/scout_02")` returns all data categories
- Transform: global/local origin, rotation, scale
- Physics: velocity, speed, on_floor/wall/ceiling, collision layers/masks, floor normal
- State: all exported variables, optionally internal variables
- Children: immediate children with name, class, key properties
- Signals: connected signals (with targets), recent emissions (with frame + args)
- Script: path, base class, method list, inheritance chain
- Spatial context: nearby entities (path, dist, bearing, group), areas the node is in, nearest navmesh edge, camera visibility

### S4.2: Selective Inspection
**Priority:** P1
**As an** AI agent,
**I want to** request only specific data categories for a node,
**So that** I can reduce token usage when I only need certain information.

**Acceptance Criteria:**
- `include: ["physics", "state"]` returns only physics and state data for the node
- All 7 categories (transform, physics, state, children, signals, script, spatial_context) can be individually selected
- Default is all categories

### S4.3: Resource Inspection
**Priority:** P2
**As an** AI agent,
**I want to** see loaded resource information for a node (mesh, materials, animations, collision shapes, shader params),
**So that** I can diagnose visual, animation, and collision issues.

**Acceptance Criteria:**
- `include: ["resources"]` returns resource data
- Covers: mesh (resource path, type, surface count), material overrides, collision shape (type, dimensions), animation player state (current animation, available anims, position, looping), navigation agent config, shader parameters
- Distinguishes inline resources from file-based resources

---

## Epic 5: Change Detection

Watching the game evolve over time.

### S5.1: Spatial Delta
**Priority:** P0
**As an** AI agent,
**I want to** see what changed since my last query,
**So that** I can track the effects of actions or game progression.

**Acceptance Criteria:**
- `spatial_delta()` returns changes since last snapshot/delta
- Categories: moved (position + delta), state_changed (old → new), entered (new nodes), exited (removed nodes with reason), signals_emitted (signal + args + frame)
- `since_frame` parameter allows diffing against a specific frame
- `static_changed` flag indicates if any static geometry moved (rare, notable)
- Respects same perspective/radius/filter options as snapshot

### S5.2: Watch Subscriptions
**Priority:** P1
**As an** AI agent,
**I want to** subscribe to changes on specific nodes or groups with optional conditions,
**So that** I get notified when something interesting happens without polling.

**Acceptance Criteria:**
- `spatial_watch(action: "add", watch: { node: "enemies/scout_02", track: ["all"] })` returns a watch_id
- Conditional watches: `conditions: [{ property: "health", operator: "lt", value: 20 }]` only fires when health < 20
- Watch triggers appear in `spatial_delta` responses under `watch_triggers`
- `action: "list"` shows all active watches
- `action: "remove"` removes a specific watch by ID
- `action: "clear"` removes all watches
- Watches survive reconnection (re-sent by server on reconnect)

### S5.3: Group Watches
**Priority:** P1
**As an** AI agent,
**I want to** watch an entire group (e.g., "enemies") instead of individual nodes,
**So that** I can monitor a category of nodes without knowing their paths in advance.

**Acceptance Criteria:**
- `watch: { node: "group:enemies", track: ["position", "state"] }` watches all nodes in the "enemies" group
- New nodes joining the group are automatically included
- Nodes leaving the group are automatically excluded

---

## Epic 6: Debugging Actions

Manipulating the game to reproduce and test.

### S6.1: Pause and Resume
**Priority:** P0
**As an** AI agent,
**I want to** pause and unpause the scene tree,
**So that** I can freeze the game to inspect state without it changing under me.

**Acceptance Criteria:**
- `spatial_action(action: "pause", paused: true)` pauses the scene tree
- `spatial_action(action: "pause", paused: false)` resumes
- While paused, spatial queries still work (reading frozen state)
- Response confirms the action with the current pause state

### S6.2: Frame Advance
**Priority:** P1
**As an** AI agent,
**I want to** step the physics simulation forward by N frames or N seconds while paused,
**So that** I can observe frame-by-frame behavior.

**Acceptance Criteria:**
- `spatial_action(action: "advance_frames", frames: 5)` steps 5 physics frames
- `spatial_action(action: "advance_time", seconds: 0.5)` steps ~30 frames at 60fps
- Only works while paused (returns error if not paused)
- Response includes the new frame number

### S6.3: Teleport Node
**Priority:** P0
**As an** AI agent,
**I want to** move a node to a specific world position,
**So that** I can set up debugging scenarios (e.g., place enemy near wall to test collision).

**Acceptance Criteria:**
- `spatial_action(action: "teleport", node: "enemies/scout_02", position: [5, 0, -3])` moves the node
- Optional `rotation_deg` sets yaw (3D) or angle (2D)
- Response includes previous and new position
- Works for any Node3D/Node2D

### S6.4: Set Property
**Priority:** P0
**As an** AI agent,
**I want to** change an exported variable on a node,
**So that** I can test different values without editing code.

**Acceptance Criteria:**
- `spatial_action(action: "set_property", node: "enemies/scout_02", property: "health", value: 10)` changes the property
- Response includes previous and new values
- Works for exported variables and built-in properties (visible, process_mode, etc.)
- Returns error if property doesn't exist

### S6.5: Call Method
**Priority:** P1
**As an** AI agent,
**I want to** call a method on a node,
**So that** I can trigger behavior (e.g., `take_damage(50)`) for debugging.

**Acceptance Criteria:**
- `spatial_action(action: "call_method", node: "enemies/scout_02", method: "take_damage", method_args: [50])` calls the method
- Response includes the return value (if any)
- Returns error if method doesn't exist

### S6.6: Emit Signal
**Priority:** P1
**As an** AI agent,
**I want to** emit a signal on a node,
**So that** I can test signal-driven behavior without triggering the original condition.

**Acceptance Criteria:**
- `spatial_action(action: "emit_signal", node: "enemies/scout_02", signal: "health_changed", args: [10])` emits the signal
- Connected receivers fire normally
- Response confirms the signal was emitted

### S6.7: Spawn and Remove Nodes
**Priority:** P2
**As an** AI agent,
**I want to** instantiate a scene at a position or remove a node,
**So that** I can set up test scenarios.

**Acceptance Criteria:**
- `spatial_action(action: "spawn_node", scene_path: "res://enemies/scout.tscn", parent: "enemies", name: "test_scout", position: [10, 0, 0])` instantiates the scene
- `spatial_action(action: "remove_node", node: "enemies/test_scout")` queue_frees the node
- Spawn response includes the new node path
- Remove response confirms the node was freed

### S6.8: Return Delta After Action
**Priority:** P1
**As an** AI agent,
**I want** an action to optionally return a spatial delta showing what changed,
**So that** I can do "teleport and show me what happened" in one round-trip.

**Acceptance Criteria:**
- `return_delta: true` on any action causes the response to include an inline `delta` block
- Delta covers the frame(s) affected by the action
- Saves a separate `spatial_delta` call

---

## Epic 7: Scene Tree Navigation

Understanding the node hierarchy (non-spatial).

### S7.1: Scene Tree Roots
**Priority:** P0
**As an** AI agent,
**I want to** see the top-level nodes of the scene tree,
**So that** I can understand the scene's organizational structure.

**Acceptance Criteria:**
- `scene_tree(action: "roots")` returns root-level nodes with class and groups
- Includes the main scene root and any autoloads

### S7.2: Children and Subtree
**Priority:** P0
**As an** AI agent,
**I want to** list children of a node or recursively traverse a subtree,
**So that** I can navigate the scene hierarchy.

**Acceptance Criteria:**
- `scene_tree(action: "children", node: "enemies")` returns immediate children
- `scene_tree(action: "subtree", node: "enemies", depth: 3)` returns recursive tree up to depth 3
- Each node includes class and groups (default), optionally script, visible, process_mode
- Depth limit prevents unbounded responses; at limit, shows `"...": "depth_limit_reached"`

### S7.3: Ancestors
**Priority:** P1
**As an** AI agent,
**I want to** see the parent chain from a node to the root,
**So that** I can understand where a node sits in the hierarchy.

**Acceptance Criteria:**
- `scene_tree(action: "ancestors", node: "enemies/scout_02/NavAgent")` returns the chain: NavAgent → scout_02 → enemies → root
- Each ancestor includes class and groups

### S7.4: Find Nodes
**Priority:** P0
**As an** AI agent,
**I want to** search the scene tree by name, class, group, or script,
**So that** I can find nodes without knowing their exact path.

**Acceptance Criteria:**
- `scene_tree(action: "find", find_by: "class", find_value: "CharacterBody3D")` returns all matching nodes
- Search by: name (substring match), class (exact), group (membership), script (resource path)
- Results include path, class, groups

---

## Epic 8: Recording & Playback

Human-drives, agent-analyzes workflow.

### S8.1: Start and Stop Recording
**Priority:** P1
**As a** Godot developer (via editor dock) or AI agent (via MCP),
**I want to** start and stop recording a spatial timeline of the running game,
**So that** the agent can later analyze what happened.

**Acceptance Criteria:**
- `recording(action: "start", recording_name: "wall_clip_repro")` begins frame capture
- `recording(action: "stop")` ends capture, returns metadata (frames captured, duration, nodes tracked)
- Human can start/stop via dock button or F8 keyboard shortcut
- Only one recording active at a time (starting while recording returns error)
- Configurable capture: specific nodes/groups, property filter, capture interval, signal capture, input capture, max frames safety valve

### S8.2: Markers
**Priority:** P1
**As a** Godot developer,
**I want to** drop timestamped markers during recording (via F9 or dock button) with optional text notes,
**So that** the agent knows when and where interesting things happened.

**Acceptance Criteria:**
- F9 in-game drops a marker at the current frame
- Dock button allows marker with text note
- Agent can add markers via `recording(action: "add_marker", marker_label: "...", marker_frame: N)`
- System auto-generates markers for anomalies (velocity spikes, collision events)
- `recording(action: "markers")` lists all markers with frame, time, source (human/agent/system), label

### S8.3: Snapshot at Frame
**Priority:** P1
**As an** AI agent,
**I want to** get the spatial state at a specific recorded frame,
**So that** I can see exactly what the scene looked like at any point in the timeline.

**Acceptance Criteria:**
- `recording(action: "snapshot_at", at_frame: 4575)` returns the same shape as `spatial_snapshot`
- `at_time_ms` as an alternative to `at_frame`
- Same detail/budget controls as live snapshots
- Uses the most recent recording by default, or a specific `recording_id`

### S8.4: Temporal Query (Query Range)
**Priority:** P1
**As an** AI agent,
**I want to** search across a range of recorded frames for specific conditions,
**So that** I can find when a bug occurred without manually scrubbing frame by frame.

**Acceptance Criteria:**
- `recording(action: "query_range", from_frame: 4570, to_frame: 4590, node: "enemies/guard_01", condition: { type: "proximity", target: "walls/*", threshold: 0.5 })` finds frames where the condition is met
- Condition types: proximity, property_change, signal_emitted, entered_area, velocity_spike, state_transition
- Results include: matching frames with timestamp, relevant data (distance, position, velocity), and annotations (first_breach, deepest_penetration)

### S8.5: Diff Frames
**Priority:** P1
**As an** AI agent,
**I want to** compare spatial state between two recorded frames,
**So that** I can see exactly what changed between two points in time.

**Acceptance Criteria:**
- `recording(action: "diff_frames", frame_a: 3010, frame_b: 3020)` returns changes
- Shows: position changes (old/new + delta), state changes (old/new), markers between the frames
- Reports unchanged node count

### S8.6: Find Events in Recording
**Priority:** P2
**As an** AI agent,
**I want to** search the recording timeline for specific event types,
**So that** I can find all signal emissions, collisions, or node changes.

**Acceptance Criteria:**
- `recording(action: "find_event", event_type: "signal", event_filter: "health_changed")` finds matching events
- Event types: signal, property_change, collision, area_enter, area_exit, node_added, node_removed, marker, input

### S8.7: Recording Management
**Priority:** P1
**As a** developer or AI agent,
**I want to** list, select, and delete recordings,
**So that** I can manage my recording library.

**Acceptance Criteria:**
- `recording(action: "list")` returns all saved recordings with name, duration, date, size
- `recording(action: "status")` shows if recording is active, frame count, duration
- `recording(action: "delete", recording_id: "rec_001")` removes a recording
- Recordings persist to disk (SQLite in `user://spectator_recordings/`)

---

## Epic 9: Configuration

Tuning Spectator for the project and debugging task.

### S9.1: Static Node Classification
**Priority:** P1
**As an** AI agent,
**I want to** tell Spectator which nodes are static (walls, terrain, props) so they're treated differently,
**So that** static geometry doesn't waste tokens in every response.

**Acceptance Criteria:**
- `spatial_config(static_patterns: ["walls/*", "terrain/*", "props/*"])` marks matching nodes as static
- Static nodes are summarized (count + categories) in standard detail, not listed individually
- Static nodes get full listing only at "full" detail level
- Static node data is cached and not re-transmitted unless changed

### S9.2: State Property Configuration
**Priority:** P1
**As an** AI agent,
**I want to** specify which properties to include in state snapshots per group or class,
**So that** I see relevant game state without irrelevant noise.

**Acceptance Criteria:**
- `state_properties: { "enemies": ["health", "alert_level"], "CharacterBody3D": ["velocity"], "*": ["visible"] }` configures property inclusion
- Wildcard `*` applies to all nodes
- Properties appear in entity `state` blocks in snapshots and deltas
- Unconfigured nodes show all exported variables (default behavior)

### S9.3: Clustering Configuration
**Priority:** P2
**As an** AI agent,
**I want to** control how nodes are clustered in summary views,
**So that** clusters make sense for my game's structure.

**Acceptance Criteria:**
- `cluster_by: "group"` clusters by Godot group membership (default)
- `cluster_by: "class"` clusters by node class
- `cluster_by: "proximity"` clusters by spatial proximity (auto-detected clusters)
- `cluster_by: "none"` disables clustering

### S9.4: Display Preferences
**Priority:** P2
**As an** AI agent or developer,
**I want to** configure bearing format and internal variable exposure,
**So that** responses match my debugging needs.

**Acceptance Criteria:**
- `bearing_format: "cardinal"` shows only "ahead_left" style bearings
- `bearing_format: "degrees"` shows only numeric degrees
- `bearing_format: "both"` shows both (default)
- `expose_internals: true` includes non-exported (underscore-prefixed) variables in state

### S9.5: Token Hard Cap Configuration
**Priority:** P1
**As a** developer,
**I want to** set the maximum token budget for any single response,
**So that** I can prevent context window blowouts.

**Acceptance Criteria:**
- `token_hard_cap: 3000` sets the server-enforced ceiling
- Default is 5000
- Agent's `token_budget` parameter is clamped to this value

### S9.6: Configuration via Project Settings
**Priority:** P1
**As a** Godot developer,
**I want to** configure Spectator's default settings in Godot's Project Settings,
**So that** I don't have to reconfigure via the agent every session.

**Acceptance Criteria:**
- Project Settings → Spectator section with: port, static patterns, default properties, hard cap
- These serve as defaults; `spatial_config` tool calls override them per-session
- Settings persist across editor restarts (saved in project.godot)

### S9.7: Configuration via File
**Priority:** P2
**As a** developer,
**I want to** configure Spectator via a `spectator.toml` file in my project root,
**So that** I can version-control my Spectator configuration.

**Acceptance Criteria:**
- `spectator.toml` in project root is read by both the addon and the MCP server
- Contains: port, static patterns, state properties, cluster_by, token_hard_cap
- Overrides Project Settings values
- `spatial_config` tool calls override file values per-session

---

## Epic 10: Editor Dock UI

The human-facing interface in the Godot editor.

### S10.1: Connection Status Display
**Priority:** P1
**As a** Godot developer,
**I want to** see whether the MCP server is connected in the editor dock,
**So that** I know if my AI agent has access to the game.

**Acceptance Criteria:**
- Green indicator when MCP server is connected
- Red indicator when disconnected
- Port number displayed
- Updates in real-time

### S10.2: Recording Controls
**Priority:** P1
**As a** Godot developer,
**I want** Record, Stop, and Marker buttons in the dock,
**So that** I can control recording sessions without keyboard shortcuts.

**Acceptance Criteria:**
- Record button (red circle icon) starts a recording session, prompts for optional name
- Stop button ends the recording
- Marker button drops a marker with optional text input
- Timer shows elapsed time and frame count during recording
- Buttons are disabled/enabled based on state (can't stop if not recording)

### S10.3: Active Session Info
**Priority:** P1
**As a** Godot developer,
**I want to** see what's currently being tracked in the dock,
**So that** I understand what data is available to the agent.

**Acceptance Criteria:**
- Shows: nodes being tracked (count + groups), active watches (from agent), current frame number, memory usage estimate for recording buffer

### S10.4: Recording Library
**Priority:** P2
**As a** Godot developer,
**I want** a list of saved recordings in the dock,
**So that** I can manage and review past sessions.

**Acceptance Criteria:**
- List shows: recording name, duration, date, size
- Click to select for agent review
- Delete button with confirmation
- Sorted by date (most recent first)

### S10.5: Agent Activity Feed
**Priority:** P2
**As a** Godot developer,
**I want to** see what the agent is doing in real-time,
**So that** I can follow along and build trust in the collaboration.

**Acceptance Criteria:**
- Shows recent agent actions: "Agent inspecting scout_02...", "Agent watching enemies group..."
- Shows agent-initiated actions: "Agent teleported scout_02 to [5, 0, -3]"
- Scrolling log with timestamps
- Can be collapsed/hidden

### S10.6: Agent Action Notifications
**Priority:** P1
**As a** Godot developer,
**I want** in-engine notifications when the agent takes actions (teleport, set property, pause),
**So that** I'm not surprised by changes to the game state.

**Acceptance Criteria:**
- Toast/notification appears in the game viewport when agent uses `spatial_action`
- Shows: action type, node affected, key details (e.g., "Teleported scout_02 to [5, 0, -3]")
- Notifications are brief (auto-dismiss after 3 seconds) and non-blocking
- Can be disabled in settings

---

## Epic 11: 2D Support

Full 2D scene debugging.

### S11.1: 2D Scene Detection
**Priority:** P1
**As an** AI agent,
**I want** Spectator to automatically detect whether the scene is 2D or 3D,
**So that** spatial data uses the correct coordinate system.

**Acceptance Criteria:**
- Handshake includes `scene_dimensions: 2` or `scene_dimensions: 3`
- Detection based on scene root type (Node2D vs Node3D)
- Mixed scenes report `"mixed"` and include both coordinate systems

### S11.2: 2D Coordinate Adaptation
**Priority:** P1
**As an** AI agent,
**I want** all positions, bearings, and transforms to use 2D conventions in 2D scenes,
**So that** the data makes sense for a 2D game.

**Acceptance Criteria:**
- Positions are `[x, y]` arrays (not `[x, y, z]`)
- Bearings use 8-direction compass without elevation
- Transforms use `Transform2D` (single angle rotation, no basis matrix unless needed)
- Velocity is `[x, y]`
- `rot` instead of `rot_y` for rotation

### S11.3: 2D Viewport and Queries
**Priority:** P1
**As an** AI agent,
**I want** camera frustum checks to use the Camera2D viewport rect in 2D scenes, and raycasts to use 2D physics,
**So that** visibility and spatial queries work correctly.

**Acceptance Criteria:**
- "Visible" means within the Camera2D's viewport rectangle
- Raycasts use `PhysicsRayQueryParameters2D`
- Spatial indexing uses 2D grid hash (not R-tree)
- Physics state includes CharacterBody2D fields (on_floor, on_wall, on_ceiling)

---

## Epic 12: Error Handling & Resilience

Graceful behavior when things go wrong.

### S12.1: Structured Error Responses
**Priority:** P0
**As an** AI agent,
**I want** clear, structured error responses with codes, messages, and suggestions,
**So that** I can recover or inform the user without guessing.

**Acceptance Criteria:**
- All errors return `{ error: { code, message, suggestion } }`
- Error codes: not_connected, scene_not_loaded, node_not_found, invalid_cursor, recording_not_found, recording_active, no_recording_active, budget_exceeded, method_not_found, eval_error, timeout, dimension_mismatch
- `suggestion` field provides actionable next steps (e.g., "Use scene_tree:find to search for nodes matching 'scout'")

### S12.2: Timeout Handling
**Priority:** P1
**As an** AI agent,
**I want** queries to time out gracefully if the game is frozen or under heavy load,
**So that** I'm not stuck waiting indefinitely.

**Acceptance Criteria:**
- TCP queries have a configurable timeout (default 5 seconds)
- Timeout returns `{ error: { code: "timeout", message: "Addon did not respond within 5000ms" } }`
- Suggestion: "Game may be frozen, at a breakpoint, or under heavy load"

### S12.3: Graceful Degradation During Scene Transitions
**Priority:** P1
**As an** AI agent,
**I want** Spectator to handle scene changes gracefully,
**So that** queries during transitions don't crash or return garbage.

**Acceptance Criteria:**
- During scene transitions, queries return `{ error: { code: "scene_not_loaded" } }`
- After transition completes, queries work normally with the new scene
- Static node cache is invalidated on scene change
- Watches are re-evaluated against the new scene tree (removed if nodes no longer exist)
