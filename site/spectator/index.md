# Spectator

Spectator gives your AI agent spatial awareness of your running Godot game. It is a read-only observation layer — it never modifies game state, never affects physics, and has negligible performance impact.

## What Spectator does

Spectator continuously tracks every node in your scene tree that has spatial relevance — `CharacterBody3D`, `RigidBody3D`, `Area3D`, `Camera3D`, `AnimationPlayer`, `NavigationAgent3D`, and more. On every physics tick, it snapshots their positions, velocities, and key properties into an in-memory frame buffer.

Your AI agent can then:
- Ask for an instant picture of the whole scene
- Query what changed since a specific frame
- Find all nodes within a radius of a point
- Inspect a specific node's complete property set
- Set up watches on nodes of interest
- Record gameplay and scrub through the timeline

## The 9 tools

<div class="tool-cards">
<ToolCard
  title="spatial_snapshot"
  icon="📸"
  description="Instant picture of every tracked node's position and properties. The starting point for most investigations."
  link="/spectator/snapshot"
/>
<ToolCard
  title="spatial_delta"
  icon="⚡"
  description="Only what changed since a given frame. Much smaller than a full snapshot when most nodes are stationary."
  link="/spectator/delta"
/>
<ToolCard
  title="spatial_query"
  icon="🔍"
  description="Geometric queries: nearest nodes, radius search, area query, raycast, path distance, relationship between two nodes."
  link="/spectator/query"
/>
<ToolCard
  title="spatial_inspect"
  icon="🔬"
  description="Deep inspection of a single node: all properties, signals, children, and spatial context."
  link="/spectator/inspect"
/>
<ToolCard
  title="spatial_watch"
  icon="👁️"
  description="Monitor a node continuously. Returns a watch_id; poll with spatial_delta to see changes."
  link="/spectator/watch"
/>
<ToolCard
  title="spatial_config"
  icon="⚙️"
  description="Configure tick rate, capture radius, and which node types are tracked."
  link="/spectator/config"
/>
<ToolCard
  title="spatial_action"
  icon="🎮"
  description="Set a property, call a method, or emit a signal on a running game node. For testing, not production."
  link="/spectator/action"
/>
<ToolCard
  title="scene_tree"
  icon="🌳"
  description="Scene tree structure without spatial data. Fast and compact — good for understanding node layout."
  link="/spectator/scene-tree"
/>
<ToolCard
  title="recording"
  icon="🎬"
  description="Record gameplay clips, query frame ranges, mark bug moments. The foundation of the dashcam workflow."
  link="/spectator/recording"
/>
</div>

## When to use each tool

Choosing the right tool saves tokens and gives the agent better signal. Here is the decision guide:

### "What does my scene look like right now?"

→ **`spatial_snapshot`** with `detail: summary`

Start here. Get a broad picture of node positions and types. Then drill down with `spatial_inspect` or `spatial_query` if you need more.

### "What changed in the last few seconds?"

→ **`spatial_delta`** with `since_frame: N`

Much more efficient than repeated snapshots. Only includes nodes that moved or changed properties.

### "Is there anything near the player?"

→ **`spatial_query`** with `type: radius`

Geometric search returns nodes sorted by distance. Use for debugging detection zones, pickup ranges, spawn distances, etc.

### "Why does this specific node behave wrong?"

→ **`spatial_inspect`** with `include: ["properties", "spatial_context"]`

Gets everything: all tracked properties, parent/child relationships, nearby nodes, signal connections.

### "I need to watch a node over time"

→ **`spatial_watch`** then poll with **`spatial_delta`**

Set up the watch once, poll the delta periodically. The watch ensures the node is tracked even if it was not in the default capture set.

### "I want to understand the scene structure"

→ **`scene_tree`**

Returns hierarchy without spatial data. Compact and fast — good for understanding what exists before deciding what to inspect.

### "I recorded a bug moment"

→ **`recording`** with `action: query_range`

The main dashcam tool. Query the spatial timeline around the marked frame. Combine with `nodes` filter to stay focused.

### "I need to test my fix right now"

→ **`spatial_action`** with `action: set_property`

Mutates the running game for testing. Changes are not saved — they only affect the current session. Use Director for permanent changes.

## Performance impact

Spectator is designed to be invisible to the player:

- **Collection**: O(n) walk of tracked nodes during `_physics_process`. With 100 tracked nodes, this takes < 0.1ms per frame.
- **Memory**: The ring buffer holds 600 frames (10 seconds at 60Hz) by default. Each frame is roughly 1-2KB depending on node count.
- **Network**: Data is only sent when the MCP server requests it. No background transmission.
- **Recordings**: Clip files are written to disk asynchronously. The write does not block the main thread.

You can adjust the collection tick rate with `spatial_config` if you need to reduce overhead for a performance-sensitive scene.

## Limitations

Spectator reads the **engine's view** of node properties. Properties that are computed at render time (shader uniforms on the GPU, specific animation blend results) may not be available. Properties that are only in GDScript variables (not Godot node properties) are not visible — only exported properties and built-in node properties are tracked.

Spectator cannot read data from `@tool` scripts running in the editor — it only sees the running game.
