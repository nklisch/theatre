# Design: Director Phase 10 — Scene Wiring & Deferred Features

## Overview

Phase 10 adds "scene wiring" tools — signal connections, groups, script
attachment, metadata, and node search — plus finishes three deferred features
from earlier phases. These are the last major gaps in Director's scene
authoring capability. All target `.tscn` internals that are fragile to hand-edit.

**New tools (7):**
- `signal_connect` — connect signals between nodes in a scene
- `signal_disconnect` — remove signal connections
- `signal_list` — list all signal connections in a scene
- `node_set_groups` — add/remove nodes from groups
- `node_set_script` — attach or detach a GDScript from a scene node
- `node_set_meta` — set/clear metadata entries on a node
- `node_find` — search scene tree by class, group, property, or name pattern

**Deferred features (3):**
- `scene_list` — add `pattern` glob filter param
- `resource_read` — add `depth` param for nested resource serialization
- `scene_diff` — add git ref support (`HEAD:path` syntax)

---

## Implementation Units

### Unit 1: `signal_connect` — GDScript operation

**File**: `addons/director/ops/signal_ops.gd`

```gdscript
class_name SignalOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_signal_connect(params: Dictionary) -> Dictionary:
    ## Connect a signal between two nodes in a scene.
    ##
    ## Params:
    ##   scene_path: String
    ##   source_path: String        — node emitting the signal (relative to root)
    ##   signal_name: String        — signal name (e.g., "pressed", "body_entered")
    ##   target_path: String        — node receiving the signal
    ##   method_name: String        — method to call on target
    ##   binds: Array? (optional)   — extra arguments to pass to the method
    ##   flags: int? (optional)     — ConnectFlags bitmask (default 0)
    ##
    ## Returns: { success, data: { source_path, signal_name, target_path, method_name } }
```

**Implementation Notes:**
- Load scene via `PackedScene`, instantiate, find source and target nodes.
- Validate signal exists on source node via `source.get_signal_list()`.
- Call `source.connect(signal_name, Callable(target, method_name))` with optional binds/flags.
- The `binds` array values pass through `NodeOps.convert_value` for type conversion.
- Re-pack and save via `NodeOps._repack_and_save()`.
- Connection flags: use Godot's `CONNECT_DEFERRED` (1), `CONNECT_PERSIST` (2),
  `CONNECT_ONE_SHOT` (4). Default 0. Note: `CONNECT_PERSIST` is required for
  connections to survive `PackedScene.pack()` — add it automatically if not
  set by the caller. This is critical: without `CONNECT_PERSIST`, the connection
  is live-only and won't be saved to the `.tscn`.

**Acceptance Criteria:**
- [ ] Connects a signal between two nodes in a scene file
- [ ] Connection persists after re-loading the scene (CONNECT_PERSIST)
- [ ] Validates signal exists on source node — returns error if not
- [ ] Validates target node exists — returns error if not
- [ ] Validates source node exists — returns error if not
- [ ] Optional `binds` array is supported
- [ ] Optional `flags` parameter is forwarded

---

### Unit 2: `signal_disconnect` — GDScript operation

**File**: `addons/director/ops/signal_ops.gd` (append to Unit 1 file)

```gdscript
static func op_signal_disconnect(params: Dictionary) -> Dictionary:
    ## Remove a signal connection from a scene.
    ##
    ## Params:
    ##   scene_path: String
    ##   source_path: String
    ##   signal_name: String
    ##   target_path: String
    ##   method_name: String
    ##
    ## Returns: { success, data: { source_path, signal_name, target_path, method_name } }
```

**Implementation Notes:**
- Load, instantiate, find source and target.
- Verify connection exists via `source.is_connected(signal_name, Callable(target, method_name))`.
- Call `source.disconnect(signal_name, Callable(target, method_name))`.
- Re-pack and save.

**Acceptance Criteria:**
- [ ] Disconnects an existing signal connection
- [ ] Returns error if connection does not exist
- [ ] Returns error if source or target node not found
- [ ] Scene file no longer contains the connection after save

---

### Unit 3: `signal_list` — GDScript operation

**File**: `addons/director/ops/signal_ops.gd` (append)

```gdscript
static func op_signal_list(params: Dictionary) -> Dictionary:
    ## List all signal connections in a scene.
    ##
    ## Params:
    ##   scene_path: String
    ##   node_path: String? (optional — filter to connections from/to this node)
    ##
    ## Returns: { success, data: { connections: [{ source_path, signal_name,
    ##           target_path, method_name, flags }] } }
```

**Implementation Notes:**
- Load and instantiate the scene.
- Walk the entire node tree recursively.
- For each node, call `node.get_signal_connection_list(signal_name)` for each
  signal in `node.get_signal_list()`.
- Alternatively, use `node.get_incoming_connections()` on each node. However,
  the most reliable approach for packed scenes is to iterate
  `PackedScene.get_state().get_connection_count()` and read each connection
  via `get_connection_source()`, `get_connection_signal()`,
  `get_connection_target()`, `get_connection_method()`, `get_connection_flags()`.
  This reads connections as stored in the `.tscn` without needing to instantiate.
- Preferred approach: use `SceneState` API for accuracy (reads what's serialized).
- If `node_path` is provided, filter connections where source or target matches.

**Acceptance Criteria:**
- [ ] Returns all signal connections in a scene
- [ ] Each connection includes source_path, signal_name, target_path, method_name, flags
- [ ] Optional node_path filter works (filters by source or target)
- [ ] Empty connections array for scenes with no connections

---

### Unit 4: `node_set_groups` — GDScript operation

**File**: `addons/director/ops/node_ops.gd` (append to existing file)

```gdscript
static func op_node_set_groups(params: Dictionary) -> Dictionary:
    ## Add or remove a node from groups.
    ##
    ## Params:
    ##   scene_path: String
    ##   node_path: String
    ##   add: Array[String]?      — groups to add
    ##   remove: Array[String]?   — groups to remove
    ##
    ## Returns: { success, data: { node_path, groups: [String] } }
```

**Implementation Notes:**
- Load, instantiate, find node.
- For each group in `add`: call `node.add_to_group(group, true)`. The second
  arg `persistent=true` is critical — without it the group membership won't
  survive `PackedScene.pack()`.
- For each group in `remove`: call `node.remove_from_group(group)`.
- Return the final group list via `node.get_groups()`. Filter out internal
  groups (those starting with `_`).
- Re-pack and save.

**Acceptance Criteria:**
- [ ] Adds groups to a node (persisted in .tscn)
- [ ] Removes groups from a node
- [ ] Returns final group list after modifications
- [ ] At least one of `add` or `remove` must be provided
- [ ] Returns error for non-existent node
- [ ] Removing a group the node isn't in is silently ignored (matches Godot behavior)

---

### Unit 5: `node_set_script` — GDScript operation

**File**: `addons/director/ops/node_ops.gd` (append)

```gdscript
static func op_node_set_script(params: Dictionary) -> Dictionary:
    ## Attach or detach a script from a node in a scene.
    ##
    ## Params:
    ##   scene_path: String
    ##   node_path: String
    ##   script_path: String?     — "res://" path or project-relative path to .gd file
    ##                               omit or null to detach
    ##
    ## Returns: { success, data: { node_path, script_path: String|null } }
```

**Implementation Notes:**
- Load, instantiate, find node.
- If `script_path` is provided and non-empty:
  - Normalize to `res://` prefix if not already present.
  - Validate the script file exists via `ResourceLoader.exists()`.
  - Load the script: `var script = load(full_script_path)`.
  - Verify it's a Script resource: `if not script is Script`.
  - Set on node: `node.set_script(script)`.
- If `script_path` is null/empty/omitted: `node.set_script(null)` to detach.
- Re-pack and save.

**Acceptance Criteria:**
- [ ] Attaches a .gd script to a node in a scene
- [ ] Detaches a script when script_path is null/omitted
- [ ] Returns error if script file does not exist
- [ ] Returns error if file is not a Script resource
- [ ] Script persists in .tscn after save
- [ ] scene_read shows the script on the node after attachment

---

### Unit 6: `node_set_meta` — GDScript operation

**File**: `addons/director/ops/node_ops.gd` (append)

```gdscript
static func op_node_set_meta(params: Dictionary) -> Dictionary:
    ## Set or remove metadata entries on a node in a scene.
    ##
    ## Params:
    ##   scene_path: String
    ##   node_path: String
    ##   meta: Dictionary          — keys to set; value of null removes the key
    ##
    ## Returns: { success, data: { node_path, meta_keys: [String] } }
```

**Implementation Notes:**
- Load, instantiate, find node.
- For each key in `meta`:
  - If value is null: `node.remove_meta(key)`.
  - Otherwise: `node.set_meta(key, value)`. Values go through
    `NodeOps.convert_value` if a type hint is available, but metadata is
    untyped so pass values as-is (Godot stores Variant).
- Return the final metadata key list via `node.get_meta_list()`.
- Re-pack and save.

**Acceptance Criteria:**
- [ ] Sets metadata entries on a node (persisted in .tscn)
- [ ] Removes metadata entries when value is null
- [ ] Returns final list of metadata keys
- [ ] Returns error for non-existent node
- [ ] Metadata survives round-trip (set → save → reload → read)

---

### Unit 7: `node_find` — GDScript operation

**File**: `addons/director/ops/node_ops.gd` (append)

```gdscript
static func op_node_find(params: Dictionary) -> Dictionary:
    ## Search for nodes in a scene tree by class, group, property, or name.
    ##
    ## Params:
    ##   scene_path: String
    ##   class_name: String?       — filter by Godot class (e.g., "Sprite2D")
    ##   group: String?            — filter by group membership
    ##   name_pattern: String?     — filter by node name (supports * wildcard)
    ##   property: String?         — property name that must exist
    ##   property_value: any?      — if set, property must equal this value
    ##   limit: int?               — max results (default 100)
    ##
    ## Returns: { success, data: { results: [{ node_path, type, name }] } }
```

**Implementation Notes:**
- Load and instantiate the scene.
- Walk the tree recursively, collecting matches.
- Filter chain (all specified filters must match):
  - `class_name`: `node.is_class(class_name)` (supports inheritance)
  - `group`: `node.is_in_group(group)`
  - `name_pattern`: `node.name.match(name_pattern)` (Godot's `String.match()` supports `*` and `?`)
  - `property`+`property_value`: `node.get(property) == property_value` (with type conversion)
  - `property` alone: `property in node` (property exists)
- Apply `limit` to cap results.
- Return `results` array (plural — this is a ranked/filtered list per contract rules).
- Free scene tree after collection.

**Acceptance Criteria:**
- [ ] Finds nodes by class name (including inherited classes)
- [ ] Finds nodes by group membership
- [ ] Finds nodes by name pattern with wildcard
- [ ] Finds nodes by property existence
- [ ] Finds nodes by property value match
- [ ] Multiple filters combine as AND
- [ ] Limit caps results
- [ ] At least one filter must be provided — returns error if all are null
- [ ] Returns empty results array when no matches

---

### Unit 8: Rust param structs for new tools

**File**: `crates/director/src/mcp/signal.rs` (new file)

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `signal_connect`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalConnectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node emitting the signal (relative to scene root, e.g., "Button1").
    pub source_path: String,

    /// Signal name (e.g., "pressed", "body_entered").
    pub signal_name: String,

    /// Path to the node receiving the signal.
    pub target_path: String,

    /// Method name to call on the target node.
    pub method_name: String,

    /// Optional extra arguments to pass to the method.
    #[serde(default)]
    pub binds: Option<Vec<serde_json::Value>>,

    /// Optional connection flags bitmask (CONNECT_DEFERRED=1, CONNECT_PERSIST=2,
    /// CONNECT_ONE_SHOT=4). CONNECT_PERSIST is added automatically for scene
    /// serialization.
    #[serde(default)]
    pub flags: Option<u32>,
}

/// Parameters for `signal_disconnect`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalDisconnectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node emitting the signal.
    pub source_path: String,

    /// Signal name to disconnect.
    pub signal_name: String,

    /// Path to the node that was receiving the signal.
    pub target_path: String,

    /// Method name that was connected.
    pub method_name: String,
}

/// Parameters for `signal_list`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalListParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Optional: filter connections involving this node (as source or target).
    #[serde(default)]
    pub node_path: Option<String>,
}
```

**File**: `crates/director/src/mcp/node.rs` (append new param structs)

```rust
/// Parameters for `node_set_groups`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetGroupsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Groups to add the node to.
    #[serde(default)]
    pub add: Option<Vec<String>>,

    /// Groups to remove the node from.
    #[serde(default)]
    pub remove: Option<Vec<String>>,
}

/// Parameters for `node_set_script`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetScriptParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Path to the .gd script file (relative to project, e.g., "scripts/player.gd").
    /// Omit or set to null to detach the current script.
    #[serde(default)]
    pub script_path: Option<String>,
}

/// Parameters for `node_set_meta`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetMetaParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Metadata entries to set. Keys are metadata names, values are the data.
    /// Set a value to null to remove that metadata key.
    pub meta: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `node_find`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeFindParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Filter by Godot class name (supports inheritance, e.g., "Sprite2D" also
    /// matches CanvasItem queries). Uses is_class() internally.
    #[serde(default)]
    pub class_name: Option<String>,

    /// Filter by group membership.
    #[serde(default)]
    pub group: Option<String>,

    /// Filter by node name pattern (supports * and ? wildcards).
    #[serde(default)]
    pub name_pattern: Option<String>,

    /// Filter: property must exist on the node.
    #[serde(default)]
    pub property: Option<String>,

    /// Filter: property must equal this value (requires `property` to also be set).
    #[serde(default)]
    pub property_value: Option<serde_json::Value>,

    /// Maximum number of results to return (default: 100).
    #[serde(default = "default_find_limit")]
    pub limit: u32,
}

fn default_find_limit() -> u32 {
    100
}
```

**Acceptance Criteria:**
- [ ] All param structs derive `Debug, Deserialize, Serialize, JsonSchema`
- [ ] All doc comments describe the field purpose
- [ ] Optional fields use `#[serde(default)]`
- [ ] Required fields have no default

---

### Unit 9: Rust tool router entries for new tools

**File**: `crates/director/src/mcp/mod.rs` (add to imports and tool_router impl)

Add `pub mod signal;` to module list.

Add imports:
```rust
use node::{NodeFindParams, NodeSetGroupsParams, NodeSetMetaParams, NodeSetScriptParams};
use signal::{SignalConnectParams, SignalDisconnectParams, SignalListParams};
```

Add 7 new tool handler methods following the exact same pattern as existing
tools. Each is ~10 lines: serialize params, run_operation, serialize response.

```rust
#[tool(
    name = "signal_connect",
    description = "Connect a signal between two nodes in a Godot scene file (.tscn). \
        The connection is serialized into the scene and persists across loads. \
        Always use this tool instead of editing .tscn files directly — signal \
        connection blocks in .tscn are fragile and hand-editing will break them."
)]
pub async fn signal_connect(
    &self,
    Parameters(params): Parameters<SignalConnectParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "signal_connect", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "signal_disconnect",
    description = "Remove a signal connection between two nodes in a Godot scene file (.tscn). \
        Always use this tool instead of editing .tscn files directly."
)]
pub async fn signal_disconnect(
    &self,
    Parameters(params): Parameters<SignalDisconnectParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "signal_disconnect", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "signal_list",
    description = "List all signal connections in a Godot scene file (.tscn). Optionally \
        filter to connections involving a specific node. Returns source, signal name, \
        target, method, and flags for each connection."
)]
pub async fn signal_list(
    &self,
    Parameters(params): Parameters<SignalListParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "signal_list", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "node_set_groups",
    description = "Add or remove a node from named groups in a Godot scene file (.tscn). \
        Groups are used for gameplay logic (e.g., 'enemies', 'interactable') and are \
        queryable at runtime via get_tree().get_nodes_in_group(). Always use this \
        tool instead of editing .tscn files directly."
)]
pub async fn node_set_groups(
    &self,
    Parameters(params): Parameters<NodeSetGroupsParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "node_set_groups", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "node_set_script",
    description = "Attach or detach a GDScript (.gd) file to/from a node in a Godot \
        scene file (.tscn). The script must already exist on disk. Omit script_path \
        to detach. Always use this tool instead of editing .tscn files directly — \
        script references use internal resource IDs that are fragile to hand-edit."
)]
pub async fn node_set_script(
    &self,
    Parameters(params): Parameters<NodeSetScriptParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "node_set_script", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "node_set_meta",
    description = "Set or remove metadata entries on a node in a Godot scene file (.tscn). \
        Metadata is arbitrary key-value data stored on nodes, useful for editor \
        annotations, gameplay tags, or tool configuration. Set a value to null to \
        remove that key. Always use this tool instead of editing .tscn files directly."
)]
pub async fn node_set_meta(
    &self,
    Parameters(params): Parameters<NodeSetMetaParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "node_set_meta", &op_params,
    ).await?;
    serialize_response(&data)
}

#[tool(
    name = "node_find",
    description = "Search for nodes in a Godot scene file by class, group, name pattern, \
        or property. Multiple filters combine as AND. Returns matching node paths \
        and types. Use this to discover nodes without knowing the exact tree structure."
)]
pub async fn node_find(
    &self,
    Parameters(params): Parameters<NodeFindParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(
        &self.backend, &params.project_path, "node_find", &op_params,
    ).await?;
    serialize_response(&data)
}
```

**Acceptance Criteria:**
- [ ] All 7 tools registered in the tool router
- [ ] Tool descriptions include anti-direct-edit guidance
- [ ] Each handler follows the exact same ~10-line pattern as existing tools
- [ ] Compiles with `cargo build -p director`

---

### Unit 10: Dispatcher entries in operations.gd, daemon.gd, editor_ops.gd

**File**: `addons/director/operations.gd`

Add `const SignalOps = preload("res://addons/director/ops/signal_ops.gd")` to
the imports block. Add these entries to the `match args.operation:` block:

```gdscript
"signal_connect":
    result = SignalOps.op_signal_connect(args.params)
"signal_disconnect":
    result = SignalOps.op_signal_disconnect(args.params)
"signal_list":
    result = SignalOps.op_signal_list(args.params)
"node_set_groups":
    result = NodeOps.op_node_set_groups(args.params)
"node_set_script":
    result = NodeOps.op_node_set_script(args.params)
"node_set_meta":
    result = NodeOps.op_node_set_meta(args.params)
"node_find":
    result = NodeOps.op_node_find(args.params)
```

**File**: `addons/director/daemon.gd`

Add `const SignalOps = preload("res://addons/director/ops/signal_ops.gd")` and
mirror the same 7 entries in `_dispatch()`.

**File**: `addons/director/editor_ops.gd`

Add `const SignalOps = preload("res://addons/director/ops/signal_ops.gd")` and:
- Add `"signal_connect"`, `"signal_disconnect"`, `"node_set_groups"`,
  `"node_set_script"`, `"node_set_meta"` to `SCENE_OPS` array (they modify
  the active scene and benefit from live tree manipulation).
- Add `"signal_list"` and `"node_find"` to `SCENE_OPS` (read operations on
  live tree see unsaved changes).
- Add live dispatch methods in `_dispatch_live()` and headless fallthrough
  entries in `_dispatch_headless()`.

**Editor live implementations:**

```gdscript
# signal_connect on live tree:
static func _live_signal_connect(params: Dictionary, scene_root: Node) -> Dictionary:
    var source_path: String = params.get("source_path", "")
    var signal_name: String = params.get("signal_name", "")
    var target_path: String = params.get("target_path", "")
    var method_name: String = params.get("method_name", "")
    var flags: int = params.get("flags", 0)

    var source: Node = _resolve_node(scene_root, source_path)
    if source == null:
        return OpsUtil._error("Source node not found: " + source_path, "signal_connect", params)

    var target: Node = _resolve_node(scene_root, target_path)
    if target == null:
        return OpsUtil._error("Target node not found: " + target_path, "signal_connect", params)

    # Ensure CONNECT_PERSIST for scene serialization
    flags = flags | 2  # CONNECT_PERSIST = 2

    source.connect(signal_name, Callable(target, method_name), flags)
    return {"success": true, "data": {
        "source_path": source_path, "signal_name": signal_name,
        "target_path": target_path, "method_name": method_name,
    }}
```

Similar live methods for `signal_disconnect`, `signal_list`, `node_set_groups`,
`node_set_script`, `node_set_meta`, and `node_find` — following the existing
pattern of resolving nodes from the live tree root.

**Acceptance Criteria:**
- [ ] All 7 operations dispatched correctly from all three entry points
- [ ] Editor live variants use the active scene tree
- [ ] Headless fallthrough delegates to ops/ methods
- [ ] New operations added to `SCENE_OPS` array where appropriate

---

### Unit 11: Deferred — `scene_list` pattern filter

**File**: `addons/director/ops/scene_ops.gd` — modify `op_scene_list`

```gdscript
# Add pattern support to op_scene_list:
var pattern: String = params.get("pattern", "")

# After collecting scene_paths:
if pattern != "":
    var filtered: Array = []
    for path in scene_paths:
        var rel = path.replace("res://", "")
        if rel.match(pattern):
            filtered.append(path)
    scene_paths = filtered
```

**File**: `crates/director/src/mcp/scene.rs` — add field to `SceneListParams`

```rust
/// Glob pattern to filter scene paths (e.g., "scenes/**/*.tscn").
/// Uses Godot's String.match() which supports * and ? wildcards.
#[serde(default)]
pub pattern: Option<String>,
```

**Acceptance Criteria:**
- [ ] `scene_list` accepts optional `pattern` parameter
- [ ] Pattern filters scene paths using Godot's `String.match()` wildcard syntax
- [ ] Omitting pattern returns all scenes (backward-compatible)

---

### Unit 12: Deferred — `resource_read` depth parameter

**File**: `addons/director/ops/resource_ops.gd` — modify `op_resource_read`

Add `depth` parameter support. When serializing resource properties, if a
property value is itself a Resource:
- At depth 0 (or when at max depth): serialize as its `resource_path` string.
- At depth > 0: recursively serialize its properties.

```gdscript
var depth: int = params.get("depth", 1)

# In the property serialization loop:
static func _serialize_resource_value(value, current_depth: int, max_depth: int):
    if value is Resource:
        if current_depth >= max_depth:
            return value.resource_path if value.resource_path != "" else str(value)
        else:
            return _serialize_resource_properties(value, current_depth + 1, max_depth)
    # ... other type handling
```

**File**: `crates/director/src/mcp/resource.rs` — add field to `ResourceReadParams`

```rust
/// Depth for nested resource serialization. At depth 0, nested resources are
/// returned as path strings. At depth 1 (default), top-level properties are
/// serialized but nested resources within them are paths. Higher depths recurse.
#[serde(default = "default_depth")]
pub depth: u32,

fn default_depth() -> u32 { 1 }
```

**Acceptance Criteria:**
- [ ] `resource_read` accepts optional `depth` parameter
- [ ] Default depth 1 matches current behavior
- [ ] Depth 0 returns all sub-resources as path strings
- [ ] Depth 2+ recursively serializes nested resources

---

### Unit 13: Deferred — `scene_diff` git ref support

**File**: `addons/director/ops/meta_ops.gd` — modify `op_scene_diff`

Add detection for git-ref syntax (`HEAD:path`, `commit:path`):

```gdscript
static func _resolve_scene_source(scene_ref: String) -> Dictionary:
    ## Resolve a scene reference to a temporary file path.
    ## Returns { path: String, is_temp: bool, error: String }
    if ":" in scene_ref and not scene_ref.begins_with("res://"):
        # Git ref syntax: "HEAD:scenes/player.tscn" or "abc123:scenes/player.tscn"
        var parts = scene_ref.split(":", true, 1)
        var git_ref = parts[0]
        var file_path = parts[1]

        # Shell out to git to extract the file content
        var output: Array = []
        var exit_code = OS.execute("git", [
            "-C", ProjectSettings.globalize_path("res://"),
            "show", git_ref + ":" + file_path,
        ], output, true)

        if exit_code != 0:
            return {"path": "", "is_temp": false, "error": "Git ref not found: " + scene_ref}

        # Write to a temp file so Godot can load it
        var temp_path = "res://tmp/_scene_diff_" + str(Time.get_ticks_msec()) + ".tscn"
        var dir = DirAccess.open("res://")
        if not DirAccess.dir_exists_absolute("res://tmp"):
            DirAccess.make_dir_recursive_absolute("res://tmp")
        var f = FileAccess.open(temp_path, FileAccess.WRITE)
        f.store_string(output[0])
        f.close()

        return {"path": temp_path, "is_temp": true, "error": ""}
    else:
        return {"path": "res://" + scene_ref, "is_temp": false, "error": ""}
```

After the diff completes, clean up any temp files.

**File**: `crates/director/src/mcp/meta.rs` — update `SceneDiffParams` description

Update the tool description to mention git ref support:

```rust
#[tool(
    name = "scene_diff",
    description = "Compare two Godot scene files structurally. Returns lists of \
        added nodes, removed nodes, and changed properties. Supports git refs \
        (e.g., \"HEAD:scenes/player.tscn\") to compare against previous versions."
)]
```

**Acceptance Criteria:**
- [ ] `scene_diff` accepts git ref syntax for scene_a and/or scene_b
- [ ] `HEAD:path` resolves to the file content at HEAD
- [ ] `commit_hash:path` resolves to file content at that commit
- [ ] Temp files are cleaned up after diff
- [ ] Returns error for invalid git refs
- [ ] Regular file paths still work (backward-compatible)
- [ ] Works when git is not available (returns clear error)

---

## Implementation Order

1. **Unit 8** — Rust param structs (`signal.rs` new file, `node.rs` additions)
2. **Unit 1** — `signal_connect` GDScript
3. **Unit 2** — `signal_disconnect` GDScript
4. **Unit 3** — `signal_list` GDScript
5. **Unit 4** — `node_set_groups` GDScript
6. **Unit 5** — `node_set_script` GDScript
7. **Unit 6** — `node_set_meta` GDScript
8. **Unit 7** — `node_find` GDScript
9. **Unit 9** — Rust tool router entries
10. **Unit 10** — Dispatcher entries (operations.gd, daemon.gd, editor_ops.gd)
11. **Unit 11** — Deferred: `scene_list` pattern
12. **Unit 12** — Deferred: `resource_read` depth
13. **Unit 13** — Deferred: `scene_diff` git refs

Units 11-13 are independent of each other and of units 1-10.

---

## Testing

### Test file: `tests/director-tests/src/test_signal.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_basic() {
    // Create scene with two nodes, connect signal, verify via signal_list
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_connect");

    f.run("scene_create", json!({"scene_path": &scene, "root_type": "Node2D"})).unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": &scene, "parent_path": ".", "node_type": "Button", "node_name": "MyButton"})).unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Handler"})).unwrap().unwrap_data();

    let data = f.run("signal_connect", json!({
        "scene_path": &scene,
        "source_path": "MyButton",
        "signal_name": "pressed",
        "target_path": "Handler",
        "method_name": "on_button_pressed",
    })).unwrap().unwrap_data();

    assert_eq!(data["signal_name"], "pressed");

    // Verify via signal_list
    let list = f.run("signal_list", json!({"scene_path": &scene})).unwrap().unwrap_data();
    let connections = list["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0]["signal_name"], "pressed");
    assert_eq!(connections[0]["method_name"], "on_button_pressed");
}

#[test]
#[ignore = "requires Godot binary"]
fn signal_disconnect_removes_connection() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_invalid_signal_returns_error() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn signal_list_empty_scene() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn signal_list_filtered_by_node() { ... }
```

### Test file: `tests/director-tests/src/test_wiring.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn node_set_groups_add_and_remove() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("groups");
    f.run("scene_create", json!({"scene_path": &scene, "root_type": "Node2D"})).unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Enemy"})).unwrap().unwrap_data();

    let data = f.run("node_set_groups", json!({
        "scene_path": &scene, "node_path": "Enemy",
        "add": ["enemies", "damageable"],
    })).unwrap().unwrap_data();

    let groups = data["groups"].as_array().unwrap();
    assert!(groups.iter().any(|g| g == "enemies"));
    assert!(groups.iter().any(|g| g == "damageable"));

    // Remove one group
    let data2 = f.run("node_set_groups", json!({
        "scene_path": &scene, "node_path": "Enemy",
        "remove": ["enemies"],
    })).unwrap().unwrap_data();

    let groups2 = data2["groups"].as_array().unwrap();
    assert!(!groups2.iter().any(|g| g == "enemies"));
    assert!(groups2.iter().any(|g| g == "damageable"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_script_attach_and_detach() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn node_set_meta_set_and_remove() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_class() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_group() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn node_find_combined_filters() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn node_find_no_filter_returns_error() { ... }
```

### Test file: `tests/director-tests/src/test_deferred.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn scene_list_with_pattern_filter() {
    let f = DirectorFixture::new();
    // Create scenes in different dirs
    f.run("scene_create", json!({"scene_path": "tmp/deferred/a.tscn", "root_type": "Node2D"})).unwrap();
    f.run("scene_create", json!({"scene_path": "tmp/deferred/b.tscn", "root_type": "Node3D"})).unwrap();
    f.run("scene_create", json!({"scene_path": "tmp/other/c.tscn", "root_type": "Node2D"})).unwrap();

    let data = f.run("scene_list", json!({"pattern": "tmp/deferred/*.tscn"})).unwrap().unwrap_data();
    let scenes = data["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_depth_parameter() { ... }

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_git_ref() {
    // This test requires a committed scene file in git.
    // Skip if git is not available.
}
```

### Register test modules in `tests/director-tests/src/lib.rs`

```rust
#[cfg(test)]
mod test_signal;
#[cfg(test)]
mod test_wiring;
#[cfg(test)]
mod test_deferred;
```

---

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Deploy GDExtension to test project
theatre-deploy ~/dev/spectator/tests/godot-project

# Run all tests
cargo test --workspace

# Run Phase 10 tests specifically
cargo test -p director-tests -- --include-ignored test_signal
cargo test -p director-tests -- --include-ignored test_wiring
cargo test -p director-tests -- --include-ignored test_deferred
```
