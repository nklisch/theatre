# Director API Reference

Complete parameter schemas for all Director operations. All operations require `project_path` as the first field.

## Scene Operations

### `scene_create`
```typescript
{
  op: "scene_create"
  project_path: string
  path: string                   // relative to project root, e.g. "scenes/player.tscn"
  root_class: string             // Godot class for root node
  root_name?: string             // default: root_class
}
```

### `scene_read`
```typescript
{
  op: "scene_read"
  project_path: string
  path: string
  max_depth?: number             // default: 10
}
```

### `scene_list`
```typescript
{
  op: "scene_list"
  project_path: string
  directory?: string             // default: "" (all scenes)
}
```

### `scene_add_instance`
```typescript
{
  op: "scene_add_instance"
  project_path: string
  scene: string                  // target scene to add instance to
  parent: string                 // node path within scene
  source_scene: string           // scene to instantiate
  name: string
  position?: [number, number, number]
}
```

### `scene_diff`
```typescript
{
  op: "scene_diff"
  project_path: string
  scene_a: string
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
  scene: string
  parent: string                 // "." for root
  name: string
  class: string
  position?: [number, number, number]
  properties?: { [key: string]: any }
}
```

### `node_remove`
```typescript
{
  op: "node_remove"
  project_path: string
  scene: string
  node: string                   // scene-relative path
}
```

### `node_set_properties`
```typescript
{
  op: "node_set_properties"
  project_path: string
  scene: string
  node: string
  properties: { [key: string]: any }
}
```

### `node_find`
```typescript
{
  op: "node_find"
  project_path: string
  scene: string
  class?: string                 // filter by Godot class
  group?: string                 // filter by group membership
  name_pattern?: string          // glob pattern, e.g. "Enemy_*"
  property?: string              // filter by property name
  property_value?: any           // filter by property value
}
```

### `node_set_groups`
```typescript
{
  op: "node_set_groups"
  project_path: string
  scene: string
  node: string
  add?: string[]                 // groups to add
  remove?: string[]              // groups to remove
}
```

### `node_set_script`
```typescript
{
  op: "node_set_script"
  project_path: string
  scene: string
  node: string
  script: string                 // path to .gd file (relative to project)
}
```

### `node_set_meta`
```typescript
{
  op: "node_set_meta"
  project_path: string
  scene: string
  node: string
  meta: { [key: string]: any }
}
```

### `node_reparent`
```typescript
{
  op: "node_reparent"
  project_path: string
  scene: string
  node: string
  new_parent: string
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
  path: string                   // .tres file path
  properties?: string[]          // specific properties to read (all if omitted)
}
```

**Response:**
```typescript
{
  op: "resource_read"
  path: string
  properties: { [key: string]: any }
}
```

### `material_create`
```typescript
{
  op: "material_create"
  project_path: string
  material_type: string          // e.g. "StandardMaterial3D", "ShaderMaterial"
  properties?: { [key: string]: any }
  save_path: string              // where to save the .tres
}
```

### `shape_create`
```typescript
{
  op: "shape_create"
  project_path: string
  shape_type: string             // e.g. "BoxShape3D", "CapsuleShape3D", "SphereShape3D"
  properties?: { [key: string]: any }
  save_path: string
}
```

### `style_box_create`
```typescript
{
  op: "style_box_create"
  project_path: string
  style_type: string             // e.g. "StyleBoxFlat", "StyleBoxTexture"
  properties?: { [key: string]: any }
  save_path: string
}
```

### `resource_duplicate`
```typescript
{
  op: "resource_duplicate"
  project_path: string
  source_path: string
  dest_path: string
}
```

---

## TileMap Operations

### `tilemap_set_cells`
```typescript
{
  op: "tilemap_set_cells"
  project_path: string
  scene: string
  node: string
  layer?: number                 // default: 0
  cells: Array<{
    position: [number, number]   // [col, row]
    source_id: number
    atlas_coords: [number, number]  // [-1,-1] to erase
  }>
}
```

### `tilemap_get_cells`
```typescript
{
  op: "tilemap_get_cells"
  project_path: string
  scene: string
  node: string
  region: { min: [number, number], max: [number, number] }
  layer?: number                 // default: 0
}
```

### `tilemap_clear`
```typescript
{
  op: "tilemap_clear"
  project_path: string
  scene: string
  node: string
  layer?: number                 // default: all layers
}
```

---

## GridMap Operations

### `gridmap_set_cells`
```typescript
{
  op: "gridmap_set_cells"
  project_path: string
  scene: string
  node: string
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
  scene: string
  node: string
  region: { min: [number, number, number], max: [number, number, number] }
}
```

### `gridmap_clear`
```typescript
{
  op: "gridmap_clear"
  project_path: string
  scene: string
  node: string
}
```

---

## Animation Operations

### `animation_create`
```typescript
{
  op: "animation_create"
  project_path: string
  scene: string
  node: string                   // AnimationPlayer path
  animation_name: string
  length: number                 // seconds
  loop_mode?: "none" | "loop" | "ping_pong"  // default: "none"
}
```

### `animation_add_track`
```typescript
{
  op: "animation_add_track"
  project_path: string
  scene: string
  node: string                   // AnimationPlayer path
  animation_name: string
  track_path: string             // "NodePath:property"
  track_type?: "property" | "method" | "audio" | "animation"  // default: "property"
  keyframes: Array<{
    time: number                 // seconds within animation
    value: any                   // keyframe value
    easing?: "linear" | "ease_in" | "ease_out" | "ease_in_out"
  }>
}
```

**Keyframes are included in `animation_add_track`. There is no separate set-key call.**

**Response:**
```typescript
{
  op: "animation_add_track"
  animation_name: string
  track_path: string
  track_index: number
  keyframes_set: number
  result: "ok"
}
```

---

## Shader Operations

### `visual_shader_create`
```typescript
{
  op: "visual_shader_create"
  project_path: string
  save_path: string              // where to save the .tres VisualShader
  shader_mode?: "spatial" | "canvas_item" | "particles"  // default: "spatial"
}
```


---

## Physics Layer Operations

### `physics_layer_names`
```typescript
{
  op: "physics_layer_names"
  project_path: string
  set?: { [layer_number: string]: string }  // omit to get current names
}
```

### `physics_layer_set`
```typescript
{
  op: "physics_layer_set"
  project_path: string
  scene: string
  node: string
  layers?: number[]              // layer numbers (1-indexed)
  collision_layer?: number       // bitmask (alternative to layers)
}
```

### `physics_mask_set`
```typescript
{
  op: "physics_mask_set"
  project_path: string
  scene: string
  node: string
  masks?: number[]               // layer numbers to detect
  collision_mask?: number        // bitmask (alternative to masks)
}
```

---

## Wiring Operations

### `signal_connect`
```typescript
{
  op: "signal_connect"
  project_path: string
  scene: string
  from_node: string
  signal: string
  to_node: string
  method: string
  flags?: number                 // default: 0
  binds?: any[]                  // additional arguments
}
```

### `signal_disconnect`
```typescript
{
  op: "signal_disconnect"
  project_path: string
  scene: string
  from_node: string
  signal: string
  to_node: string
  method: string
}
```

### `signal_list`
```typescript
{
  op: "signal_list"
  project_path: string
  scene: string
  node: string
}
```

---

## Batch Operations

### `batch`
```typescript
{
  op: "batch"
  project_path: string           // inherited by all operations
  stop_on_error?: boolean        // default: true
  operations: Array<{
    operation: string            // any director op name (no project_path needed)
    params: { [key: string]: any }  // parameters for the operation
  }>
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
  path: string                   // resource path to look up UID for
}
```

**Response:**
```typescript
{
  op: "uid_get"
  path: string
  uid: string                    // "uid://..." format
}
```

### `uid_update_project`
```typescript
{
  op: "uid_update_project"
  project_path: string
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
  scene: string                  // scene containing the MeshLibrary items
  save_path: string              // output .meshlib path
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
