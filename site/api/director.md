# Director API Reference

Complete parameter schemas for all Director operations. All operations require `project_path` as the first field.

## Scene Operations

### `scene_create`
```typescript
{
  op: "scene_create"
  project_path: string
  scene_path: string             // relative to project root, e.g. "scenes/player.tscn"
  root_type: string              // Godot class for root node
}
```

### `scene_read`
```typescript
{
  op: "scene_read"
  project_path: string
  scene_path: string
  depth?: number                 // optional; unlimited when omitted
  properties?: boolean           // include property values, default: true
}
```

### `scene_list`
```typescript
{
  op: "scene_list"
  project_path: string
  directory?: string             // default: "" (all scenes)
  pattern?: string               // glob pattern to filter scene paths
}
```

### `scene_add_instance`
```typescript
{
  op: "scene_add_instance"
  project_path: string
  scene_path: string             // target scene to add instance to
  instance_scene: string         // scene to instantiate
  parent_path?: string           // node path within scene, default: "."
  node_name?: string             // name for the new instance node
}
```

### `scene_diff`
```typescript
{
  op: "scene_diff"
  project_path: string
  scene_a: string                // supports git refs, e.g. "HEAD:scenes/level.tscn"
  scene_b: string
}
```

---

## Node Operations

### `node_add`
```typescript
{
  op: "node_add"
  project_path: string
  scene_path: string
  parent_path?: string           // "." for root (default: ".")
  node_type: string              // Godot class name
  node_name: string
  properties?: { [key: string]: any }
}
```

### `node_remove`
```typescript
{
  op: "node_remove"
  project_path: string
  scene_path: string
  node_path: string              // scene-relative path
}
```

### `node_set_properties`
```typescript
{
  op: "node_set_properties"
  project_path: string
  scene_path: string
  node_path: string
  properties: { [key: string]: any }
}
```

### `node_find`
```typescript
{
  op: "node_find"
  project_path: string
  scene_path: string
  class_name?: string            // filter by Godot class
  group?: string                 // filter by group membership
  name_pattern?: string          // glob pattern, e.g. "Enemy_*"
  property?: string              // filter by property name
  property_value?: any           // filter by property value
  limit?: number                 // max results, default: 100
}
```

### `node_set_groups`
```typescript
{
  op: "node_set_groups"
  project_path: string
  scene_path: string
  node_path: string
  add?: string[]                 // groups to add
  remove?: string[]              // groups to remove
}
```

### `node_set_script`
```typescript
{
  op: "node_set_script"
  project_path: string
  scene_path: string
  node_path: string
  script_path?: string           // path to .gd file (relative to project); omit to detach
}
```

### `node_set_meta`
```typescript
{
  op: "node_set_meta"
  project_path: string
  scene_path: string
  node_path: string
  meta: { [key: string]: any }
}
```

### `node_reparent`
```typescript
{
  op: "node_reparent"
  project_path: string
  scene_path: string
  node_path: string
  new_parent_path: string
  new_name?: string              // also rename the node in place
}
```

---

## Resource Operations

### `resource_read`
```typescript
{
  op: "resource_read"
  project_path: string
  resource_path: string          // .tres file path
  depth?: number                 // depth for sub-resources, default: 1
}
```

**Response:**
```typescript
{
  op: "resource_read"
  resource_path: string
  properties: { [key: string]: any }
}
```

### `material_create`
```typescript
{
  op: "material_create"
  project_path: string
  resource_path: string          // where to save the .tres
  material_type: string          // e.g. "StandardMaterial3D", "ShaderMaterial"
  properties?: { [key: string]: any }
  shader_path?: string           // required when material_type is "ShaderMaterial"
}
```

### `shape_create`
```typescript
{
  op: "shape_create"
  project_path: string
  shape_type: string             // e.g. "BoxShape3D", "CapsuleShape3D", "SphereShape3D"
  shape_params?: { [key: string]: any }
  save_path?: string             // if provided, saves the shape resource to disk
  scene_path?: string            // if provided, assigns shape to a node in this scene
  node_path?: string             // node path within scene_path to assign the shape to
}
```

### `style_box_create`
```typescript
{
  op: "style_box_create"
  project_path: string
  resource_path: string          // where to save the .tres
  style_type: string             // e.g. "StyleBoxFlat", "StyleBoxTexture"
  properties?: { [key: string]: any }
}
```

### `resource_duplicate`
```typescript
{
  op: "resource_duplicate"
  project_path: string
  source_path: string
  dest_path: string
  property_overrides?: { [key: string]: any }  // properties to change on the copy
  deep_copy?: boolean            // duplicate sub-resources, default: false
}
```

---

## TileMap Operations

### `tilemap_set_cells`
```typescript
{
  op: "tilemap_set_cells"
  project_path: string
  scene_path: string
  node_path: string
  cells: Array<{
    coords: [number, number]          // [col, row]
    source_id: number
    atlas_coords: [number, number]    // [-1,-1] to erase
    alternative_tile?: number         // default: 0
  }>
}
```

### `tilemap_get_cells`
```typescript
{
  op: "tilemap_get_cells"
  project_path: string
  scene_path: string
  node_path: string
  region?: {
    position: [number, number]
    size: [number, number]
  }
  source_id?: number             // filter by source
}
```

### `tilemap_clear`
```typescript
{
  op: "tilemap_clear"
  project_path: string
  scene_path: string
  node_path: string
  region?: {
    position: [number, number]
    size: [number, number]
  }
}
```

---

## GridMap Operations

### `gridmap_set_cells`
```typescript
{
  op: "gridmap_set_cells"
  project_path: string
  scene_path: string
  node_path: string
  cells: Array<{
    position: [number, number, number]
    item: number                 // MeshLibrary item index, -1 to erase
    orientation?: number         // 0-23, default: 0
  }>
}
```

### `gridmap_get_cells`
```typescript
{
  op: "gridmap_get_cells"
  project_path: string
  scene_path: string
  node_path: string
  bounds?: {
    min: [number, number, number]
    max: [number, number, number]
  }
  item?: number                  // filter by item index
}
```

### `gridmap_clear`
```typescript
{
  op: "gridmap_clear"
  project_path: string
  scene_path: string
  node_path: string
  bounds?: {
    min: [number, number, number]
    max: [number, number, number]
  }
}
```

---

## Animation Operations

Animations are stored as `.tres` resources (AnimationLibrary or Animation). All animation operations use `resource_path` to target the animation resource directly — not a scene node.

### `animation_create`
```typescript
{
  op: "animation_create"
  project_path: string
  resource_path: string          // path to save the .tres animation resource
  length: number                 // seconds
  loop_mode?: "none" | "linear" | "pingpong"  // default: "none"
  step?: number                  // keyframe step size in seconds
}
```

### `animation_add_track`
```typescript
{
  op: "animation_add_track"
  project_path: string
  resource_path: string          // animation resource to add track to
  track_type: "value" | "position_3d" | "rotation_3d" | "scale_3d" | "blend_shape" | "method" | "bezier"
  node_path: string              // "NodePath:property" for value tracks; NodePath for transform tracks
  keyframes: Array<{
    time: number                 // seconds within animation
    value?: any                  // keyframe value (value/blend_shape/bezier tracks)
    transition?: number          // easing value, default: 1.0
    method?: string              // method name (method tracks)
    args?: any[]                 // method arguments (method tracks)
    in_handle?: [number, number] // bezier in handle
    out_handle?: [number, number]// bezier out handle
  }>
  interpolation?: "nearest" | "linear" | "cubic" | "linear_angle" | "cubic_angle"
  update_mode?: "continuous" | "discrete" | "trigger" | "capture"
}
```

**Response:**
```typescript
{
  op: "animation_add_track"
  resource_path: string
  track_index: number
  keyframes_set: number
  result: "ok"
}
```

### `animation_read`
```typescript
{
  op: "animation_read"
  project_path: string
  resource_path: string          // animation resource path
}
```

### `animation_remove_track`
```typescript
{
  op: "animation_remove_track"
  project_path: string
  resource_path: string          // animation resource path
  track_index?: number           // remove by index
  node_path?: string             // remove by node path (removes all matching tracks)
}
```

---

## Shader Operations

### `visual_shader_create`
```typescript
{
  op: "visual_shader_create"
  project_path: string
  resource_path: string          // where to save the .tres VisualShader
  shader_mode: "spatial" | "canvas_item" | "particles"
  nodes: Array<{
    node_id: number              // unique integer ID for this shader node
    type: string                 // VisualShader node type class name
    shader_function: string      // function/operation specifier
    position?: [number, number]  // position in the visual shader graph
    properties?: { [key: string]: any }
  }>
  connections?: Array<{
    from_node: number            // source node_id
    from_port: number
    to_node: number              // destination node_id
    to_port: number
    shader_function: string
  }>
}
```

---

## Physics Layer Operations

### `physics_set_layers`
```typescript
{
  op: "physics_set_layers"
  project_path: string
  scene_path: string
  node_path: string
  collision_layer?: number       // bitmask
  collision_mask?: number        // bitmask
}
```

### `physics_set_layer_names`
```typescript
{
  op: "physics_set_layer_names"
  project_path: string
  layer_type: "2d_physics" | "3d_physics" | "2d_render" | "3d_render" | "2d_navigation" | "3d_navigation" | "avoidance"
  layers: { [layer_number: string]: string }  // e.g. { "1": "Player", "2": "Enemies" }
}
```

---

## Wiring Operations

### `signal_connect`
```typescript
{
  op: "signal_connect"
  project_path: string
  scene_path: string
  source_path: string            // path to node that emits the signal
  signal_name: string
  target_path: string            // path to node with the handler method
  method_name: string
  binds?: any[]                  // additional arguments passed to the method
  flags?: number                 // default: 0
}
```

### `signal_disconnect`
```typescript
{
  op: "signal_disconnect"
  project_path: string
  scene_path: string
  source_path: string
  signal_name: string
  target_path: string
  method_name: string
}
```

### `signal_list`
```typescript
{
  op: "signal_list"
  project_path: string
  scene_path: string
  node_path?: string             // if omitted, lists signals for all nodes in scene
}
```

---

## Batch Operations

### `batch`
```typescript
{
  op: "batch"
  project_path: string           // inherited by all operations
  operations: Array<{
    operation: string            // any director op name (no project_path needed)
    params: { [key: string]: any }  // parameters for the operation
  }>
  stop_on_error?: boolean        // default: true
}
```

**Response:**
```typescript
{
  op: "batch"
  total: number
  succeeded: number
  failed: number
  error_at?: number              // index of failed operation
  results: Array<{
    operation: string
    result: "ok" | "error"
    error?: string
    // ...other response fields
  }>
}
```

---

## UID Operations

### `uid_get`
```typescript
{
  op: "uid_get"
  project_path: string
  file_path: string              // resource path to look up UID for
}
```

**Response:**
```typescript
{
  op: "uid_get"
  file_path: string
  uid: string                    // "uid://..." format
}
```

### `uid_update_project`
```typescript
{
  op: "uid_update_project"
  project_path: string
  directory?: string             // limit rescan to this subdirectory
}
```

Rescans all resources and updates the project's UID cache. Run after adding or moving resources.

---

## Export Operations

### `export_mesh_library`
```typescript
{
  op: "export_mesh_library"
  project_path: string
  scene_path: string             // scene containing the MeshLibrary items
  output_path: string            // output .meshlib path
  items?: string[]               // specific item names to export (all if omitted)
}
```

Exports the meshes from a scene into a MeshLibrary resource, which can then be used by GridMap nodes.

---

## Common response fields

Every successful operation response includes:
```typescript
{
  op: string       // echoes the operation name
  result: "ok"     // always "ok" on success
}
```

Every error response:
```typescript
{
  op: string
  result: "error"
  error: string    // human-readable error message
}
```
