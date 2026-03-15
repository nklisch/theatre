---
name: theatre
description: >
  Use Director to create and modify Godot scenes, nodes, resources, tilemaps,
  animations, and signals via MCP tools or CLI. Invoke when the user asks to
  build scenes, add nodes, set properties, create materials, configure physics
  layers, edit animations, or perform any Godot project authoring task
  programmatically.
---

# Director — Godot Scene & Resource Authoring

Director is part of the **Theatre** toolkit (alongside Stage). It gives you 38 tools to create and modify Godot project files: scenes, nodes, resources, tilemaps, gridmaps, animations, physics, signals, and shaders.

**Two interfaces, identical capabilities:**

| Interface | When to use | Example |
|---|---|---|
| MCP tools | Agent has MCP connection to director | `scene_create(project_path: "...", ...)` |
| CLI | Agent uses bash, no MCP server | `director scene_create '{"project_path":"...", ...}'` |

**CLI basics:**
```bash
director <tool> '<json-params>'            # direct invocation
echo '<json>' | director scene_create      # stdin pipe
director --help                            # list all tools (categorized)
director --version                         # {"version": "0.1.0"}
```

All CLI output is JSON to stdout. Errors are JSON to stdout with exit codes: 0 success, 1 runtime error, 2 usage error.

**Every tool requires `project_path`** — the absolute path to the Godot project directory.

## Tool Reference

### Scene Tools
| Tool | Purpose |
|---|---|
| `scene_create` | Create a new .tscn with a root node type |
| `scene_read` | Read full node tree with types, properties, hierarchy |
| `scene_list` | List all .tscn files (with root type + node count) |
| `scene_diff` | Compare two scenes structurally (supports git refs) |
| `scene_add_instance` | Add a scene instance as a child node |

### Node Tools
| Tool | Purpose |
|---|---|
| `node_add` | Add a node to a scene with optional properties |
| `node_remove` | Remove a node and its children |
| `node_set_properties` | Set properties (auto-converts Vector2, Color, etc.) |
| `node_reparent` | Move a node to a new parent (optional rename) |
| `node_find` | Search by class, group, name pattern, or property |
| `node_set_groups` | Add/remove node from named groups |
| `node_set_script` | Attach/detach a GDScript file |
| `node_set_meta` | Set/remove metadata entries |

### Resource Tools
| Tool | Purpose |
|---|---|
| `resource_read` | Read .tres/.res file (type + properties) |
| `resource_duplicate` | Duplicate with optional overrides and deep copy |
| `material_create` | Create StandardMaterial3D, ShaderMaterial, etc. |
| `shape_create` | Create collision shapes (Box, Sphere, Capsule, etc.) |
| `style_box_create` | Create StyleBox resources for UI theming |

### TileMap Tools
| Tool | Purpose |
|---|---|
| `tilemap_set_cells` | Set cells by coords, source ID, atlas coords |
| `tilemap_get_cells` | Read cells (with optional region/source filter) |
| `tilemap_clear` | Clear cells (optional region) |

### GridMap Tools
| Tool | Purpose |
|---|---|
| `gridmap_set_cells` | Set 3D grid cells by position and item index |
| `gridmap_get_cells` | Read cells (with optional bounds/item filter) |
| `gridmap_clear` | Clear cells (optional bounds) |

### Animation Tools
| Tool | Purpose |
|---|---|
| `animation_create` | Create .tres animation (length, loop mode) |
| `animation_add_track` | Add track with keyframes (value, position, rotation, method, bezier) |
| `animation_read` | Read animation structure (tracks + keyframes) |
| `animation_remove_track` | Remove track by index or node path |

### Physics Tools
| Tool | Purpose |
|---|---|
| `physics_set_layers` | Set collision_layer/collision_mask bitmasks |
| `physics_set_layer_names` | Name physics/render/navigation layers in project.godot |

### Signal Tools
| Tool | Purpose |
|---|---|
| `signal_connect` | Connect a signal between two nodes |
| `signal_disconnect` | Remove a signal connection |
| `signal_list` | List all connections (optional node filter) |

### Other Tools
| Tool | Purpose |
|---|---|
| `visual_shader_create` | Create VisualShader with node graph |
| `export_mesh_library` | Export MeshInstance3D nodes as MeshLibrary |
| `uid_get` | Resolve a file's Godot UID |
| `uid_update_project` | Scan and register missing UIDs |
| `batch` | Execute multiple operations in one Godot invocation |

## Key Workflows

### Create a Scene from Scratch

```jsonc
// 1. Create the scene
{ "project_path": "/home/user/game", "scene_path": "res://levels/level_01.tscn", "root_type": "Node3D" }

// 2. Add nodes
{ "project_path": "/home/user/game", "scene_path": "res://levels/level_01.tscn",
  "parent_path": ".", "node_type": "DirectionalLight3D", "node_name": "Sun",
  "properties": { "rotation_degrees": "Vector3(-45, 30, 0)" } }

// 3. Instance a sub-scene
{ "project_path": "/home/user/game", "scene_path": "res://levels/level_01.tscn",
  "instance_scene": "res://characters/player.tscn", "parent_path": ".",
  "node_name": "Player" }
```

### Batch Operations (reduces cold-start overhead)

```jsonc
{
  "project_path": "/home/user/game",
  "operations": [
    { "operation": "node_add", "params": {
        "scene_path": "res://ui/hud.tscn", "parent_path": ".",
        "node_type": "Label", "node_name": "ScoreLabel" }},
    { "operation": "node_set_properties", "params": {
        "scene_path": "res://ui/hud.tscn", "node_path": "ScoreLabel",
        "properties": { "text": "Score: 0", "position": "Vector2(10, 10)" }}}
  ]
}
```

### Create a Material

```jsonc
{
  "project_path": "/home/user/game",
  "resource_path": "res://materials/metal.tres",
  "material_type": "StandardMaterial3D",
  "properties": {
    "metallic": 0.9,
    "roughness": 0.2,
    "albedo_color": "Color(0.8, 0.8, 0.85, 1.0)"
  }
}
```

### Set Up TileMap

```jsonc
// Set cells on a TileMapLayer
{
  "project_path": "/home/user/game",
  "scene_path": "res://levels/level_01.tscn",
  "node_path": "Ground",
  "cells": [
    { "coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0] },
    { "coords": [1, 0], "source_id": 0, "atlas_coords": [1, 0] },
    { "coords": [2, 0], "source_id": 0, "atlas_coords": [0, 0] }
  ]
}
```

### Animation Workflow

```jsonc
// 1. Create animation
{ "project_path": "/home/user/game", "resource_path": "res://anims/walk.tres",
  "length": 1.0, "loop_mode": "linear" }

// 2. Add position track
{ "project_path": "/home/user/game", "resource_path": "res://anims/walk.tres",
  "track_type": "position_3d", "node_path": "Skeleton3D:LeftFoot",
  "keyframes": [
    { "time": 0.0, "value": [0, 0, 0] },
    { "time": 0.5, "value": [0, 0.3, 0.5] },
    { "time": 1.0, "value": [0, 0, 1.0] }
  ] }
```

### Connect Signals

```jsonc
{ "project_path": "/home/user/game", "scene_path": "res://ui/button.tscn",
  "source_path": "StartButton", "signal_name": "pressed",
  "target_path": ".", "method_name": "_on_start_pressed" }
```

### Scene Diffing

```jsonc
// Compare current vs git commit
{
  "project_path": "/home/user/game",
  "scene_a": "HEAD:res://levels/level_01.tscn",
  "scene_b": "res://levels/level_01.tscn"
}
```

## Property Type Conversion

Director auto-converts string property values to Godot types:

| Write as | Godot type |
|---|---|
| `"Vector2(10, 20)"` | Vector2 |
| `"Vector3(1, 2, 3)"` | Vector3 |
| `"Color(1, 0, 0, 1)"` | Color |
| `"res://path/to/resource.tres"` | Resource path |
| `"NodePath(../Sibling)"` | NodePath |
| `0.5` (number) | float |
| `true` / `false` | bool |

## Error Reference

| Error | Meaning | Fix |
|---|---|---|
| `missing_project_path` | No `project_path` in params | Add absolute path to Godot project |
| `invalid_project` | project.godot not found | Check path exists and has project.godot |
| `godot_not_found` | Godot binary not in PATH | Install Godot and add to PATH |
| `operation_failed` | Godot rejected the operation | Check error message for details |
| `invalid_json` | Bad JSON params (CLI) | Fix JSON syntax |
| `missing_params` | No params provided (CLI) | Provide JSON arg or pipe via stdin |

## Full Parameter Reference

See [references/director-tools.md](references/director-tools.md) for complete parameter specifications for all 38 tools.
