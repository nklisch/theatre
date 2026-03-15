# Design: Director Phase 4 — Resources & Materials

## Overview

Phase 4 adds four resource creation tools to Director: `material_create`,
`shape_create`, `style_box_create`, and `resource_duplicate`. These are the
first tools that create standalone `.tres` resource files (as opposed to
modifying scenes). `shape_create` also supports attaching directly to a
CollisionShape node in a scene.

All four follow the same pattern: GDScript creates the resource via ClassDB,
sets properties, and saves via ResourceSaver. The Rust layer is a thin
pass-through as with all existing tools.

**Design decisions (confirmed with user):**
- Materials: 5 named types + ClassDB fallback for any Material subclass
- Shapes: 2D and 3D, require at least one of `save_path` / scene attachment
- StyleBox: all 4 types (Flat, Texture, Line, Empty)
- ShaderMaterial: accepts `shader_path` to load .gdshader during creation
- resource_duplicate: shallow copy default, optional `deep_copy` param

---

## Implementation Units

### Unit 1: Rust Parameter Structs

**File**: `crates/director/src/mcp/resource.rs`

Add to the existing file alongside `ResourceReadParams`:

```rust
/// Parameters for `material_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MaterialCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the material relative to project (e.g. "materials/ground.tres").
    pub resource_path: String,

    /// Material class name. Common types: StandardMaterial3D, ORMMaterial3D,
    /// ShaderMaterial, CanvasItemMaterial, ParticleProcessMaterial.
    /// Any ClassDB Material subclass is accepted.
    pub material_type: String,

    /// Optional properties to set on the material after creation.
    /// Type conversion is automatic (Color from "#ff0000" or {"r":1,"g":0,"b":0}).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,

    /// For ShaderMaterial: path to a .gdshader file (relative to project).
    /// Loaded and assigned as the shader property.
    #[serde(default)]
    pub shader_path: Option<String>,
}

/// Parameters for `shape_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ShapeCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Shape class name. 3D: BoxShape3D, SphereShape3D, CapsuleShape3D,
    /// CylinderShape3D, ConcavePolygonShape3D, ConvexPolygonShape3D,
    /// WorldBoundaryShape3D, SeparationRayShape3D, HeightMapShape3D.
    /// 2D: CircleShape2D, RectangleShape2D, CapsuleShape2D, SegmentShape2D,
    /// ConvexPolygonShape2D, ConcavePolygonShape2D, WorldBoundaryShape2D,
    /// SeparationRayShape2D.
    pub shape_type: String,

    /// Shape configuration. Keys are property names on the shape resource
    /// (e.g. "radius", "size", "height"). Type conversion is automatic.
    #[serde(default)]
    pub shape_params: Option<serde_json::Map<String, serde_json::Value>>,

    /// Save the shape as a .tres file at this path (relative to project).
    #[serde(default)]
    pub save_path: Option<String>,

    /// Attach the shape to a CollisionShape2D/3D node in a scene.
    /// Requires scene_path and node_path to also be set.
    /// The shape is assigned to the node's "shape" property.
    #[serde(default)]
    pub scene_path: Option<String>,

    /// Path to the CollisionShape node within the scene tree.
    /// Required when scene_path is set.
    #[serde(default)]
    pub node_path: Option<String>,
}

/// Parameters for `style_box_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct StyleBoxCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the StyleBox relative to project (e.g. "ui/panel.tres").
    pub resource_path: String,

    /// StyleBox class name: StyleBoxFlat, StyleBoxTexture, StyleBoxLine,
    /// or StyleBoxEmpty.
    pub style_type: String,

    /// Optional properties to set on the StyleBox after creation.
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parameters for `resource_duplicate`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResourceDuplicateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the source resource file (relative to project).
    pub source_path: String,

    /// Path where the duplicate will be saved (relative to project).
    pub dest_path: String,

    /// Property overrides to apply after duplication. Keys are property names.
    #[serde(default)]
    pub property_overrides: Option<serde_json::Map<String, serde_json::Value>>,

    /// Deep copy sub-resources (making them independent). Default: false
    /// (shallow copy — sub-resources are shared references).
    #[serde(default)]
    pub deep_copy: Option<bool>,
}
```

**Acceptance Criteria:**
- [ ] All five structs derive `Debug, Deserialize, Serialize, JsonSchema`
- [ ] `project_path` is required on all structs
- [ ] Optional fields use `Option<T>` with `#[serde(default)]`
- [ ] Doc comments match MCP tool description style from existing structs

---

### Unit 2: Rust MCP Tool Handlers

**File**: `crates/director/src/mcp/mod.rs`

Add imports at the top:

```rust
use resource::{
    MaterialCreateParams, ResourceDuplicateParams, ResourceReadParams,
    ShapeCreateParams, StyleBoxCreateParams,
};
```

Add four new tool handlers inside the existing `#[tool_router]` impl block:

```rust
#[tool(
    name = "material_create",
    description = "Create a Godot material resource (.tres). Supports StandardMaterial3D, \
        ORMMaterial3D, ShaderMaterial, CanvasItemMaterial, ParticleProcessMaterial, and \
        any ClassDB Material subclass. Always use this instead of hand-writing .tres files."
)]
pub async fn material_create(
    &self,
    Parameters(params): Parameters<MaterialCreateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "material_create", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "shape_create",
    description = "Create a Godot collision shape resource. Supports 3D shapes (BoxShape3D, \
        SphereShape3D, CapsuleShape3D, etc.) and 2D shapes (CircleShape2D, RectangleShape2D, \
        etc.). Can save as .tres and/or attach directly to a CollisionShape node in a scene. \
        At least one of save_path or scene attachment (scene_path + node_path) is required."
)]
pub async fn shape_create(
    &self,
    Parameters(params): Parameters<ShapeCreateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "shape_create", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "style_box_create",
    description = "Create a Godot StyleBox resource (.tres) for UI theming. Supports \
        StyleBoxFlat, StyleBoxTexture, StyleBoxLine, and StyleBoxEmpty. Always use this \
        instead of hand-writing .tres files."
)]
pub async fn style_box_create(
    &self,
    Parameters(params): Parameters<StyleBoxCreateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "style_box_create", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "resource_duplicate",
    description = "Duplicate a Godot resource file (.tres, .res) to a new path, optionally \
        applying property overrides. Use deep_copy to make nested sub-resources independent."
)]
pub async fn resource_duplicate(
    &self,
    Parameters(params): Parameters<ResourceDuplicateParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "resource_duplicate", &op_params).await?;
    serialize_response(&data)
}
```

**Acceptance Criteria:**
- [ ] All four handlers follow the exact `serialize_params` → `run_operation` → `serialize_response` pattern
- [ ] Tool descriptions include anti-direct-edit guidance
- [ ] Compiles with `cargo build -p director`

---

### Unit 3: GDScript Resource Operations

**File**: `addons/director/ops/resource_ops.gd`

Extend the existing `ResourceOps` class with four new static functions. The
type conversion helper from `NodeOps.convert_value` is reused for setting
properties on resources.

```gdscript
static func op_material_create(params: Dictionary) -> Dictionary:
    ## Create a material resource and save to disk.
    ##
    ## Params: resource_path, material_type, properties?, shader_path?
    ## Returns: { success, data: { path, type } }

    var resource_path: String = params.get("resource_path", "")
    var material_type: String = params.get("material_type", "")

    if resource_path == "":
        return _error("resource_path is required", "material_create", params)
    if material_type == "":
        return _error("material_type is required", "material_create", params)
    if not ClassDB.class_exists(material_type):
        return _error("Unknown class: " + material_type, "material_create",
            {"material_type": material_type})
    if not ClassDB.is_parent_class(material_type, "Material"):
        return _error(material_type + " is not a Material subclass",
            "material_create", {"material_type": material_type})

    var material = ClassDB.instantiate(material_type)

    # Handle ShaderMaterial shader_path
    var shader_path: String = params.get("shader_path", "")
    if shader_path != "":
        if material_type != "ShaderMaterial":
            material.free()
            return _error("shader_path is only valid for ShaderMaterial",
                "material_create", {"material_type": material_type})
        var full_shader = "res://" + shader_path
        if not ResourceLoader.exists(full_shader):
            material.free()
            return _error("Shader not found: " + shader_path,
                "material_create", {"shader_path": shader_path})
        material.shader = load(full_shader)

    # Set properties
    var properties = params.get("properties", null)
    if properties is Dictionary and not properties.is_empty():
        var result = _set_properties_on_resource(material, properties)
        if not result.success:
            material.free()
            return result

    # Save
    var full_path = "res://" + resource_path
    _ensure_directory(full_path)
    var err = ResourceSaver.save(material, full_path)
    material.free()
    if err != OK:
        return _error("Failed to save material: " + str(err),
            "material_create", {"resource_path": resource_path})

    return {"success": true, "data": {"path": resource_path, "type": material_type}}


static func op_shape_create(params: Dictionary) -> Dictionary:
    ## Create a collision shape and save/attach it.
    ##
    ## Params: shape_type, shape_params?, save_path?, scene_path?, node_path?
    ## Returns: { success, data: { shape_type, saved_to?, attached_to? } }

    var shape_type: String = params.get("shape_type", "")
    var save_path: String = params.get("save_path", "")
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")

    if shape_type == "":
        return _error("shape_type is required", "shape_create", params)
    if save_path == "" and scene_path == "":
        return _error("At least one of save_path or scene_path is required",
            "shape_create", params)
    if scene_path != "" and node_path == "":
        return _error("node_path is required when scene_path is set",
            "shape_create", {"scene_path": scene_path})

    if not ClassDB.class_exists(shape_type):
        return _error("Unknown class: " + shape_type, "shape_create",
            {"shape_type": shape_type})
    # Accept both Shape2D and Shape3D subclasses
    if not (ClassDB.is_parent_class(shape_type, "Shape3D") or
            ClassDB.is_parent_class(shape_type, "Shape2D")):
        return _error(shape_type + " is not a Shape2D or Shape3D subclass",
            "shape_create", {"shape_type": shape_type})

    var shape = ClassDB.instantiate(shape_type)

    # Set shape params
    var shape_params = params.get("shape_params", null)
    if shape_params is Dictionary and not shape_params.is_empty():
        var result = _set_properties_on_resource(shape, shape_params)
        if not result.success:
            shape.free()
            return result

    var data: Dictionary = {"shape_type": shape_type}

    # Save to file if requested
    if save_path != "":
        var full_save = "res://" + save_path
        _ensure_directory(full_save)
        var err = ResourceSaver.save(shape, full_save)
        if err != OK:
            shape.free()
            return _error("Failed to save shape: " + str(err),
                "shape_create", {"save_path": save_path})
        data["saved_to"] = save_path

    # Attach to scene node if requested
    if scene_path != "":
        var full_scene = "res://" + scene_path
        if not ResourceLoader.exists(full_scene):
            shape.free()
            return _error("Scene not found: " + scene_path,
                "shape_create", {"scene_path": scene_path})
        var packed: PackedScene = load(full_scene)
        var root = packed.instantiate()
        var target = root.get_node_or_null(node_path)
        if target == null:
            root.free()
            shape.free()
            return _error("Node not found: " + node_path,
                "shape_create", {"scene_path": scene_path, "node_path": node_path})

        # Verify the node has a "shape" property
        var has_shape_prop := false
        for prop_info in target.get_property_list():
            if prop_info["name"] == "shape":
                has_shape_prop = true
                break
        if not has_shape_prop:
            root.free()
            shape.free()
            return _error("Node " + node_path + " (" + target.get_class() +
                ") has no 'shape' property",
                "shape_create", {"node_path": node_path, "class": target.get_class()})

        target.shape = shape
        var save_result = NodeOps._repack_and_save(root, full_scene)
        root.free()
        if not save_result.success:
            shape.free()
            return save_result
        data["attached_to"] = node_path

    shape.free()
    return {"success": true, "data": data}


static func op_style_box_create(params: Dictionary) -> Dictionary:
    ## Create a StyleBox resource and save to disk.
    ##
    ## Params: resource_path, style_type, properties?
    ## Returns: { success, data: { path, type } }

    var resource_path: String = params.get("resource_path", "")
    var style_type: String = params.get("style_type", "")

    if resource_path == "":
        return _error("resource_path is required", "style_box_create", params)
    if style_type == "":
        return _error("style_type is required", "style_box_create", params)

    var valid_types = ["StyleBoxFlat", "StyleBoxTexture", "StyleBoxLine", "StyleBoxEmpty"]
    if not style_type in valid_types:
        return _error("Invalid style_type: " + style_type +
            ". Must be one of: " + ", ".join(valid_types),
            "style_box_create", {"style_type": style_type})

    var style_box = ClassDB.instantiate(style_type)

    var properties = params.get("properties", null)
    if properties is Dictionary and not properties.is_empty():
        var result = _set_properties_on_resource(style_box, properties)
        if not result.success:
            style_box.free()
            return result

    var full_path = "res://" + resource_path
    _ensure_directory(full_path)
    var err = ResourceSaver.save(style_box, full_path)
    style_box.free()
    if err != OK:
        return _error("Failed to save style box: " + str(err),
            "style_box_create", {"resource_path": resource_path})

    return {"success": true, "data": {"path": resource_path, "type": style_type}}


static func op_resource_duplicate(params: Dictionary) -> Dictionary:
    ## Duplicate a resource file to a new path, optionally overriding properties.
    ##
    ## Params: source_path, dest_path, property_overrides?, deep_copy?
    ## Returns: { success, data: { path, type, overrides_applied: [] } }

    var source_path: String = params.get("source_path", "")
    var dest_path: String = params.get("dest_path", "")
    var deep_copy: bool = params.get("deep_copy", false)

    if source_path == "":
        return _error("source_path is required", "resource_duplicate", params)
    if dest_path == "":
        return _error("dest_path is required", "resource_duplicate", params)

    var full_source = "res://" + source_path
    if not ResourceLoader.exists(full_source):
        return _error("Source resource not found: " + source_path,
            "resource_duplicate", {"source_path": source_path})

    var source = load(full_source)
    if source == null:
        return _error("Failed to load source resource: " + source_path,
            "resource_duplicate", {"source_path": source_path})

    var duplicate = source.duplicate(deep_copy)
    var overrides_applied: Array = []

    var property_overrides = params.get("property_overrides", null)
    if property_overrides is Dictionary and not property_overrides.is_empty():
        var result = _set_properties_on_resource(duplicate, property_overrides)
        if not result.success:
            return result
        overrides_applied = result.properties_set

    var full_dest = "res://" + dest_path
    _ensure_directory(full_dest)
    var err = ResourceSaver.save(duplicate, full_dest)
    if err != OK:
        return _error("Failed to save duplicate: " + str(err),
            "resource_duplicate", {"dest_path": dest_path})

    return {"success": true, "data": {
        "path": dest_path,
        "type": duplicate.get_class(),
        "overrides_applied": overrides_applied,
    }}
```

**Shared helper functions** (also in `resource_ops.gd`):

```gdscript
static func _set_properties_on_resource(resource: Resource, properties: Dictionary) -> Dictionary:
    ## Set multiple properties on a resource with type conversion.
    ## Mirrors NodeOps._set_properties_on_node but for Resource instead of Node.
    var properties_set: Array = []
    var prop_list = resource.get_property_list()
    var type_map: Dictionary = {}
    for prop_info in prop_list:
        type_map[prop_info["name"]] = prop_info["type"]

    for prop_name in properties:
        var value = properties[prop_name]
        if not type_map.has(prop_name):
            return {"success": false, "error": "Unknown property: " + prop_name +
                " on " + resource.get_class(), "operation": "set_properties",
                "context": {"resource": resource.get_class(), "property": prop_name}}
        var expected_type = type_map[prop_name]
        var converted = NodeOps.convert_value(value, expected_type)
        resource.set(prop_name, converted)
        properties_set.append(prop_name)

    return {"success": true, "properties_set": properties_set}


static func _ensure_directory(full_path: String) -> void:
    ## Create parent directories for a resource path if they don't exist.
    var dir_path = full_path.get_base_dir()
    if not DirAccess.dir_exists_absolute(dir_path):
        DirAccess.make_dir_recursive_absolute(dir_path)
```

**Implementation Notes:**
- `_set_properties_on_resource` reuses `NodeOps.convert_value` for type
  conversion. This keeps the conversion logic in one place.
- `_ensure_directory` is needed because `ResourceSaver.save` fails if the
  parent directory doesn't exist (unlike scene creation which uses an existing
  scene path).
- `shape_create` with attachment reuses `NodeOps._repack_and_save` to save the
  modified scene. This cross-class call is fine since both are static.
- For `op_shape_create`, the `shape.free()` at the end is safe because
  `ResourceSaver.save()` and `target.shape = shape` both create their own
  internal references.

**Acceptance Criteria:**
- [ ] All four ops return `{success: true, data: {...}}` on success
- [ ] All four ops return `{success: false, error: "...", operation: "...", context: {...}}` on every error path
- [ ] `material_create` validates material_type via ClassDB
- [ ] `material_create` loads shader for ShaderMaterial when shader_path provided
- [ ] `shape_create` errors if neither save_path nor scene_path is set
- [ ] `shape_create` errors if scene_path is set without node_path
- [ ] `shape_create` validates the target node has a `shape` property
- [ ] `style_box_create` validates against the four allowed types
- [ ] `resource_duplicate` passes `deep_copy` to `Resource.duplicate()`
- [ ] `_set_properties_on_resource` reuses `NodeOps.convert_value`

---

### Unit 4: Dispatcher Updates

**File**: `addons/director/operations.gd`

Add four new match arms in `_init()`:

```gdscript
"material_create":
    result = ResourceOps.op_material_create(args.params)
"shape_create":
    result = ResourceOps.op_shape_create(args.params)
"style_box_create":
    result = ResourceOps.op_style_box_create(args.params)
"resource_duplicate":
    result = ResourceOps.op_resource_duplicate(args.params)
```

**File**: `addons/director/daemon.gd`

Add the same four match arms in `_dispatch()`:

```gdscript
"material_create":
    return ResourceOps.op_material_create(params)
"shape_create":
    return ResourceOps.op_shape_create(params)
"style_box_create":
    return ResourceOps.op_style_box_create(params)
"resource_duplicate":
    return ResourceOps.op_resource_duplicate(params)
```

**Implementation Notes:**
- `NodeOps` is already loaded via `preload` in both dispatchers for
  cross-class access to `convert_value` and `_repack_and_save`. No new
  preload needed — `ResourceOps` already references `NodeOps` by class_name.

**Acceptance Criteria:**
- [ ] Both dispatchers route all four new operations
- [ ] Unknown operation still returns the existing error response

---

### Unit 5: E2E Tests

**File**: `tests/director-tests/src/test_resource_create.rs` (new file)

```rust
use serde_json::json;
use crate::harness::DirectorFixture;

#[test]
#[ignore = "requires Godot binary"]
fn material_create_standard_material_3d() {
    let f = DirectorFixture::new();
    let path = "tmp/test_mat_standard.tres";
    let data = f.run("material_create", json!({
        "resource_path": path,
        "material_type": "StandardMaterial3D",
        "properties": {
            "albedo_color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
            "metallic": 0.8
        }
    })).unwrap().unwrap_data();
    assert_eq!(data["path"], path);
    assert_eq!(data["type"], "StandardMaterial3D");

    // Verify via resource_read
    let read = f.run("resource_read", json!({
        "resource_path": path
    })).unwrap().unwrap_data();
    assert_eq!(read["type"], "StandardMaterial3D");
    assert_eq!(read["properties"]["metallic"], 0.8);
}

#[test]
#[ignore = "requires Godot binary"]
fn material_create_rejects_non_material() {
    let f = DirectorFixture::new();
    let err = f.run("material_create", json!({
        "resource_path": "tmp/bad.tres",
        "material_type": "Node2D"
    })).unwrap().unwrap_err();
    assert!(err.contains("not a Material subclass"));
}

#[test]
#[ignore = "requires Godot binary"]
fn material_create_shader_material_with_shader_path() {
    // This test requires a .gdshader file to exist in the test project.
    // Create a minimal shader first (or skip if not feasible).
    let f = DirectorFixture::new();
    let data = f.run("material_create", json!({
        "resource_path": "tmp/test_shader_mat.tres",
        "material_type": "ShaderMaterial",
        "shader_path": "test_shader.gdshader"
    })).unwrap();
    // If the shader file doesn't exist, this will error — that's fine,
    // it validates the path-checking logic.
    // If it does exist, verify success.
    if data.success {
        assert_eq!(data.data["type"], "ShaderMaterial");
    } else {
        assert!(data.error.unwrap().contains("Shader not found"));
    }
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_save_to_file() {
    let f = DirectorFixture::new();
    let path = "tmp/test_box_shape.tres";
    let data = f.run("shape_create", json!({
        "shape_type": "BoxShape3D",
        "shape_params": {"size": {"x": 2.0, "y": 3.0, "z": 4.0}},
        "save_path": path
    })).unwrap().unwrap_data();
    assert_eq!(data["shape_type"], "BoxShape3D");
    assert_eq!(data["saved_to"], path);

    // Verify via resource_read
    let read = f.run("resource_read", json!({
        "resource_path": path
    })).unwrap().unwrap_data();
    assert_eq!(read["type"], "BoxShape3D");
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_attach_to_collision_node() {
    let f = DirectorFixture::new();
    // Create a scene with a CollisionShape3D
    let scene = "tmp/test_shape_attach.tscn";
    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "StaticBody3D"
    })).unwrap().unwrap_data();
    f.run("node_add", json!({
        "scene_path": scene, "node_type": "CollisionShape3D",
        "node_name": "Collision"
    })).unwrap().unwrap_data();

    // Attach a shape
    let data = f.run("shape_create", json!({
        "shape_type": "SphereShape3D",
        "shape_params": {"radius": 2.5},
        "scene_path": scene,
        "node_path": "Collision"
    })).unwrap().unwrap_data();
    assert_eq!(data["shape_type"], "SphereShape3D");
    assert_eq!(data["attached_to"], "Collision");

    // Verify via scene_read
    let read = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();
    let collision = &read["root"]["children"][0];
    assert_eq!(collision["name"], "Collision");
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_2d() {
    let f = DirectorFixture::new();
    let path = "tmp/test_circle_shape.tres";
    let data = f.run("shape_create", json!({
        "shape_type": "CircleShape2D",
        "shape_params": {"radius": 50.0},
        "save_path": path
    })).unwrap().unwrap_data();
    assert_eq!(data["shape_type"], "CircleShape2D");
    assert_eq!(data["saved_to"], path);
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_rejects_no_output() {
    let f = DirectorFixture::new();
    let err = f.run("shape_create", json!({
        "shape_type": "BoxShape3D"
    })).unwrap().unwrap_err();
    assert!(err.contains("At least one of save_path or scene_path"));
}

#[test]
#[ignore = "requires Godot binary"]
fn style_box_create_flat() {
    let f = DirectorFixture::new();
    let path = "tmp/test_stylebox.tres";
    let data = f.run("style_box_create", json!({
        "resource_path": path,
        "style_type": "StyleBoxFlat",
        "properties": {
            "bg_color": "#336699",
            "corner_radius_top_left": 8,
            "corner_radius_top_right": 8
        }
    })).unwrap().unwrap_data();
    assert_eq!(data["path"], path);
    assert_eq!(data["type"], "StyleBoxFlat");
}

#[test]
#[ignore = "requires Godot binary"]
fn style_box_create_rejects_invalid_type() {
    let f = DirectorFixture::new();
    let err = f.run("style_box_create", json!({
        "resource_path": "tmp/bad.tres",
        "style_type": "StyleBoxFancy"
    })).unwrap().unwrap_err();
    assert!(err.contains("Invalid style_type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_duplicate_shallow() {
    let f = DirectorFixture::new();
    // Create source material
    f.run("material_create", json!({
        "resource_path": "tmp/dup_source.tres",
        "material_type": "StandardMaterial3D",
        "properties": {"metallic": 0.5}
    })).unwrap().unwrap_data();

    // Duplicate with override
    let data = f.run("resource_duplicate", json!({
        "source_path": "tmp/dup_source.tres",
        "dest_path": "tmp/dup_dest.tres",
        "property_overrides": {"metallic": 0.9}
    })).unwrap().unwrap_data();
    assert_eq!(data["path"], "tmp/dup_dest.tres");
    assert_eq!(data["type"], "StandardMaterial3D");
    assert!(data["overrides_applied"].as_array().unwrap().contains(&json!("metallic")));

    // Verify override took effect
    let read = f.run("resource_read", json!({
        "resource_path": "tmp/dup_dest.tres"
    })).unwrap().unwrap_data();
    assert_eq!(read["properties"]["metallic"], 0.9);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_duplicate_not_found() {
    let f = DirectorFixture::new();
    let err = f.run("resource_duplicate", json!({
        "source_path": "nonexistent.tres",
        "dest_path": "tmp/dup_out.tres"
    })).unwrap().unwrap_err();
    assert!(err.contains("not found"));
}
```

**File**: `tests/director-tests/src/lib.rs`

Add module declaration:

```rust
mod test_resource_create;
```

**Acceptance Criteria:**
- [ ] All tests use `#[ignore = "requires Godot binary"]`
- [ ] Tests cover success and error paths for all four operations
- [ ] Tests verify round-trip (create → read back)
- [ ] `shape_create` attachment test creates a scene, adds CollisionShape, then attaches
- [ ] `resource_duplicate` test verifies property overrides took effect

---

## Implementation Order

1. **Unit 3: GDScript operations** — the operations themselves. Can be tested
   immediately via `godot --headless --script operations.gd` from the command
   line.
2. **Unit 4: Dispatcher updates** — wire the new ops into both dispatchers.
3. **Unit 1: Rust param structs** — defines the MCP schema.
4. **Unit 2: Rust tool handlers** — wires the MCP tools to the backend.
5. **Unit 5: E2E tests** — validates end-to-end.

Units 1-4 can be implemented together in practice since they're all small.
Unit 5 should be written last so it validates the full stack.

---

## Testing

### E2E Tests: `tests/director-tests/src/test_resource_create.rs`

11 test cases total:
- 3 for `material_create` (success, rejection, shader_path)
- 4 for `shape_create` (save, attach, 2D, rejection)
- 2 for `style_box_create` (success, rejection)
- 2 for `resource_duplicate` (with override, not found)

All tests use `DirectorFixture` from the existing harness. No new test
infrastructure needed.

### Test data

Tests create resources in the `tmp/` directory within the test project. The
`shape_create` attachment test needs to first create a scene and add a
CollisionShape3D node — this is done inline using existing `scene_create` and
`node_add` operations.

The `shader_path` test for ShaderMaterial may need a `.gdshader` file in the
test project. If one doesn't exist, create a minimal one at
`tests/godot-project/test_shader.gdshader`:

```gdshader
shader_type spatial;
void fragment() {
    ALBEDO = vec3(1.0, 0.0, 0.0);
}
```

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Lint
cargo clippy -p director

# Deploy and run E2E tests
theatre-deploy ~/dev/stage/tests/godot-project
cargo test -p director-tests -- --include-ignored

# Verify all existing tests still pass
cargo test --workspace
```
