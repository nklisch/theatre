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

### `scene_instance`
```typescript
{
  op: "scene_instance"
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

### `node_set_property`
```typescript
{
  op: "node_set_property"
  project_path: string
  scene: string
  node: string
  property?: string              // single property
  value?: any                    // single value
  properties?: { [key: string]: any }  // multiple at once
}
```

### `node_get_property`
```typescript
{
  op: "node_get_property"
  project_path: string
  scene: string
  node: string
  properties: string[]
}
```

**Response:**
```typescript
{
  op: "node_get_property"
  node: string
  properties: { [key: string]: any }
}
```

### `node_move`
```typescript
{
  op: "node_move"
  project_path: string
  scene: string
  node: string
  new_parent: string
}
```

### `node_rename`
```typescript
{
  op: "node_rename"
  project_path: string
  scene: string
  node: string
  new_name: string
}
```

---

## Resource Operations

### `resource_create`
```typescript
{
  op: "resource_create"
  project_path: string
  type: string                   // Godot resource class
  properties?: { [key: string]: any }
  save_path?: string             // if omitted, creates embedded resource
}
```

**Response:**
```typescript
{
  op: "resource_create"
  resource_id: string            // "@resource_id_xxx" for use in same session
  save_path?: string
  result: "ok"
}
```

### `resource_set`
```typescript
{
  op: "resource_set"
  project_path: string
  path: string                   // .tres file path
  properties: { [key: string]: any }
}
```

### `resource_get`
```typescript
{
  op: "resource_get"
  project_path: string
  path: string
  properties: string[]
}
```

### `resource_list`
```typescript
{
  op: "resource_list"
  project_path: string
  directory?: string
  type_filter?: string           // Godot class name filter
}
```

---

## TileMap Operations

### `tilemap_set`
```typescript
{
  op: "tilemap_set"
  project_path: string
  scene: string
  node: string
  layer?: number                 // default: 0
  tiles: Array<{
    position: [number, number]   // [col, row]
    source_id: number
    atlas_coords: [number, number]  // [-1,-1] to erase
  }>
}
```

### `tilemap_get`
```typescript
{
  op: "tilemap_get"
  project_path: string
  scene: string
  node: string
  region: { min: [number, number], max: [number, number] }
  layer?: number                 // default: 0
}
```

### `tilemap_fill`
```typescript
{
  op: "tilemap_fill"
  project_path: string
  scene: string
  node: string
  region: { min: [number, number], max: [number, number] }
  source_id: number
  atlas_coords: [number, number]
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

### `gridmap_set`
```typescript
{
  op: "gridmap_set"
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

### `gridmap_get`
```typescript
{
  op: "gridmap_get"
  project_path: string
  scene: string
  node: string
  region: { min: [number, number, number], max: [number, number, number] }
}
```

### `gridmap_fill`
```typescript
{
  op: "gridmap_fill"
  project_path: string
  scene: string
  node: string
  region: { min: [number, number, number], max: [number, number, number] }
  item: number
  orientation?: number           // default: 0
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
}
```

**Response includes `track_index` needed for `animation_set_key`.**

### `animation_set_key`
```typescript
{
  op: "animation_set_key"
  project_path: string
  scene: string
  node: string                   // AnimationPlayer path
  animation_name: string
  track_index: number            // from animation_add_track response
  time: number                   // seconds within animation
  value: any                     // keyframe value
  easing?: "linear" | "ease_in" | "ease_out" | "ease_in_out"
}
```

### `animation_play`
```typescript
{
  op: "animation_play"
  project_path: string
  node: string                   // AnimationPlayer path (running game)
  animation_name: string
  speed_scale?: number           // default: 1.0
}
```

---

## Shader Operations

### `shader_set`
```typescript
{
  op: "shader_set"
  project_path: string
  material_path: string          // ShaderMaterial .tres path
  shader_code: string            // GLSL code
  save_shader_path?: string      // save as .gdshader if set
}
```

### `shader_get_param`
```typescript
{
  op: "shader_get_param"
  project_path: string
  material_path: string
  param: string
}
```

### `shader_set_param`
```typescript
{
  op: "shader_set_param"
  project_path: string
  material_path: string
  param?: string                 // single param
  value?: any
  params?: { [key: string]: any }  // multiple params
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

### `export_set`
```typescript
{
  op: "export_set"
  project_path: string
  scene: string
  node: string
  property?: string
  value?: any
  properties?: { [key: string]: any }
}
```

---

## Batch Operations

### `batch_execute`
```typescript
{
  op: "batch_execute"
  project_path: string           // inherited by all operations
  stop_on_error?: boolean        // default: true
  operations: Array<{
    op: string                   // any director op (no project_path needed)
    // ...other params for the operation
  }>
}
```

**Response:**
```typescript
{
  op: "batch_execute"
  total: number
  succeeded: number
  failed: number
  error_at?: number              // index of failed operation
  results: Array<{
    op: string
    result: "ok" | "error"
    error?: string
    // ...other response fields
  }>
}
```

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
