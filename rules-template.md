# Godot Project Rules

## Never hand-edit Godot files

Do NOT directly read or edit `.tscn`, `.tres`, `.cfg`, `.import`, or
`project.godot` files. These are Godot's binary/text serialization
formats with UIDs, resource references, and ordering constraints that
break silently when edited by hand.

Instead, use **Director** MCP tools (or CLI) for all project file changes:
- `scene_create`, `scene_read`, `scene_list`, `scene_diff` — scene operations
- `node_add`, `node_remove`, `node_set_properties`, `node_reparent` — node manipulation
- `material_create`, `shape_create`, `resource_read` — resources
- `tilemap_set_cells`, `gridmap_set_cells` — tile/grid maps
- `animation_create`, `animation_add_track` — animations
- `signal_connect`, `signal_disconnect` — signal wiring
- `physics_set_layers` — collision layers/masks
- `batch` — multiple operations in one call

Use **Stage** MCP tools to observe and interact with the running game:
- `spatial_snapshot`, `spatial_delta`, `spatial_query` — see the game world
- `spatial_inspect` — deep-dive a single node
- `spatial_action` — teleport, pause, set properties, call methods
- `scene_tree` — navigate the node hierarchy

## GDScript is fine to edit directly

`.gd` files are regular source code — read and edit them normally.
Only `.tscn`, `.tres`, and Godot config files require Director.
