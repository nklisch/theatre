# Spectator API Reference

Complete parameter schemas for all 9 Spectator MCP tools.

## `spatial_snapshot`

Get an instant snapshot of all tracked nodes.

```typescript
{
  detail?: "summary" | "full" | "custom"  // default: "summary"
  budget_tokens?: number                   // default: 2000
  focus_node?: string                      // node name or path
  include_types?: string[]                 // Godot class names to include
  exclude_types?: string[]                 // Godot class names to exclude
  include_properties?: string[]            // for detail="custom"
}
```

**Response:**
```typescript
{
  frame: number
  timestamp_ms: number
  node_count: number
  included_nodes: number
  truncated: boolean
  nodes: {
    [name: string]: {
      class: string
      path: string
      global_position: [number, number, number]
      velocity?: [number, number, number]
      rotation_deg?: [number, number, number]
      scale?: [number, number, number]
      visible?: boolean
      collision_layer?: number
      collision_mask?: number
      on_floor?: boolean
      on_wall?: boolean
      // ...additional properties for detail="full"
    }
  }
}
```

---

## `spatial_delta`

Get only what changed since a specific frame.

```typescript
{
  since_frame: number                      // required
  budget_tokens?: number                   // default: 1000
  include_types?: string[]
  exclude_types?: string[]
  min_distance_change?: number             // default: 0.01 (meters)
  min_velocity_change?: number             // default: 0.1
}
```

**Response:**
```typescript
{
  from_frame: number
  to_frame: number
  elapsed_ms: number
  changed_node_count: number
  unchanged_node_count: number
  nodes: {
    [name: string]: {
      // Only fields that changed since from_frame
      global_position?: [number, number, number]
      velocity?: [number, number, number]
      // ...any other changed properties
    }
  }
}
```

**Errors:**
- `since_frame` older than ring buffer depth → `"Frame out of buffer range"`

---

## `spatial_query`

Run geometric queries against the scene.

```typescript
{
  type: "nearest" | "radius" | "area" | "raycast" | "path_distance" | "relationship"

  // For type="nearest":
  origin: string | [number, number, number]
  limit?: number                           // default: 10
  include_types?: string[]
  exclude_types?: string[]

  // For type="radius":
  origin: string | [number, number, number]
  radius: number
  limit?: number
  include_types?: string[]
  exclude_types?: string[]

  // For type="area":
  min: [number, number, number]
  max: [number, number, number]
  include_types?: string[]
  exclude_types?: string[]

  // For type="raycast":
  origin: string | [number, number, number]
  direction: [number, number, number]      // normalized
  max_distance?: number                    // default: 100.0
  collision_mask?: number                  // default: 0xFFFFFFFF

  // For type="path_distance":
  from: string | [number, number, number]
  to: string | [number, number, number]

  // For type="relationship":
  from: string
  to: string
}
```

**Response (nearest/radius):**
```typescript
{
  result: {
    origin: [number, number, number]
    radius?: number                        // radius queries only
    results: Array<{
      node: string
      class: string
      global_position: [number, number, number]
      distance: number
    }>
  }
}
```

**Response (area):**
```typescript
{
  result: {
    bounds: { min: [number, number, number], max: [number, number, number] }
    results: Array<{
      node: string
      class: string
      global_position: [number, number, number]
    }>
  }
}
```

**Response (raycast):**
```typescript
{
  result: {
    hit: boolean
    node?: string
    class?: string
    global_position?: [number, number, number]
    normal?: [number, number, number]
    distance?: number
  }
}
```

**Response (path_distance):**
```typescript
{
  result: {
    from: [number, number, number]
    to: [number, number, number]
    distance: number
    reachable: boolean
    waypoints: Array<[number, number, number]>
  }
}
```

**Response (relationship):**
```typescript
{
  result: {
    from: string
    to: string
    distance: number
    bearing_deg: number
    relative: [number, number, number]
    occluded: boolean
    in_fov: boolean
  }
}
```

---

## `spatial_inspect`

Deep inspection of a single node.

```typescript
{
  node: string                             // node name or scene path; required
  include?: Array<"properties" | "signals" | "children" | "spatial_context">
  // default: ["properties", "spatial_context"]
}
```

**Response:**
```typescript
{
  node: string
  path: string
  class: string
  frame: number
  properties?: {
    global_position: [number, number, number]
    // ...all tracked properties for this class
  }
  signals?: Array<{
    signal: string
    connected_to: string
    method: string
    flags: number
  }>
  children?: Array<{
    name: string
    class: string
    relative_position: [number, number, number]
  }>
  spatial_context?: {
    parent: {
      name: string
      class: string
      relative_position: [number, number, number]
    }
    nearby: Array<{
      node: string
      class: string
      distance: number
    }>
  }
}
```

---

## `spatial_watch`

Monitor nodes for continuous change tracking.

```typescript
// Create
{
  action: "create"
  node: string
  track?: string[]                         // default: ["position", "velocity"]
}

// List
{
  action: "list"
}

// Delete
{
  action: "delete"
  watch_id: string
}

// Clear all
{
  action: "clear"
}
```

**Create response:**
```typescript
{
  watch_id: string
  node: string
  track: string[]
  active_watches: number
}
```

**List response:**
```typescript
{
  watches: Array<{
    watch_id: string
    node: string
    track: string[]
    created_frame: number
  }>
}
```

**Delete response:**
```typescript
{
  watch_id: string
  result: "ok"
}
```

---

## `spatial_config`

Configure collection behavior.

```typescript
{
  tick_rate?: number                       // 1-120, default: 60
  capture_radius?: number                  // meters, default: 200.0
  capture_center?: string | null           // node to follow, default: null (origin)
  tracked_types?: string[] | null          // null = restore defaults
  extra_tracked_types?: string[]
  buffer_depth_frames?: number             // default: 600
  default_budget_tokens?: number           // default: 2000
  default_detail?: "summary" | "full"
  record_path?: string                     // directory for clip files
}
```

**No parameters → returns current config:**
```typescript
{
  tick_rate: number
  capture_radius: number
  capture_center: string | null
  buffer_depth_frames: number
  buffer_depth_seconds: number
  default_budget_tokens: number
  default_detail: string
  record_path: string
  tracked_types: string[]
  extra_tracked_types: string[]
}
```

---

## `spatial_action`

Set properties, call methods, or emit signals on running game nodes.

```typescript
// Set property
{
  node: string
  action: "set_property"
  property: string
  value: any
}

// Call method
{
  node: string
  action: "call_method"
  method: string
  args?: any[]
}

// Emit signal
{
  node: string
  action: "emit_signal"
  signal: string
  signal_args?: any[]
}
```

**Response:**
```typescript
{
  node: string
  action: string
  result: "ok" | "error"
  return_value?: any                       // for call_method with return value
  error?: string                           // on failure
}
```

---

## `scene_tree`

Get scene tree structure.

```typescript
{
  root?: string                            // default: "/" (full tree)
  max_depth?: number                       // default: 5
  include_types?: string[]
  exclude_types?: string[]
  show_properties?: string[]               // inline properties to include
}
```

**Response:**
```typescript
{
  root: string
  frame: number
  node_count: number
  tree: {
    name: string
    class: string
    // inline properties if show_properties set
    children: Array</* recursive */>
  }
}
```

---

## `recording`

Record and query spatial gameplay clips.

```typescript
// Start recording
{
  action: "start"
  clip_id?: string                         // auto-generated if omitted
}

// Stop recording
{
  action: "stop"
}

// Mark a frame
{
  action: "mark"
  label?: string
}

// List clips
{
  action: "list"
}

// Query single frame
{
  action: "query_frame"
  clip_id: string
  frame: number
  nodes?: string[]
  detail?: "summary" | "full"
}

// Query frame range
{
  action: "query_range"
  clip_id: string
  start_frame: number
  end_frame: number
  nodes?: string[]
  detail?: "summary" | "full"
  stride?: number                          // default: 1
  condition?: {
    type: "proximity" | "velocity_above" | "property_equals"
    // proximity:
    nodes?: [string, string]
    max_distance?: number
    // velocity_above:
    node?: string
    threshold?: number
    // property_equals:
    node?: string
    property?: string
    value?: any
  }
}

// Delete clip
{
  action: "delete"
  clip_id: string
}
```

**List response:**
```typescript
{
  clips: Array<{
    clip_id: string
    frame_count: number
    duration_ms: number
    created_at: string               // ISO 8601
    markers: Array<{
      frame: number
      label: string
    }>
  }>
}
```

**query_range response:**
```typescript
{
  clip_id: string
  start_frame: number
  end_frame: number
  frame_count: number
  frames: Array<{
    frame: number
    timestamp_ms: number
    nodes: {
      [name: string]: {
        class: string
        global_position?: [number, number, number]
        velocity?: [number, number, number]
        // ...other properties per detail level
      }
    }
  }>
}
```
