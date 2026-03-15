# Design: Phase 6 — Animation

## Overview

Add four MCP tools for creating, reading, and modifying Godot `Animation`
resources (`.tres` files). Animations are standalone resources that can later be
assigned to `AnimationPlayer`/`AnimationLibrary` nodes. This phase covers the
full CRUD lifecycle for animation resources and their tracks/keyframes.

**Tools:** `animation_create`, `animation_add_track`, `animation_read`,
`animation_remove_track`

**Files touched:**
- New: `crates/director/src/mcp/animation.rs` (Rust param structs)
- New: `addons/director/ops/animation_ops.gd` (GDScript operations)
- New: `tests/director-tests/src/test_animation.rs` (E2E tests)
- Modified: `crates/director/src/mcp/mod.rs` (tool handlers + imports)
- Modified: `addons/director/operations.gd` (dispatcher entries)
- Modified: `addons/director/daemon.gd` (dispatcher entries)
- Modified: `tests/director-tests/src/lib.rs` (test module)

---

## Implementation Units

### Unit 1: Rust Parameter Structs

**File**: `crates/director/src/mcp/animation.rs`

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `animation_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the animation resource relative to project
    /// (e.g. "animations/walk.tres").
    pub resource_path: String,

    /// Animation length in seconds.
    pub length: f64,

    /// Loop mode: "none", "linear", or "pingpong". Default: "none".
    #[serde(default)]
    pub loop_mode: Option<String>,

    /// Snap step for keyframe times (seconds). Default: Godot's default (0.0333...).
    #[serde(default)]
    pub step: Option<f64>,
}

/// A single keyframe for `animation_add_track`.
///
/// The required fields depend on track_type:
/// - value: `value` (any JSON value — type-converted per track path), optional `transition`
/// - position_3d, scale_3d: `value` as {x, y, z}
/// - rotation_3d: `value` as {x, y, z, w} (quaternion)
/// - blend_shape: `value` as float
/// - method: `method` (string) and `args` (array)
/// - bezier: `value` (float), optional `in_handle` and `out_handle` as {x, y}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Keyframe {
    /// Time position in seconds.
    pub time: f64,

    /// Keyframe value. Required for value, position_3d, rotation_3d,
    /// scale_3d, blend_shape, and bezier tracks. For method tracks, omit
    /// and use `method`/`args` instead.
    #[serde(default)]
    pub value: Option<serde_json::Value>,

    /// Transition curve for value tracks. 1.0 = linear (default).
    /// <1 = ease in, >1 = ease out. Negative = back/elastic.
    #[serde(default)]
    pub transition: Option<f64>,

    /// Method name (method tracks only).
    #[serde(default)]
    pub method: Option<String>,

    /// Method arguments (method tracks only).
    #[serde(default)]
    pub args: Option<Vec<serde_json::Value>>,

    /// Bezier in-handle as {x, y} (bezier tracks only).
    /// x is time offset (negative), y is value offset.
    #[serde(default)]
    pub in_handle: Option<serde_json::Value>,

    /// Bezier out-handle as {x, y} (bezier tracks only).
    /// x is time offset (positive), y is value offset.
    #[serde(default)]
    pub out_handle: Option<serde_json::Value>,
}

/// Parameters for `animation_add_track`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationAddTrackParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,

    /// Track type: "value", "position_3d", "rotation_3d", "scale_3d",
    /// "blend_shape", "method", or "bezier".
    pub track_type: String,

    /// Node path for the track, relative to the AnimationPlayer.
    /// For value tracks, include the property as a subpath
    /// (e.g. "Sprite2D:modulate", "../Player:position").
    /// For position/rotation/scale tracks, just the node path
    /// (e.g. "Mesh", "../Player").
    pub node_path: String,

    /// Keyframes to insert on the track.
    pub keyframes: Vec<Keyframe>,

    /// Interpolation type: "nearest", "linear", or "cubic".
    /// Default: "linear".
    #[serde(default)]
    pub interpolation: Option<String>,

    /// Update mode for value tracks: "continuous", "discrete",
    /// or "capture". Default: "continuous". Ignored for non-value tracks.
    #[serde(default)]
    pub update_mode: Option<String>,
}

/// Parameters for `animation_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationReadParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,
}

/// Parameters for `animation_remove_track`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationRemoveTrackParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,

    /// Track index to remove. Mutually exclusive with `node_path`.
    #[serde(default)]
    pub track_index: Option<u32>,

    /// Remove all tracks matching this node path. Mutually exclusive
    /// with `track_index`.
    #[serde(default)]
    pub node_path: Option<String>,
}
```

**Implementation Notes:**
- Follows the exact derive pattern from `resource.rs`: `Debug, Deserialize, Serialize, JsonSchema`
- `project_path` is first and required on all structs per convention
- `Keyframe` uses `Option<serde_json::Value>` for `value` because method tracks don't use it
- Track type and loop mode are strings validated on the GDScript side (matches pattern from `style_box_create` which validates against a list)

**Acceptance Criteria:**
- [ ] All four structs compile and produce correct JSON schemas
- [ ] `Keyframe` struct handles all track-type-specific fields as optionals

---

### Unit 2: MCP Tool Handlers

**File**: `crates/director/src/mcp/mod.rs` (additions to existing file)

Add to imports section:
```rust
pub mod animation;

use animation::{
    AnimationAddTrackParams, AnimationCreateParams, AnimationReadParams, AnimationRemoveTrackParams,
};
```

Add to `#[tool_router]` impl block:
```rust
#[tool(
    name = "animation_create",
    description = "Create a Godot Animation resource (.tres) with specified length and \
        loop mode. The animation starts empty — use animation_add_track to add tracks \
        and keyframes. Always use this instead of hand-writing .tres files."
)]
pub async fn animation_create(
    &self,
    Parameters(params): Parameters<AnimationCreateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "animation_create", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "animation_add_track",
    description = "Add a track with keyframes to a Godot Animation resource. Supports \
        value, position_3d, rotation_3d, scale_3d, blend_shape, method, and bezier \
        track types. Node paths are relative to the AnimationPlayer that will play this \
        animation. Always use this instead of editing .tres files directly."
)]
pub async fn animation_add_track(
    &self,
    Parameters(params): Parameters<AnimationAddTrackParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "animation_add_track", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "animation_read",
    description = "Read a Godot Animation resource (.tres) and return its full structure: \
        length, loop mode, and all tracks with their keyframes serialized as JSON. Use \
        this to inspect animation structure before making modifications."
)]
pub async fn animation_read(
    &self,
    Parameters(params): Parameters<AnimationReadParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "animation_read", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "animation_remove_track",
    description = "Remove a track from a Godot Animation resource by index or node path. \
        When removing by node_path, all tracks matching that path are removed. Always \
        use this instead of editing .tres files directly."
)]
pub async fn animation_remove_track(
    &self,
    Parameters(params): Parameters<AnimationRemoveTrackParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "animation_remove_track", &op_params).await?;
    serialize_response(&data)
}
```

**Implementation Notes:**
- Each handler is the standard 4-line pattern: `serialize_params` → `run_operation` → `serialize_response`
- Tool descriptions include anti-direct-edit guidance per spec requirement

**Acceptance Criteria:**
- [ ] All four tools registered in the `#[tool_router]` impl
- [ ] `cargo build -p director` succeeds with all new types imported

---

### Unit 3: GDScript Animation Operations

**File**: `addons/director/ops/animation_ops.gd`

```gdscript
class_name AnimationOps

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")
const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")


# ---- Track type mapping ----

const TRACK_TYPES := {
    "value": Animation.TYPE_VALUE,
    "position_3d": Animation.TYPE_POSITION_3D,
    "rotation_3d": Animation.TYPE_ROTATION_3D,
    "scale_3d": Animation.TYPE_SCALE_3D,
    "blend_shape": Animation.TYPE_BLEND_SHAPE,
    "method": Animation.TYPE_METHOD,
    "bezier": Animation.TYPE_BEZIER,
}

const TRACK_TYPE_NAMES := {}  # Populated by _init_reverse_map()
# Reverse lookup: Animation.TYPE_VALUE → "value", etc.
# Built lazily on first use.

static func _get_track_type_name(godot_type: int) -> String:
    for name in TRACK_TYPES:
        if TRACK_TYPES[name] == godot_type:
            return name
    return "unknown_%d" % godot_type


const LOOP_MODES := {
    "none": Animation.LOOP_NONE,
    "linear": Animation.LOOP_LINEAR,
    "pingpong": Animation.LOOP_PINGPONG,
}

const LOOP_MODE_NAMES := {
    Animation.LOOP_NONE: "none",
    Animation.LOOP_LINEAR: "linear",
    Animation.LOOP_PINGPONG: "pingpong",
}

const INTERPOLATION_TYPES := {
    "nearest": Animation.INTERPOLATION_NEAREST,
    "linear": Animation.INTERPOLATION_LINEAR,
    "cubic": Animation.INTERPOLATION_CUBIC,
}

const UPDATE_MODES := {
    "continuous": Animation.UPDATE_CONTINUOUS,
    "discrete": Animation.UPDATE_DISCRETE,
    "capture": Animation.UPDATE_CAPTURE,
}


# ---- Operations ----

static func op_animation_create(params: Dictionary) -> Dictionary:
    ## Create an Animation resource and save to disk.
    ##
    ## Params: resource_path, length, loop_mode?, step?
    ## Returns: { success, data: { path, length, loop_mode } }

    var resource_path: String = params.get("resource_path", "")
    var length = params.get("length", null)

    if resource_path == "":
        return OpsUtil._error("resource_path is required", "animation_create", params)
    if length == null:
        return OpsUtil._error("length is required", "animation_create", params)
    if float(length) <= 0.0:
        return OpsUtil._error("length must be positive", "animation_create",
            {"resource_path": resource_path, "length": length})

    var anim := Animation.new()
    anim.length = float(length)

    # Loop mode
    var loop_mode_str: String = params.get("loop_mode", "none")
    if not LOOP_MODES.has(loop_mode_str):
        return OpsUtil._error(
            "Invalid loop_mode: " + loop_mode_str +
            ". Must be one of: " + ", ".join(LOOP_MODES.keys()),
            "animation_create", {"loop_mode": loop_mode_str})
    anim.loop_mode = LOOP_MODES[loop_mode_str]

    # Step
    var step = params.get("step", null)
    if step != null:
        anim.step = float(step)

    # Save
    var full_path = "res://" + resource_path
    ResourceOps._ensure_directory(full_path)
    var err = ResourceSaver.save(anim, full_path)
    if err != OK:
        return OpsUtil._error("Failed to save animation: " + str(err),
            "animation_create", {"resource_path": resource_path})

    return {"success": true, "data": {
        "path": resource_path,
        "length": anim.length,
        "loop_mode": loop_mode_str,
    }}


static func op_animation_add_track(params: Dictionary) -> Dictionary:
    ## Add a track with keyframes to an existing Animation resource.
    ##
    ## Params: resource_path, track_type, node_path, keyframes,
    ##         interpolation?, update_mode?
    ## Returns: { success, data: { track_index, keyframe_count } }

    var resource_path: String = params.get("resource_path", "")
    var track_type_str: String = params.get("track_type", "")
    var node_path_str: String = params.get("node_path", "")
    var keyframes = params.get("keyframes", null)

    if resource_path == "":
        return OpsUtil._error("resource_path is required", "animation_add_track", params)
    if track_type_str == "":
        return OpsUtil._error("track_type is required", "animation_add_track", params)
    if node_path_str == "":
        return OpsUtil._error("node_path is required", "animation_add_track", params)
    if keyframes == null or not keyframes is Array or keyframes.is_empty():
        return OpsUtil._error("keyframes must be a non-empty array",
            "animation_add_track", {"resource_path": resource_path})

    if not TRACK_TYPES.has(track_type_str):
        return OpsUtil._error(
            "Invalid track_type: " + track_type_str +
            ". Must be one of: " + ", ".join(TRACK_TYPES.keys()),
            "animation_add_track", {"track_type": track_type_str})

    var full_path = "res://" + resource_path
    if not ResourceLoader.exists(full_path):
        return OpsUtil._error("Animation not found: " + resource_path,
            "animation_add_track", {"resource_path": resource_path})

    var anim: Animation = load(full_path)
    if anim == null or not anim is Animation:
        return OpsUtil._error("Resource is not an Animation: " + resource_path,
            "animation_add_track", {"resource_path": resource_path})

    var track_type: int = TRACK_TYPES[track_type_str]
    var track_idx: int = anim.add_track(track_type)
    anim.track_set_path(track_idx, NodePath(node_path_str))

    # Set interpolation
    var interp_str: String = params.get("interpolation", "linear")
    if not INTERPOLATION_TYPES.has(interp_str):
        return OpsUtil._error(
            "Invalid interpolation: " + interp_str +
            ". Must be one of: " + ", ".join(INTERPOLATION_TYPES.keys()),
            "animation_add_track", {"interpolation": interp_str})
    anim.track_set_interpolation_type(track_idx, INTERPOLATION_TYPES[interp_str])

    # Set update mode (value tracks only)
    if track_type == Animation.TYPE_VALUE:
        var update_str: String = params.get("update_mode", "continuous")
        if not UPDATE_MODES.has(update_str):
            return OpsUtil._error(
                "Invalid update_mode: " + update_str +
                ". Must be one of: " + ", ".join(UPDATE_MODES.keys()),
                "animation_add_track", {"update_mode": update_str})
        anim.value_track_set_update_mode(track_idx, UPDATE_MODES[update_str])

    # Insert keyframes
    var key_count := 0
    for kf in keyframes:
        var result = _insert_keyframe(anim, track_idx, track_type, kf)
        if not result.success:
            return result
        key_count += 1

    # Save
    var err = ResourceSaver.save(anim, full_path)
    if err != OK:
        return OpsUtil._error("Failed to save animation: " + str(err),
            "animation_add_track", {"resource_path": resource_path})

    return {"success": true, "data": {
        "track_index": track_idx,
        "keyframe_count": key_count,
    }}


static func op_animation_read(params: Dictionary) -> Dictionary:
    ## Read an Animation resource and serialize all tracks and keyframes.
    ##
    ## Params: resource_path
    ## Returns: { success, data: { path, length, loop_mode, step, tracks: [...] } }

    var resource_path: String = params.get("resource_path", "")
    if resource_path == "":
        return OpsUtil._error("resource_path is required", "animation_read", params)

    var full_path = "res://" + resource_path
    if not ResourceLoader.exists(full_path):
        return OpsUtil._error("Animation not found: " + resource_path,
            "animation_read", {"resource_path": resource_path})

    var anim: Animation = load(full_path)
    if anim == null or not anim is Animation:
        return OpsUtil._error("Resource is not an Animation: " + resource_path,
            "animation_read", {"resource_path": resource_path})

    var tracks: Array = []
    for i in range(anim.get_track_count()):
        tracks.append(_serialize_track(anim, i))

    return {"success": true, "data": {
        "path": resource_path,
        "length": anim.length,
        "loop_mode": LOOP_MODE_NAMES.get(anim.loop_mode, "none"),
        "step": anim.step,
        "tracks": tracks,
    }}


static func op_animation_remove_track(params: Dictionary) -> Dictionary:
    ## Remove a track from an Animation resource.
    ##
    ## Params: resource_path, track_index? | node_path?
    ## Returns: { success, data: { tracks_removed } }

    var resource_path: String = params.get("resource_path", "")
    var track_index = params.get("track_index", null)
    var node_path_str = params.get("node_path", null)

    if resource_path == "":
        return OpsUtil._error("resource_path is required", "animation_remove_track", params)
    if track_index == null and (node_path_str == null or node_path_str == ""):
        return OpsUtil._error("Either track_index or node_path is required",
            "animation_remove_track", params)
    if track_index != null and node_path_str != null and node_path_str != "":
        return OpsUtil._error("Specify track_index or node_path, not both",
            "animation_remove_track", params)

    var full_path = "res://" + resource_path
    if not ResourceLoader.exists(full_path):
        return OpsUtil._error("Animation not found: " + resource_path,
            "animation_remove_track", {"resource_path": resource_path})

    var anim: Animation = load(full_path)
    if anim == null or not anim is Animation:
        return OpsUtil._error("Resource is not an Animation: " + resource_path,
            "animation_remove_track", {"resource_path": resource_path})

    var removed := 0

    if track_index != null:
        var idx := int(track_index)
        if idx < 0 or idx >= anim.get_track_count():
            return OpsUtil._error(
                "Track index out of range: %d (animation has %d tracks)" % [idx, anim.get_track_count()],
                "animation_remove_track",
                {"track_index": idx, "track_count": anim.get_track_count()})
        anim.remove_track(idx)
        removed = 1
    else:
        # Remove all tracks matching node_path (iterate in reverse to
        # avoid index shift issues).
        var target_path := NodePath(node_path_str)
        for i in range(anim.get_track_count() - 1, -1, -1):
            if anim.track_get_path(i) == target_path:
                anim.remove_track(i)
                removed += 1
        if removed == 0:
            return OpsUtil._error(
                "No tracks found with node_path: " + node_path_str,
                "animation_remove_track",
                {"resource_path": resource_path, "node_path": node_path_str})

    # Save
    var err = ResourceSaver.save(anim, full_path)
    if err != OK:
        return OpsUtil._error("Failed to save animation: " + str(err),
            "animation_remove_track", {"resource_path": resource_path})

    return {"success": true, "data": {"tracks_removed": removed}}


# ---- Keyframe insertion per track type ----

static func _insert_keyframe(anim: Animation, track_idx: int,
        track_type: int, kf: Dictionary) -> Dictionary:
    ## Insert a single keyframe into a track. Format depends on track type.

    var time: float = kf.get("time", 0.0)

    match track_type:
        Animation.TYPE_VALUE:
            var value = kf.get("value", null)
            if value == null:
                return OpsUtil._error("Keyframe missing 'value'",
                    "animation_add_track", {"time": time})
            # No type conversion here — value tracks reference a specific
            # property, but we don't have the target node to query
            # get_property_list(). The value is inserted as-is; Godot will
            # coerce at playback time. For typed values (Vector2, Color, etc.),
            # the caller provides the JSON object format.
            value = _convert_keyframe_value(value)
            var transition: float = kf.get("transition", 1.0)
            anim.track_insert_key(track_idx, time, value, transition)

        Animation.TYPE_POSITION_3D:
            var v = kf.get("value", {})
            if not v is Dictionary:
                return OpsUtil._error("position_3d keyframe value must be {x, y, z}",
                    "animation_add_track", {"time": time})
            var pos := Vector3(v.get("x", 0), v.get("y", 0), v.get("z", 0))
            anim.position_track_insert_key(track_idx, time, pos)

        Animation.TYPE_ROTATION_3D:
            var v = kf.get("value", {})
            if not v is Dictionary:
                return OpsUtil._error("rotation_3d keyframe value must be {x, y, z, w}",
                    "animation_add_track", {"time": time})
            var quat := Quaternion(v.get("x", 0), v.get("y", 0),
                v.get("z", 0), v.get("w", 1))
            anim.rotation_track_insert_key(track_idx, time, quat)

        Animation.TYPE_SCALE_3D:
            var v = kf.get("value", {})
            if not v is Dictionary:
                return OpsUtil._error("scale_3d keyframe value must be {x, y, z}",
                    "animation_add_track", {"time": time})
            var scale := Vector3(v.get("x", 1), v.get("y", 1), v.get("z", 1))
            anim.scale_track_insert_key(track_idx, time, scale)

        Animation.TYPE_BLEND_SHAPE:
            var value = kf.get("value", null)
            if value == null:
                return OpsUtil._error("blend_shape keyframe missing 'value'",
                    "animation_add_track", {"time": time})
            anim.blend_shape_track_insert_key(track_idx, time, float(value))

        Animation.TYPE_METHOD:
            var method_name: String = kf.get("method", "")
            if method_name == "":
                return OpsUtil._error("Method keyframe missing 'method' name",
                    "animation_add_track", {"time": time})
            var args_arr = kf.get("args", [])
            if not args_arr is Array:
                args_arr = []
            # Method track keys are dictionaries with "method" and "args"
            anim.track_insert_key(track_idx, time, {
                "method": method_name,
                "args": args_arr,
            })

        Animation.TYPE_BEZIER:
            var value = kf.get("value", null)
            if value == null:
                return OpsUtil._error("bezier keyframe missing 'value'",
                    "animation_add_track", {"time": time})
            var in_h = kf.get("in_handle", {"x": 0, "y": 0})
            var out_h = kf.get("out_handle", {"x": 0, "y": 0})
            var in_vec := Vector2(in_h.get("x", 0), in_h.get("y", 0))
            var out_vec := Vector2(out_h.get("x", 0), out_h.get("y", 0))
            anim.bezier_track_insert_key(track_idx, time, float(value),
                in_vec, out_vec)

        _:
            return OpsUtil._error("Unsupported track type for keyframe insertion",
                "animation_add_track", {"track_type": track_type})

    return {"success": true}


static func _convert_keyframe_value(value):
    ## Best-effort conversion of JSON keyframe values to Godot types.
    ## For value tracks, we don't have the target node's property list,
    ## so we use heuristics based on the JSON structure.
    if value is Dictionary:
        if value.has("x") and value.has("y"):
            if value.has("z"):
                if value.has("w"):
                    return Quaternion(value.x, value.y, value.z, value.w)
                return Vector3(value.x, value.y, value.z)
            return Vector2(value.x, value.y)
        if value.has("r") and value.has("g") and value.has("b"):
            return Color(value.r, value.g, value.b, value.get("a", 1.0))
    if value is String and value.begins_with("#"):
        return Color.html(value)
    return value


# ---- Track serialization ----

static func _serialize_track(anim: Animation, track_idx: int) -> Dictionary:
    ## Serialize a single track with all its keyframes.
    var track_type := anim.track_get_type(track_idx)
    var type_name := _get_track_type_name(track_type)
    var node_path := str(anim.track_get_path(track_idx))

    var track_data := {
        "track_index": track_idx,
        "type": type_name,
        "node_path": node_path,
        "interpolation": _get_interpolation_name(
            anim.track_get_interpolation_type(track_idx)),
    }

    # Value tracks have update mode
    if track_type == Animation.TYPE_VALUE:
        track_data["update_mode"] = _get_update_mode_name(
            anim.value_track_get_update_mode(track_idx))

    # Serialize keyframes
    var keyframes: Array = []
    var key_count := anim.track_get_key_count(track_idx)
    for k in range(key_count):
        keyframes.append(_serialize_keyframe(anim, track_idx, track_type, k))

    track_data["keyframes"] = keyframes
    return track_data


static func _serialize_keyframe(anim: Animation, track_idx: int,
        track_type: int, key_idx: int) -> Dictionary:
    ## Serialize a single keyframe, format depends on track type.
    var kf := {"time": anim.track_get_key_time(track_idx, key_idx)}

    match track_type:
        Animation.TYPE_VALUE:
            kf["value"] = SceneOps._serialize_value(
                anim.track_get_key_value(track_idx, key_idx))
            var transition := anim.track_get_key_transition(track_idx, key_idx)
            if transition != 1.0:
                kf["transition"] = transition

        Animation.TYPE_POSITION_3D, Animation.TYPE_SCALE_3D:
            var v: Vector3 = anim.track_get_key_value(track_idx, key_idx)
            kf["value"] = {"x": v.x, "y": v.y, "z": v.z}

        Animation.TYPE_ROTATION_3D:
            var q: Quaternion = anim.track_get_key_value(track_idx, key_idx)
            kf["value"] = {"x": q.x, "y": q.y, "z": q.z, "w": q.w}

        Animation.TYPE_BLEND_SHAPE:
            kf["value"] = anim.track_get_key_value(track_idx, key_idx)

        Animation.TYPE_METHOD:
            var name_str: String = anim.method_track_get_name(track_idx, key_idx)
            var args_arr = anim.method_track_get_params(track_idx, key_idx)
            kf["method"] = name_str
            kf["args"] = SceneOps._serialize_value(args_arr)

        Animation.TYPE_BEZIER:
            kf["value"] = anim.bezier_track_get_key_value(track_idx, key_idx)
            var in_h := anim.bezier_track_get_key_in_handle(track_idx, key_idx)
            var out_h := anim.bezier_track_get_key_out_handle(track_idx, key_idx)
            kf["in_handle"] = {"x": in_h.x, "y": in_h.y}
            kf["out_handle"] = {"x": out_h.x, "y": out_h.y}

        _:
            kf["value"] = SceneOps._serialize_value(
                anim.track_get_key_value(track_idx, key_idx))

    return kf


static func _get_interpolation_name(interp: int) -> String:
    match interp:
        Animation.INTERPOLATION_NEAREST: return "nearest"
        Animation.INTERPOLATION_LINEAR: return "linear"
        Animation.INTERPOLATION_CUBIC: return "cubic"
        _: return "linear"


static func _get_update_mode_name(mode: int) -> String:
    match mode:
        Animation.UPDATE_CONTINUOUS: return "continuous"
        Animation.UPDATE_DISCRETE: return "discrete"
        Animation.UPDATE_CAPTURE: return "capture"
        _: return "continuous"
```

**Implementation Notes:**

- `_convert_keyframe_value` uses JSON structure heuristics (`{x,y}` → Vector2,
  `{x,y,z}` → Vector3, `{r,g,b}` → Color) because value tracks don't carry
  the property type info in the animation resource itself. This matches how an
  agent would naturally express these values in JSON.
- `_serialize_track` / `_serialize_keyframe` reuse `SceneOps._serialize_value`
  for serializing arbitrary Godot variants to JSON, avoiding duplication.
- `ResourceOps._ensure_directory` is reused for creating parent dirs.
- Method track keyframes in Godot 4 are stored as `{"method": name, "args": [...]}`
  dictionaries — the GDScript reads them via `method_track_get_name` /
  `method_track_get_params` for clean serialization.
- Transition values are only included in serialized output when non-default
  (≠ 1.0) to reduce noise.
- Track removal by `node_path` iterates in reverse to avoid index shift after
  each `remove_track` call.

**Acceptance Criteria:**
- [ ] `animation_create` creates a valid `.tres` file loadable by Godot
- [ ] `animation_add_track` supports all 7 track types with correct keyframe insertion
- [ ] `animation_read` round-trips: create + add tracks → read returns matching data
- [ ] `animation_remove_track` by index and by node_path both work correctly
- [ ] All operations return structured errors on invalid input

---

### Unit 4: Dispatcher Registration

**File**: `addons/director/operations.gd` — add import and match cases

Add to const imports at top:
```gdscript
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
```

Add to `match args.operation:` block:
```gdscript
        "animation_create":
            result = AnimationOps.op_animation_create(args.params)
        "animation_add_track":
            result = AnimationOps.op_animation_add_track(args.params)
        "animation_read":
            result = AnimationOps.op_animation_read(args.params)
        "animation_remove_track":
            result = AnimationOps.op_animation_remove_track(args.params)
```

**File**: `addons/director/daemon.gd` — same changes

Add to const imports at top:
```gdscript
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
```

Add to `_dispatch` match block:
```gdscript
        "animation_create":
            return AnimationOps.op_animation_create(params)
        "animation_add_track":
            return AnimationOps.op_animation_add_track(params)
        "animation_read":
            return AnimationOps.op_animation_read(params)
        "animation_remove_track":
            return AnimationOps.op_animation_remove_track(params)
```

**Acceptance Criteria:**
- [ ] Both dispatchers route all four animation operations correctly
- [ ] Unknown operations still return the "Unknown operation" error

---

### Unit 5: E2E Tests

**File**: `tests/director-tests/src/test_animation.rs`

```rust
use crate::harness::{assert_approx, DirectorFixture};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_basic() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_basic.tres";
    let data = f
        .run("animation_create", json!({
            "resource_path": path,
            "length": 2.0,
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], path);
    assert_approx(data["length"].as_f64().unwrap(), 2.0);
    assert_eq!(data["loop_mode"], "none");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_with_loop_and_step() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_loop.tres";
    let data = f
        .run("animation_create", json!({
            "resource_path": path,
            "length": 1.5,
            "loop_mode": "linear",
            "step": 0.05,
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["loop_mode"], "linear");

    // Verify via animation_read
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_approx(read["length"].as_f64().unwrap(), 1.5);
    assert_eq!(read["loop_mode"], "linear");
    assert_approx(read["step"].as_f64().unwrap(), 0.05);
    assert!(read["tracks"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_rejects_invalid_loop_mode() {
    let f = DirectorFixture::new();
    let err = f
        .run("animation_create", json!({
            "resource_path": "tmp/bad.tres",
            "length": 1.0,
            "loop_mode": "bounce",
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Invalid loop_mode"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_rejects_zero_length() {
    let f = DirectorFixture::new();
    let err = f
        .run("animation_create", json!({
            "resource_path": "tmp/bad.tres",
            "length": 0.0,
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("length must be positive"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_value_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_value.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let data = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "value",
            "node_path": "Sprite2D:modulate",
            "keyframes": [
                {"time": 0.0, "value": "#ffffff"},
                {"time": 0.5, "value": "#ff0000", "transition": 0.5},
                {"time": 1.0, "value": "#ffffff"},
            ]
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["track_index"], 0);
    assert_eq!(data["keyframe_count"], 3);

    // Read back and verify
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["type"], "value");
    assert_eq!(tracks[0]["node_path"], "Sprite2D:modulate");
    let kfs = tracks[0]["keyframes"].as_array().unwrap();
    assert_eq!(kfs.len(), 3);
    assert_approx(kfs[1]["time"].as_f64().unwrap(), 0.5);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_position_3d_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_pos3d.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 2.0,
    }))
    .unwrap()
    .unwrap_data();

    let data = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "position_3d",
            "node_path": "MeshInstance3D",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                {"time": 1.0, "value": {"x": 5, "y": 2, "z": -3}},
                {"time": 2.0, "value": {"x": 0, "y": 0, "z": 0}},
            ],
            "interpolation": "cubic",
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["track_index"], 0);
    assert_eq!(data["keyframe_count"], 3);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let track = &read["tracks"][0];
    assert_eq!(track["type"], "position_3d");
    assert_eq!(track["interpolation"], "cubic");
    let kf1 = &track["keyframes"][1];
    assert_approx(kf1["value"]["x"].as_f64().unwrap(), 5.0);
    assert_approx(kf1["value"]["y"].as_f64().unwrap(), 2.0);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_method_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_method.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let data = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "method",
            "node_path": "../Player",
            "keyframes": [
                {"time": 0.0, "method": "play_sfx", "args": ["jump"]},
                {"time": 0.5, "method": "set_speed", "args": [2.0]},
            ]
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["keyframe_count"], 2);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let kfs = read["tracks"][0]["keyframes"].as_array().unwrap();
    assert_eq!(kfs[0]["method"], "play_sfx");
    assert_eq!(kfs[1]["method"], "set_speed");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_bezier_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_bezier.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let data = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "bezier",
            "node_path": "Sprite2D:modulate:a",
            "keyframes": [
                {
                    "time": 0.0,
                    "value": 1.0,
                    "in_handle": {"x": -0.5, "y": 0.0},
                    "out_handle": {"x": 0.5, "y": -0.5},
                },
                {
                    "time": 1.0,
                    "value": 0.0,
                    "in_handle": {"x": -0.5, "y": 0.5},
                    "out_handle": {"x": 0.5, "y": 0.0},
                },
            ]
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["keyframe_count"], 2);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let kfs = read["tracks"][0]["keyframes"].as_array().unwrap();
    assert_approx(kfs[0]["value"].as_f64().unwrap(), 1.0);
    assert_approx(kfs[1]["value"].as_f64().unwrap(), 0.0);
    // Verify handles
    assert_approx(kfs[0]["out_handle"]["y"].as_f64().unwrap(), -0.5);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_multiple_tracks() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_multi.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 2.0,
    }))
    .unwrap()
    .unwrap_data();

    // Add position track
    let d1 = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "position_3d",
            "node_path": "Mesh",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                {"time": 2.0, "value": {"x": 10, "y": 0, "z": 0}},
            ]
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(d1["track_index"], 0);

    // Add rotation track
    let d2 = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "rotation_3d",
            "node_path": "Mesh",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0, "w": 1}},
                {"time": 2.0, "value": {"x": 0, "y": 0.707, "z": 0, "w": 0.707}},
            ]
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(d2["track_index"], 1);

    // Read and verify both tracks
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["type"], "position_3d");
    assert_eq!(tracks[1]["type"], "rotation_3d");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_by_index() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_idx.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    // Add two tracks
    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "value",
        "node_path": "Sprite:position",
        "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0}}],
    }))
    .unwrap()
    .unwrap_data();

    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "value",
        "node_path": "Sprite:scale",
        "keyframes": [{"time": 0.0, "value": {"x": 1, "y": 1}}],
    }))
    .unwrap()
    .unwrap_data();

    // Remove first track by index
    let data = f
        .run("animation_remove_track", json!({
            "resource_path": path,
            "track_index": 0,
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["tracks_removed"], 1);

    // Verify only scale track remains
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["node_path"], "Sprite:scale");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_by_node_path() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_path.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    // Add two tracks on same node path, one on different
    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "position_3d",
        "node_path": "Enemy",
        "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}}],
    }))
    .unwrap()
    .unwrap_data();

    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "rotation_3d",
        "node_path": "Enemy",
        "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0, "w": 1}}],
    }))
    .unwrap()
    .unwrap_data();

    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "position_3d",
        "node_path": "Player",
        "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}}],
    }))
    .unwrap()
    .unwrap_data();

    // Remove all Enemy tracks by path
    let data = f
        .run("animation_remove_track", json!({
            "resource_path": path,
            "node_path": "Enemy",
        }))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["tracks_removed"], 2);

    // Verify only Player track remains
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["node_path"], "Player");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_out_of_range() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_oor.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let err = f
        .run("animation_remove_track", json!({
            "resource_path": path,
            "track_index": 5,
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_no_match() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_nomatch.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let err = f
        .run("animation_remove_track", json!({
            "resource_path": path,
            "node_path": "Nonexistent",
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("No tracks found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_track_rejects_invalid_type() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_bad_type.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    let err = f
        .run("animation_add_track", json!({
            "resource_path": path,
            "track_type": "audio",
            "node_path": "Player",
            "keyframes": [{"time": 0.0, "value": 1.0}],
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Invalid track_type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_value_track_discrete() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_discrete.tres";
    f.run("animation_create", json!({
        "resource_path": path,
        "length": 1.0,
    }))
    .unwrap()
    .unwrap_data();

    f.run("animation_add_track", json!({
        "resource_path": path,
        "track_type": "value",
        "node_path": "Sprite:frame",
        "update_mode": "discrete",
        "keyframes": [
            {"time": 0.0, "value": 0},
            {"time": 0.5, "value": 1},
            {"time": 1.0, "value": 2},
        ],
    }))
    .unwrap()
    .unwrap_data();

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["tracks"][0]["update_mode"], "discrete");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_read_not_found() {
    let f = DirectorFixture::new();
    let err = f
        .run("animation_read", json!({
            "resource_path": "nonexistent.tres",
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_read_non_animation_resource() {
    let f = DirectorFixture::new();
    // Create a material (not an animation)
    f.run("material_create", json!({
        "resource_path": "tmp/not_an_anim.tres",
        "material_type": "StandardMaterial3D",
    }))
    .unwrap()
    .unwrap_data();

    let err = f
        .run("animation_read", json!({
            "resource_path": "tmp/not_an_anim.tres",
        }))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not an Animation"));
}
```

**File**: `tests/director-tests/src/lib.rs` — add module:
```rust
#[cfg(test)]
mod test_animation;
```

**Acceptance Criteria:**
- [ ] All tests pass with `cargo test -p director-tests -- --include-ignored`
- [ ] Tests cover: create, create with options, all track types (value, position_3d,
  rotation_3d, method, bezier), multiple tracks, remove by index, remove by path,
  error cases (invalid loop mode, invalid track type, out of range, not found)

---

## Implementation Order

1. **Unit 1: Rust param structs** (`animation.rs`) — no dependencies, compiles standalone
2. **Unit 2: MCP tool handlers** (`mod.rs`) — depends on Unit 1
3. **Unit 3: GDScript operations** (`animation_ops.gd`) — no Rust dependency, can be parallel with Units 1-2
4. **Unit 4: Dispatcher registration** (`operations.gd`, `daemon.gd`) — depends on Unit 3
5. **Unit 5: E2E tests** (`test_animation.rs`, `lib.rs`) — depends on all above

Units 1-2 (Rust) and Unit 3 (GDScript) can be implemented in parallel.

---

## Testing

### E2E Tests: `tests/director-tests/src/test_animation.rs`

16 test cases covering:

| Test | What it verifies |
|---|---|
| `animation_create_basic` | Minimal create with defaults |
| `animation_create_with_loop_and_step` | Loop mode + step + round-trip via read |
| `animation_create_rejects_invalid_loop_mode` | Error on bad loop_mode |
| `animation_create_rejects_zero_length` | Error on non-positive length |
| `animation_add_value_track` | Value track with Color keyframes + transition |
| `animation_add_position_3d_track` | Position 3D track with cubic interpolation |
| `animation_add_method_track` | Method track with name + args |
| `animation_add_bezier_track` | Bezier track with handles |
| `animation_add_multiple_tracks` | Two tracks on same animation, correct indices |
| `animation_add_value_track_discrete` | Update mode on value tracks |
| `animation_add_track_rejects_invalid_type` | Error on unsupported track type |
| `animation_remove_track_by_index` | Remove by index, verify remaining |
| `animation_remove_track_by_node_path` | Remove all matching path, verify remaining |
| `animation_remove_track_out_of_range` | Error on bad index |
| `animation_remove_track_no_match` | Error when no tracks match path |
| `animation_read_not_found` | Error on missing resource |
| `animation_read_non_animation_resource` | Error when resource is not Animation |

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Clippy
cargo clippy -p director

# Deploy to test project
theatre-deploy ~/dev/stage/tests/godot-project

# Run all tests (includes animation E2E)
cargo test -p director-tests -- --include-ignored

# Run only animation tests
cargo test -p director-tests -- --include-ignored test_animation
```
