# Design: Phase 8 — Advanced Tools

## Overview

Phase 8 adds three tools to Director's tool surface:

1. **`visual_shader_create`** — Build a VisualShader node graph from a JSON description of nodes and connections
2. **`physics_set_layers`** — Set `collision_layer`/`collision_mask` bitmasks on a node in a scene
3. **`physics_set_layer_names`** — Write physics/render layer names to `project.godot`

These are independent of each other and of Phases 2-7. They depend only on Phase 1 infrastructure (backend routing, operation dispatch, type conversion).

---

## Implementation Units

### Unit 1: `physics_set_layers` — Rust MCP Tool

**File**: `crates/director/src/mcp/physics.rs`

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `physics_set_layers`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhysicsSetLayersParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file to modify (relative to project, e.g. "scenes/player.tscn").
    pub scene_path: String,

    /// Path to the node within the scene tree (e.g. "Player/CollisionShape2D").
    pub node_path: String,

    /// Collision layer bitmask (32-bit unsigned integer). Determines which
    /// physics layers this object occupies. Omit to leave unchanged.
    #[serde(default)]
    pub collision_layer: Option<u32>,

    /// Collision mask bitmask (32-bit unsigned integer). Determines which
    /// physics layers this object scans/detects. Omit to leave unchanged.
    #[serde(default)]
    pub collision_mask: Option<u32>,
}
```

**Implementation Notes**:
- Follows the exact same pattern as every other scene-targeting tool: `serialize_params` → `run_operation` → `serialize_response`.
- This is a scene modification operation, so it enters the `SCENE_OPS` list for live editor dispatch.
- At least one of `collision_layer` or `collision_mask` must be set (validated in GDScript, not Rust — consistent with how other tools validate).

**Acceptance Criteria**:
- [ ] `PhysicsSetLayersParams` struct exists with all fields documented
- [ ] Tool registered in `#[tool_router]` with description mentioning bitmask usage
- [ ] Tool description advises using this instead of editing .tscn files directly

---

### Unit 2: `physics_set_layers` — GDScript Operation

**File**: `addons/director/ops/physics_ops.gd`

```gdscript
class_name PhysicsOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_physics_set_layers(params: Dictionary) -> Dictionary:
    ## Set collision_layer and/or collision_mask on a node in a scene.
    ##
    ## Params: scene_path, node_path, collision_layer?, collision_mask?
    ## Returns: { success, data: { node_path, collision_layer, collision_mask } }

    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var collision_layer = params.get("collision_layer", null)
    var collision_mask = params.get("collision_mask", null)

    # Validation
    if scene_path == "":
        return OpsUtil._error("scene_path is required", "physics_set_layers", params)
    if node_path == "":
        return OpsUtil._error("node_path is required", "physics_set_layers", params)
    if collision_layer == null and collision_mask == null:
        return OpsUtil._error(
            "At least one of collision_layer or collision_mask is required",
            "physics_set_layers", params)

    # Load scene + find node
    var full_scene = "res://" + scene_path
    if not ResourceLoader.exists(full_scene):
        return OpsUtil._error("Scene not found: " + scene_path,
            "physics_set_layers", {"scene_path": scene_path})

    var packed: PackedScene = load(full_scene)
    var root = packed.instantiate()
    var target = root.get_node_or_null(node_path)
    if target == null:
        root.free()
        return OpsUtil._error("Node not found: " + node_path,
            "physics_set_layers", {"scene_path": scene_path, "node_path": node_path})

    # Verify the node has collision properties
    var has_layer := false
    var has_mask := false
    for prop_info in target.get_property_list():
        if prop_info["name"] == "collision_layer":
            has_layer = true
        if prop_info["name"] == "collision_mask":
            has_mask = true
    if not has_layer and not has_mask:
        root.free()
        return OpsUtil._error(
            "Node " + node_path + " (" + target.get_class() +
            ") has no collision_layer/collision_mask properties",
            "physics_set_layers",
            {"node_path": node_path, "class": target.get_class()})

    # Apply values
    if collision_layer != null:
        if not has_layer:
            root.free()
            return OpsUtil._error(
                "Node does not have collision_layer property",
                "physics_set_layers",
                {"node_path": node_path, "class": target.get_class()})
        target.collision_layer = int(collision_layer)

    if collision_mask != null:
        if not has_mask:
            root.free()
            return OpsUtil._error(
                "Node does not have collision_mask property",
                "physics_set_layers",
                {"node_path": node_path, "class": target.get_class()})
        target.collision_mask = int(collision_mask)

    # Repack and save
    var save_result = NodeOps._repack_and_save(root, full_scene)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {
        "node_path": node_path,
        "collision_layer": target.collision_layer,
        "collision_mask": target.collision_mask,
    }}
```

**Implementation Notes**:
- Uses `get_property_list()` to verify the node actually supports collision layers (works for `PhysicsBody2D`, `PhysicsBody3D`, `Area2D`, `Area3D`, `TileMapLayer`, etc.)
- Returns the final values after setting, so the agent can verify. Reads are taken before `root.free()`.
- `int()` cast ensures JSON numbers become Godot integers.

**Acceptance Criteria**:
- [ ] Setting `collision_layer` alone works
- [ ] Setting `collision_mask` alone works
- [ ] Setting both at once works
- [ ] Error returned for non-physics node (e.g. `Node2D`)
- [ ] Error returned for missing node
- [ ] Error returned when neither layer nor mask provided

---

### Unit 3: `physics_set_layers` — Live Editor Dispatch

**File**: `addons/director/editor_ops.gd` (modify existing)

Add `"physics_set_layers"` to `SCENE_OPS` and add a live dispatch handler.

```gdscript
# In SCENE_OPS const array, add:
"physics_set_layers",

# In _dispatch_live match:
"physics_set_layers":
    return _live_physics_set_layers(params, scene_root)

# New method:
static func _live_physics_set_layers(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var collision_layer = params.get("collision_layer", null)
    var collision_mask = params.get("collision_mask", null)

    if node_path == "":
        return OpsUtil._error("node_path is required", "physics_set_layers", params)
    if collision_layer == null and collision_mask == null:
        return OpsUtil._error(
            "At least one of collision_layer or collision_mask is required",
            "physics_set_layers", params)

    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path,
            "physics_set_layers", params)

    # Verify collision properties exist
    var has_layer := "collision_layer" in node
    var has_mask := "collision_mask" in node
    if not has_layer and not has_mask:
        return OpsUtil._error(
            "Node " + node_path + " (" + node.get_class() +
            ") has no collision properties",
            "physics_set_layers",
            {"node_path": node_path, "class": node.get_class()})

    if collision_layer != null:
        node.collision_layer = int(collision_layer)
    if collision_mask != null:
        node.collision_mask = int(collision_mask)

    return {"success": true, "data": {
        "node_path": node_path,
        "collision_layer": node.collision_layer,
        "collision_mask": node.collision_mask,
    }}
```

**Implementation Notes**:
- Live dispatch uses the existing `_resolve_node` helper.
- Uses `"collision_layer" in node` as a simpler property existence check for live nodes.
- No repack/save needed — the editor tracks the change.

**Acceptance Criteria**:
- [ ] Live dispatch path reached when scene is active in editor
- [ ] Headless fallthrough path reached when scene is not active

---

### Unit 4: `physics_set_layer_names` — Rust MCP Tool

**File**: `crates/director/src/mcp/physics.rs` (append to same file)

```rust
/// Parameters for `physics_set_layer_names`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhysicsSetLayerNamesParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Which layer category to configure.
    /// Valid values: "2d_physics", "3d_physics", "2d_render", "3d_render",
    /// "2d_navigation", "3d_navigation", "avoidance".
    pub layer_type: String,

    /// Map of layer number (1-32) to human-readable name.
    /// Example: {"1": "player", "2": "enemies", "5": "projectiles"}.
    /// Layers not included are left unchanged.
    pub layers: serde_json::Map<String, serde_json::Value>,
}
```

**Implementation Notes**:
- This is NOT a scene operation — it modifies `project.godot`. It is always headless.
- The `layers` field uses `Map<String, Value>` because JSON object keys are always strings, and layer numbers 1-32 come as string keys.
- Backend routing: this operation has no scene_path, so it always falls through to headless dispatch. No special backend logic needed.

**Acceptance Criteria**:
- [ ] `PhysicsSetLayerNamesParams` struct with all fields documented
- [ ] Tool registered with description explaining layer categories
- [ ] `layer_type` valid values listed in description

---

### Unit 5: `physics_set_layer_names` — GDScript Operation

**File**: `addons/director/ops/physics_ops.gd` (append)

```gdscript
static func op_physics_set_layer_names(params: Dictionary) -> Dictionary:
    ## Write physics/render/navigation layer names to project.godot.
    ##
    ## Params: layer_type, layers (dict of layer_number → name)
    ## Returns: { success, data: { layer_type, layers_set: int } }

    var layer_type: String = params.get("layer_type", "")
    var layers = params.get("layers", {})

    if layer_type == "":
        return OpsUtil._error("layer_type is required",
            "physics_set_layer_names", params)

    var valid_types := [
        "2d_physics", "3d_physics",
        "2d_render", "3d_render",
        "2d_navigation", "3d_navigation",
        "avoidance",
    ]
    if not layer_type in valid_types:
        return OpsUtil._error(
            "Invalid layer_type: " + layer_type +
            ". Must be one of: " + ", ".join(valid_types),
            "physics_set_layer_names", {"layer_type": layer_type})

    if not layers is Dictionary or layers.is_empty():
        return OpsUtil._error("layers must be a non-empty dictionary",
            "physics_set_layer_names", params)

    var layers_set := 0
    for key in layers:
        var layer_num: int = int(key)
        if layer_num < 1 or layer_num > 32:
            return OpsUtil._error(
                "Layer number must be 1-32, got: " + str(key),
                "physics_set_layer_names",
                {"layer_type": layer_type, "layer": key})

        var name: String = str(layers[key])
        var setting := "layer_names/" + layer_type + "/layer_" + str(layer_num)
        ProjectSettings.set_setting(setting, name)
        layers_set += 1

    var err := ProjectSettings.save()
    if err != OK:
        return OpsUtil._error("Failed to save project settings: " + str(err),
            "physics_set_layer_names", {"layer_type": layer_type})

    return {"success": true, "data": {
        "layer_type": layer_type,
        "layers_set": layers_set,
    }}
```

**Implementation Notes**:
- Uses `ProjectSettings.set_setting()` and `ProjectSettings.save()` — Godot's own API for modifying `project.godot`.
- The setting path format is `layer_names/<category>/layer_<N>` where N is 1-32.
- This does NOT go through the editor live dispatch path. It has no `scene_path` and doesn't target a scene node. It always runs headless (or through the headless fallthrough in editor mode).
- Layer names with empty string values effectively clear/reset that layer name.
- `avoidance` is a valid category in Godot 4.x for navigation avoidance layers.

**Acceptance Criteria**:
- [ ] Setting 2d_physics layer names writes correct entries to project.godot
- [ ] Setting 3d_physics layer names works
- [ ] Setting render layer names works
- [ ] Setting navigation layer names works
- [ ] Invalid layer_type returns error
- [ ] Layer number out of range (0, 33) returns error
- [ ] Empty layers dict returns error
- [ ] Multiple layers set in one call works

---

### Unit 6: `visual_shader_create` — Rust MCP Tool

**File**: `crates/director/src/mcp/shader.rs`

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A node in a VisualShader graph.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderNode {
    /// Unique integer ID for this node within the shader. IDs 0 and 1 are
    /// reserved (output node). Start custom nodes at 2+.
    pub node_id: i32,

    /// VisualShader node class name (e.g. "VisualShaderNodeInput",
    /// "VisualShaderNodeVectorOp", "VisualShaderNodeColorConstant").
    /// Must be a valid ClassDB class that extends VisualShaderNode.
    #[serde(rename = "type")]
    pub node_type: String,

    /// Which shader function graph this node belongs to.
    /// Valid values: "vertex", "fragment", "light".
    /// For particles mode: "start", "process", "collide".
    pub shader_function: String,

    /// Position in the visual shader editor graph (for layout).
    #[serde(default)]
    pub position: Option<[f64; 2]>,

    /// Properties to set on the node after creation.
    /// Type conversion is automatic (same as node_set_properties).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}


/// A connection between two nodes in a VisualShader graph.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderConnection {
    /// Source node ID.
    pub from_node: i32,

    /// Source port index on the from_node.
    pub from_port: i32,

    /// Destination node ID.
    pub to_node: i32,

    /// Destination port index on the to_node.
    pub to_port: i32,

    /// Which shader function graph this connection belongs to.
    /// Must match the shader_function of the connected nodes.
    pub shader_function: String,
}

/// Parameters for `visual_shader_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the VisualShader resource (relative to project,
    /// e.g. "shaders/water.tres").
    pub resource_path: String,

    /// Shader processing mode. Valid values: "spatial" (3D), "canvas_item" (2D),
    /// "particles", "sky", "fog".
    pub shader_mode: String,

    /// Nodes to add to the shader graph. The output node (ID 0) exists
    /// automatically — do not include it. Node IDs must be unique and >= 2.
    pub nodes: Vec<VisualShaderNode>,

    /// Connections between nodes. Each connection links an output port on
    /// one node to an input port on another.
    #[serde(default)]
    pub connections: Vec<VisualShaderConnection>,
}
```

**Implementation Notes**:
- `node_id` uses the contract rule: `<resource>_id` naming. The field is `node_id` (not bare `id`).
- The `type` field is renamed from `node_type` via `#[serde(rename = "type")]` because "type" is the natural JSON key for node class names, and `type` is a Rust keyword.
- Node ID 0 is the output node (always exists in a VisualShader). Node ID 1 is reserved. Custom nodes start at 2.
- This is a resource creation operation → always headless. No live editor dispatch needed.

**Acceptance Criteria**:
- [ ] All three structs (`VisualShaderNode`, `VisualShaderConnection`, `VisualShaderCreateParams`) compile
- [ ] Tool registered with description explaining the node/connection model
- [ ] Description warns against hand-editing .tres files

---

### Unit 7: `visual_shader_create` — GDScript Operation

**File**: `addons/director/ops/shader_ops.gd`

```gdscript
class_name ShaderOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

## Maps shader_mode strings to VisualShader.Mode enum values.
const SHADER_MODES := {
    "spatial": VisualShader.MODE_SPATIAL,
    "canvas_item": VisualShader.MODE_CANVAS_ITEM,
    "particles": VisualShader.MODE_PARTICLES,
    "sky": VisualShader.MODE_SKY,
    "fog": VisualShader.MODE_FOG,
}

## Maps shader_function strings to VisualShader.Type enum values.
const SHADER_FUNCTIONS := {
    "vertex": VisualShader.TYPE_VERTEX,
    "fragment": VisualShader.TYPE_FRAGMENT,
    "light": VisualShader.TYPE_LIGHT,
    "start": VisualShader.TYPE_START,
    "process": VisualShader.TYPE_PROCESS,
    "collide": VisualShader.TYPE_COLLIDE,
    "start_custom": VisualShader.TYPE_START_CUSTOM,
    "process_custom": VisualShader.TYPE_PROCESS_CUSTOM,
    "sky": VisualShader.TYPE_SKY,
    "fog": VisualShader.TYPE_FOG,
}


static func op_visual_shader_create(params: Dictionary) -> Dictionary:
    ## Create a VisualShader resource with nodes and connections.
    ##
    ## Params: resource_path, shader_mode, nodes: [{node_id, type, position?, properties?}],
    ##         connections: [{from_node, from_port, to_node, to_port}]
    ## Returns: { success, data: { path, node_count, connection_count } }

    var resource_path: String = params.get("resource_path", "")
    var shader_mode: String = params.get("shader_mode", "")
    var nodes: Array = params.get("nodes", [])
    var connections: Array = params.get("connections", [])

    # Validate required params
    if resource_path == "":
        return OpsUtil._error("resource_path is required",
            "visual_shader_create", params)
    if shader_mode == "":
        return OpsUtil._error("shader_mode is required",
            "visual_shader_create", params)
    if not SHADER_MODES.has(shader_mode):
        return OpsUtil._error(
            "Invalid shader_mode: " + shader_mode +
            ". Must be one of: " + ", ".join(SHADER_MODES.keys()),
            "visual_shader_create", {"shader_mode": shader_mode})

    # Create the VisualShader
    var shader := VisualShader.new()
    shader.set_mode(SHADER_MODES[shader_mode])

    # Add nodes
    var node_count := 0
    for node_def in nodes:
        var result := _add_shader_node(shader, node_def)
        if not result.success:
            return result
        node_count += 1

    # Add connections
    var connection_count := 0
    for conn in connections:
        var result := _add_connection(shader, conn)
        if not result.success:
            return result
        connection_count += 1

    # Save
    var full_path := "res://" + resource_path
    ResourceOps._ensure_directory(full_path)
    var err := ResourceSaver.save(shader, full_path)
    if err != OK:
        return OpsUtil._error("Failed to save visual shader: " + str(err),
            "visual_shader_create", {"resource_path": resource_path})

    return {"success": true, "data": {
        "path": resource_path,
        "node_count": node_count,
        "connection_count": connection_count,
    }}


static func _add_shader_node(shader: VisualShader, node_def: Dictionary) -> Dictionary:
    ## Add a single node to the visual shader graph.
    var node_id: int = node_def.get("node_id", -1)
    var node_type: String = node_def.get("type", "")
    var shader_function: String = node_def.get("shader_function", "")

    if node_id < 2:
        return OpsUtil._error(
            "node_id must be >= 2 (0 and 1 are reserved), got: " + str(node_id),
            "visual_shader_create", {"node_id": node_id})
    if node_type == "":
        return OpsUtil._error("Node type is required",
            "visual_shader_create", {"node_id": node_id})
    if shader_function == "":
        return OpsUtil._error(
            "shader_function is required on each node",
            "visual_shader_create", {"node_id": node_id})
    if not SHADER_FUNCTIONS.has(shader_function):
        return OpsUtil._error(
            "Invalid shader_function: " + shader_function +
            ". Must be one of: " + ", ".join(SHADER_FUNCTIONS.keys()),
            "visual_shader_create",
            {"node_id": node_id, "shader_function": shader_function})
    if not ClassDB.class_exists(node_type):
        return OpsUtil._error("Unknown class: " + node_type,
            "visual_shader_create", {"node_id": node_id, "type": node_type})
    if not ClassDB.is_parent_class(node_type, "VisualShaderNode"):
        return OpsUtil._error(
            node_type + " is not a VisualShaderNode subclass",
            "visual_shader_create", {"node_id": node_id, "type": node_type})

    var vs_type: int = SHADER_FUNCTIONS[shader_function]
    var node: VisualShaderNode = ClassDB.instantiate(node_type)

    # Set properties if provided
    var properties = node_def.get("properties", null)
    if properties is Dictionary and not properties.is_empty():
        var prop_list = node.get_property_list()
        var type_map: Dictionary = {}
        for prop_info in prop_list:
            type_map[prop_info["name"]] = prop_info["type"]

        for prop_name in properties:
            if not type_map.has(prop_name):
                return OpsUtil._error(
                    "Unknown property: " + prop_name + " on " + node_type,
                    "visual_shader_create",
                    {"node_id": node_id, "property": prop_name})
            var converted = NodeOps.convert_value(
                properties[prop_name], type_map[prop_name])
            node.set(prop_name, converted)

    # Add to the specified shader function graph
    shader.add_node(vs_type, node, Vector2.ZERO, node_id)

    # Set position if provided
    var position = node_def.get("position", null)
    if position is Array and position.size() == 2:
        shader.set_node_position(vs_type, node_id,
            Vector2(position[0], position[1]))

    return {"success": true}


static func _add_connection(shader: VisualShader, conn: Dictionary) -> Dictionary:
    ## Add a connection between two nodes in the shader graph.
    var from_node: int = conn.get("from_node", -1)
    var from_port: int = conn.get("from_port", -1)
    var to_node: int = conn.get("to_node", -1)
    var to_port: int = conn.get("to_port", -1)
    var shader_function: String = conn.get("shader_function", "")

    if from_node < 0 or to_node < 0:
        return OpsUtil._error(
            "Connection requires from_node and to_node",
            "visual_shader_create", conn)
    if shader_function == "":
        return OpsUtil._error(
            "shader_function is required on each connection",
            "visual_shader_create", conn)
    if not SHADER_FUNCTIONS.has(shader_function):
        return OpsUtil._error(
            "Invalid shader_function: " + shader_function,
            "visual_shader_create", conn)

    var vs_type: int = SHADER_FUNCTIONS[shader_function]
    var err := shader.connect_nodes(
        vs_type, from_node, from_port, to_node, to_port)
    if err != OK:
        return OpsUtil._error(
            "Failed to connect nodes: " + str(from_node) + ":" +
            str(from_port) + " → " + str(to_node) + ":" + str(to_port) +
            " (error " + str(err) + ")",
            "visual_shader_create", conn)

    return {"success": true}
```

**Implementation Notes**:

- **Shader function routing**: VisualShader maintains separate node graphs for each processing function (vertex, fragment, light, and particle-specific functions). Each node and connection specifies which function graph it belongs to via the `shader_function` field (defaults to `"vertex"`). This maps directly to Godot's `VisualShader.Type` enum passed to `add_node()` and `connect_nodes()`.
  - `shader_mode` sets the overall shader type (spatial, canvas_item, particles, sky, fog)
  - `shader_function` on each node/connection sets which processing function graph within that mode
  - The output node (ID 0) exists automatically for each function type — agent connects to it, never creates it
  - Node IDs are unique per function type (same ID can exist in vertex and fragment graphs)
  - Connections must reference nodes within the same function graph — Godot enforces this
- **Function/mode compatibility**: Not all functions are valid for all modes. Godot silently accepts invalid combinations but the nodes won't do anything. The valid combinations are:
  - spatial/canvas_item: vertex, fragment, light
  - particles: start, process, collide, start_custom, process_custom
  - sky: sky
  - fog: fog
  - The GDScript layer does not validate compatibility — Godot handles this gracefully. Invalid functions simply produce empty shader functions.
- **Node ID 0**: The output node. Always exists per function type. Agent connects to it, never creates it.
- **Node ID 1**: Reserved by Godot internally. Agent should not use it.
- **Property setting**: Reuses `NodeOps.convert_value()` for type conversion, same as all other property-setting operations.
- **Position**: Editor graph layout position. Optional — sensible default is `Vector2.ZERO`. Agent can provide positions for readable graph layout.

**Acceptance Criteria**:
- [ ] Empty shader (no nodes, no connections) creates valid .tres
- [ ] Single constant node creates and saves
- [ ] Multiple nodes with connections create valid graph
- [ ] Fragment-function nodes create correctly (e.g. albedo color shader)
- [ ] Mixed vertex + fragment nodes in one call creates multi-function shader
- [ ] Invalid node type returns error
- [ ] Invalid node_id (< 2) returns error
- [ ] Invalid shader_function returns error with valid options listed
- [ ] Invalid connection (bad port) returns error with context
- [ ] Properties on shader nodes are set correctly (e.g. `VisualShaderNodeInput.input_name`)
- [ ] All five shader modes work (spatial, canvas_item, particles, sky, fog)

---

### Unit 8: Dispatcher Registration

Three files need updating to register the new operations.

**File**: `addons/director/operations.gd` (modify existing)

Add import and dispatch entries:

```gdscript
# Add to imports at top:
const PhysicsOps = preload("res://addons/director/ops/physics_ops.gd")
const ShaderOps = preload("res://addons/director/ops/shader_ops.gd")

# Add to match block:
"physics_set_layers":
    result = PhysicsOps.op_physics_set_layers(args.params)
"physics_set_layer_names":
    result = PhysicsOps.op_physics_set_layer_names(args.params)
"visual_shader_create":
    result = ShaderOps.op_visual_shader_create(args.params)
```

**File**: `addons/director/editor_ops.gd` (modify existing)

Add import, SCENE_OPS entry, and dispatch entries:

```gdscript
# Add to imports:
const PhysicsOps = preload("res://addons/director/ops/physics_ops.gd")
const ShaderOps = preload("res://addons/director/ops/shader_ops.gd")

# Add to SCENE_OPS array:
"physics_set_layers",

# Add to _dispatch_live match:
"physics_set_layers":
    return _live_physics_set_layers(params, scene_root)

# Add to _dispatch_headless match:
"physics_set_layers": return PhysicsOps.op_physics_set_layers(params)
"physics_set_layer_names": return PhysicsOps.op_physics_set_layer_names(params)
"visual_shader_create": return ShaderOps.op_visual_shader_create(params)
```

Note: `physics_set_layer_names` and `visual_shader_create` are NOT in SCENE_OPS — they don't target scene nodes. They always run headless (or headless-fallthrough in editor mode).

**File**: `addons/director/daemon.gd` (modify existing)

Add same dispatch entries to the daemon's match block (same pattern as operations.gd).

**File**: `crates/director/src/mcp/mod.rs` (modify existing)

```rust
// Add module:
pub mod physics;
pub mod shader;

// Add imports:
use physics::{PhysicsSetLayersParams, PhysicsSetLayerNamesParams};
use shader::VisualShaderCreateParams;

// Add to #[tool_router] impl:

#[tool(
    name = "physics_set_layers",
    description = "Set collision_layer and/or collision_mask bitmasks on a physics \
        node in a Godot scene. Works with any node that has collision properties \
        (PhysicsBody2D/3D, Area2D/3D, TileMapLayer, etc.). Always use this tool \
        instead of editing .tscn files directly."
)]
pub async fn physics_set_layers(
    &self,
    Parameters(params): Parameters<PhysicsSetLayersParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "physics_set_layers", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "physics_set_layer_names",
    description = "Set human-readable names for physics, render, navigation, or \
        avoidance layers in project.godot. Layer numbers are 1-32. Valid layer types: \
        2d_physics, 3d_physics, 2d_render, 3d_render, 2d_navigation, 3d_navigation, \
        avoidance. Names appear in the editor's layer picker UI."
)]
pub async fn physics_set_layer_names(
    &self,
    Parameters(params): Parameters<PhysicsSetLayerNamesParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "physics_set_layer_names", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "visual_shader_create",
    description = "Create a Godot VisualShader resource (.tres) with a node graph. \
        Define shader nodes and connections as JSON — the graph is built using \
        Godot's VisualShader API. Each node specifies a shader_function (vertex, \
        fragment, light, or particle functions) to target the correct processing \
        stage. Supports spatial (3D), canvas_item (2D), particles, sky, and fog \
        shader modes. Always use this instead of hand-writing shader .tres files."
)]
pub async fn visual_shader_create(
    &self,
    Parameters(params): Parameters<VisualShaderCreateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "visual_shader_create", &op_params).await?;
    serialize_response(&data)
}
```

**Acceptance Criteria**:
- [ ] All three operations reachable via oneshot subprocess
- [ ] All three operations reachable via daemon TCP
- [ ] `physics_set_layers` reachable via editor plugin (live dispatch)
- [ ] `physics_set_layer_names` and `visual_shader_create` use headless fallthrough in editor mode
- [ ] `cargo build -p director` compiles with new modules

---

## Implementation Order

1. **Unit 1 + Unit 2 + Unit 3**: `physics_set_layers` (Rust params + GDScript op + editor dispatch) — simplest tool, validates the physics_ops.gd scaffold
2. **Unit 4 + Unit 5**: `physics_set_layer_names` (Rust params + GDScript op) — adds to the same physics_ops.gd file, no live dispatch needed
3. **Unit 6 + Unit 7**: `visual_shader_create` (Rust params + GDScript op) — most complex, new shader_ops.gd file
4. **Unit 8**: Dispatcher registration — wire everything into operations.gd, editor_ops.gd, daemon.gd, mcp/mod.rs

Units 1-3 and 4-5 can be implemented in parallel since they target different properties in the same file. Unit 8 should be last since it depends on all ops files existing.

---

## Testing

### Test File: `tests/director-tests/src/test_physics.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_collision_layer() {
    // Create scene with CharacterBody2D, set collision_layer = 5
    // Read back via scene_read, verify collision_layer = 5
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_collision_mask() {
    // Create scene with Area3D, set collision_mask = 0xFF
    // Read back, verify
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_both() {
    // Set both collision_layer and collision_mask at once
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_rejects_non_physics_node() {
    // Create scene with Node2D, attempt to set layers → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_rejects_neither_set() {
    // Call with neither collision_layer nor collision_mask → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_writes_project_settings() {
    // Set 2d_physics layer names, then read project.godot to verify
    // Note: reading project.godot after headless Godot writes it
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_rejects_invalid_type() {
    // layer_type = "invalid" → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_rejects_out_of_range() {
    // layer 0 or 33 → expect error
}
```

### Test File: `tests/director-tests/src/test_shader.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_empty() {
    // Create shader with no nodes, no connections
    // Read back via resource_read, verify type = "VisualShader"
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_with_nodes() {
    // Create shader with VisualShaderNodeInput + VisualShaderNodeColorConstant
    // Verify node_count = 2 in response
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_with_connections() {
    // Create shader with two nodes connected
    // Verify connection_count = 1 in response
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_fragment_nodes() {
    // Create spatial shader with VisualShaderNodeColorConstant in fragment graph
    // connected to output node port 0 (albedo)
    // Verify node_count = 1, connection_count = 1
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_mixed_vertex_fragment() {
    // Create spatial shader with nodes in both vertex and fragment graphs
    // Verify total node_count covers both function types
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_shader_function() {
    // shader_function = "invalid" → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_mode() {
    // shader_mode = "invalid" → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_node_type() {
    // node type = "NotAClass" → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_reserved_node_id() {
    // node_id = 0 → expect error
    // node_id = 1 → expect error
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_sets_node_properties() {
    // Create VisualShaderNodeInput with input_name = "vertex"
    // Read back shader, verify the property is set
}
```

### Test Registration: `tests/director-tests/src/lib.rs`

Add:
```rust
#[cfg(test)]
mod test_physics;
#[cfg(test)]
mod test_shader;
```

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Lint
cargo clippy -p director
cargo fmt --check

# Unit tests (no Godot needed)
cargo test -p director

# E2E tests (requires GODOT_BIN)
theatre-deploy ~/dev/stage/tests/godot-project
cargo test -p director-tests -- --include-ignored

# Specific new tests
cargo test -p director-tests test_physics -- --include-ignored
cargo test -p director-tests test_shader -- --include-ignored
```
