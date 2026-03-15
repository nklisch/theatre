# Stage API Reference

Complete parameter schemas for all 9 Stage MCP tools.

## `spatial_snapshot`

Get an instant picture of every tracked node in the running game.

```typescript
{
  perspective?: "Camera" | "Node" | "Point"  // default: "Camera"
  focal_node?: string                         // node name or path; anchors perspective for "Node"
  focal_point?: [number, number, number]      // world-space point; used for "Point" perspective
  radius?: number                             // default: 50.0
  detail?: "summary" | "standard" | "full"   // default: "standard"
  groups?: string[]                           // filter to nodes belonging to these groups
  class_filter?: string[]                     // Godot class names to include
  include_offscreen?: boolean                 // default: false
  token_budget?: number                       // default: 1500 (standard tier)
  expand?: string                             // node path to expand with extra detail
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

Get only what changed since the last `spatial_snapshot` baseline.

```typescript
{
  perspective?: "Camera" | "Node" | "Point"  // default: "Camera"
  radius?: number                             // default: 50.0
  groups?: string[]                           // filter to nodes in these groups
  class_filter?: string[]
  token_budget?: number
}
```

Delta computes changes against the baseline established by the most recent `spatial_snapshot` call. There is no `since_frame` parameter — the baseline is stored automatically when you call `spatial_snapshot`.

**Response:**
```typescript
{
  frame: number
  baseline_frame: number
  elapsed_ms: number
  changed_node_count: number
  nodes: {
    [name: string]: {
      // Only fields that changed since the baseline snapshot
      global_position?: [number, number, number]
      velocity?: [number, number, number]
      // ...any other changed properties
    }
  }
}
```

---

## `spatial_query`

Run geometric queries against the current game state.

```typescript
{
  query_type: "nearest" | "radius" | "area" | "raycast" | "path_distance" | "relationship"
  from: string | [number, number, number]   // node name/path or [x, y, z] coordinate
  to?: string | [number, number, number]    // used by path_distance and relationship
  k?: number                               // max results for nearest, default: 5
  radius?: number                          // search radius for radius query, default: 20.0
  groups?: string[]
  class_filter?: string[]
  token_budget?: number
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
    bearing_from_a: number               // horizontal bearing from A to B (degrees)
    bearing_from_b: number               // horizontal bearing from B to A (degrees)
    relative: [number, number, number]   // offset from A to B in world space
    occluded: boolean
  }
}
```

---

## `spatial_inspect`

Deep inspection of a single node.

```typescript
{
  node: string                             // node name or scene path; required
  include?: Array<
    "transform" | "physics" | "state" | "children" |
    "signals" | "script" | "spatial_context" | "resources"
  >
  // default: ["transform", "physics", "state", "children", "signals", "script", "spatial_context"]
}
```

**Response:**
```typescript
{
  node: string
  path: string
  class: string
  frame: number
  transform?: {
    global_position: [number, number, number]
    rotation_deg: [number, number, number]
    scale: [number, number, number]
  }
  physics?: {
    velocity?: [number, number, number]
    collision_layer?: number
    collision_mask?: number
    on_floor?: boolean
    on_wall?: boolean
  }
  state?: {
    visible: boolean
    // ...node-class-specific state properties
  }
  children?: Array<{
    name: string
    class: string
    relative_position: [number, number, number]
  }>
  signals?: Array<{
    signal: string
    connected_to: string
    method: string
  }>
  script?: {
    path: string
    // ...exported script properties
  }
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
  resources?: {
    // ...resource references attached to this node
  }
}
```

---

## `spatial_watch`

Monitor nodes continuously for changes.

```typescript
// Add a watch
{
  action: "add"
  watch: {
    node: string
    conditions?: Array<{
      property: string
      operator: "Lt" | "Gt" | "Eq" | "Changed"
      value?: any
    }>              // default: []
    track?: Array<"Position" | "State" | "Signals" | "Physics" | "All">
                    // default: ["All"]
  }
}

// Remove a watch
{
  action: "remove"
  watch_id: string
}

// List active watches
{
  action: "list"
}

// Remove all watches
{
  action: "clear"
}
```

**Add response:**
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

**Remove response:**
```typescript
{
  watch_id: string
  result: "ok"
}
```

---

## `spatial_config`

Configure MCP behavior (clustering, bearing format, token limits, static patterns).

```typescript
{
  static_patterns?: string[]               // node name patterns treated as static (not tracked)
  state_properties?: { [class: string]: string[] }  // extra properties to capture per class
  cluster_by?: "Group" | "Class" | "Proximity" | "None"
  bearing_format?: "Cardinal" | "Degrees" | "Both"
  expose_internals?: boolean               // include internal/hidden nodes
  poll_interval?: number                   // polling interval in ms
  token_hard_cap?: number                  // hard cap on response tokens
}
```

**No parameters → returns current config:**
```typescript
{
  static_patterns: string[]
  state_properties: { [class: string]: string[] }
  cluster_by: string
  bearing_format: string
  expose_internals: boolean
  poll_interval: number
  token_hard_cap: number
}
```

**When parameters are set, the response echoes the new values with `"result": "ok"`.**

---

## `spatial_action`

Interact with the running game: control execution, modify state, inject input, spawn/remove nodes.

```typescript
// Pause or unpause the game
{ action: "pause", paused: boolean }

// Step N physics frames (game must be paused)
{ action: "advance_frames", frames: number }

// Advance by N seconds (game must be paused)
{ action: "advance_time", seconds: number }

// Move a node to a position
{
  action: "teleport"
  node: string
  position: [number, number, number]
  rotation_deg?: number
}

// Set a property on a node
{
  action: "set_property"
  node: string
  property: string
  value: any
}

// Call a method on a node
{
  action: "call_method"
  node: string
  method: string
  args?: any[]
}

// Emit a signal from a node
{
  action: "emit_signal"
  node: string
  signal: string
  args?: any[]
}

// Instantiate a scene as a child node
{
  action: "spawn_node"
  scene_path: string
  parent: string
  name?: string
}

// Remove a node from the scene tree
{
  action: "remove_node"
  node: string
}

// Simulate input action press
{
  action: "action_press"
  input_action: string
  strength?: number
}

// Simulate input action release
{
  action: "action_release"
  input_action: string
}

// Simulate keyboard input
{
  action: "inject_key"
  keycode: string
  pressed?: boolean
  echo?: boolean
}

// Simulate mouse button click
{
  action: "inject_mouse_button"
  button: string
  pressed?: boolean
  position?: [number, number]
}
```

All actions accept an optional `return_delta: boolean` (default: `false`) that appends a `spatial_delta` to the response.

**Response:**
```typescript
{
  action: string
  result: "ok" | "error"
  return_value?: any                       // for call_method with return value
  delta?: object                           // if return_delta: true
  error?: string                           // on failure
}
```

---

## `scene_tree`

Get scene tree structure without spatial data.

```typescript
{
  action: "roots" | "children" | "subtree" | "ancestors" | "find"
  node?: string                            // required for children/subtree/ancestors
  depth?: number                           // default: 3
  find_by?: "Name" | "Class" | "Group" | "Script"
  find_value?: string                      // search term for find action
  include?: Array<"Class" | "Groups" | "Script" | "Visible" | "ProcessMode">
                                           // default: ["Class", "Groups"]
  token_budget?: number
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
    groups?: string[]
    children: Array</* recursive */>
  }
}
```

---

## `clips`

Manage dashcam clips and analyze recorded gameplay frame by frame.

```typescript
// Mark the current moment and trigger clip capture
{
  action: "add_marker"
  marker_label?: string
  marker_frame?: number                    // defaults to current frame
}

// Force-save dashcam buffer as clip
{
  action: "save"
}

// Dashcam buffer state and config
{
  action: "status"
}

// List saved clips
{
  action: "list"
}

// Remove a clip
{
  action: "delete"
  clip_id: string
}

// List markers in a clip
{
  action: "markers"
  clip_id: string
}

// Spatial state at a frame
{
  action: "snapshot_at"
  clip_id?: string                         // defaults to most recent
  at_frame?: number
  at_time_ms?: number                      // alternative: find nearest frame
  detail?: "summary" | "standard" | "full"
}

// Position/property timeseries
{
  action: "trajectory"
  clip_id?: string
  node: string
  from_frame?: number
  to_frame?: number
  properties?: string[]                    // default: ["position"]
  sample_interval?: number                 // default: 1
}

// Search frames for spatial conditions
{
  action: "query_range"
  clip_id?: string
  from_frame?: number
  to_frame?: number
  node?: string
  condition?: {
    type: "moved" | "proximity" | "velocity_spike" | "property_change"
        | "state_transition" | "signal_emitted" | "entered_area" | "collision"
    // + type-specific fields (target, threshold, property, signal)
  }
  token_budget?: number
}

// Compare two frames
{
  action: "diff_frames"
  clip_id?: string
  frame_a: number
  frame_b: number
}

// Search events
{
  action: "find_event"
  clip_id?: string
  event_type?: string
  event_filter?: string
  from_frame?: number
  to_frame?: number
}

// Viewport screenshot at a frame
{
  action: "screenshot_at"
  clip_id?: string
  at_frame?: number
  at_time_ms?: number
}

// List screenshot metadata
{
  action: "screenshots"
  clip_id?: string
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
      marker_id: string
      frame: number
      marker_label: string
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
