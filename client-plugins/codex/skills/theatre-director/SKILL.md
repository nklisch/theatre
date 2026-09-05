---
name: theatre-director
description: >
  Godot scene and resource authoring via Director MCP tools or CLI.
  ACTIVATE when: user asks to create scenes, add/remove/reparent nodes,
  set node properties, create materials or shapes, edit tilemaps or gridmaps,
  create or modify animations, connect/disconnect signals, set collision
  layers, attach scripts, diff scenes, manage autoloads, set project settings,
  check for script errors, see editor state, or perform any Godot project file
  editing task programmatically. Also activate for batch operations or
  "build me a level/scene/UI". Do NOT activate for observing a running
  game — use theatre-stage for that.
---

# Director — Godot Scene & Resource Authoring

Director is part of the **Theatre** toolkit (alongside Stage). It creates and modifies Godot project files, queries the installed engine API, controls saved-scene runs through an open editor, and reads project-local human feedback.

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

All CLI output is JSON to stdout. Exit codes 1 and 2 report runtime and usage
failures. A Director operation can also return `"success": false` with exit code
0, so inspect the JSON `success`, `error`, `context`, and `persistence` fields —
including ordered per-entry batch results — before deciding what succeeded or
should be retried.

**Every tool requires `project_path`** — the absolute path to the Godot project directory.

## Set Up and Select a Godot Project

Install Theatre once, then initialize each Godot project once so it has the
addons and plugin registration:

```bash
theatre init /absolute/path/to/godot-project
```

Respect the target repository's instructions and generators. If a generator owns
`project.godot`, plugin registration, scenes, or resources, change that owner and
regenerate rather than treating a generated output as authoritative.

Director does not use the Stage startup directory to select its target. Pass the
absolute Godot project directory as `project_path` on every MCP or CLI operation:

```json
{"project_path":"/absolute/path/to/godot-project"}
```

This allows consecutive Director calls to target different projects without
restarting Director. Stage is different: its persistent server selects a project
from `THEATRE_PROJECT_DIR` at startup and requires an MCP restart or new agent
session after that startup environment changes. For a one-off Stage CLI call,
use `THEATRE_PROJECT_DIR=/absolute/path/to/godot-project stage <tool> '<json>'`.
If projects share the default Theatre ports, stop the old running game before
starting another project so Director and Stage do not reach the old processes.

When an agent starts at a repository root whose MCP config already registers
Stage and Director for a nested sandbox, keep using that root config. Do not also
load the nested project's generated `.mcp.json` or duplicate its generated agent
rules. Initialize the sandbox once, then select Director with `project_path` and
Stage with the root server's startup environment.

An MCP entry's `env` applies only to that server process. If the optional native
client plugin should surface feedback for a nested project while the client runs
from the repository root, launch the client with the same absolute selection,
for example `THEATRE_PROJECT_DIR=/absolute/path/to/project claude ...` or
`THEATRE_PROJECT_DIR=/absolute/path/to/project codex`. The hook honors an
explicit selection and will not fall through to another ancestor project; when
unset it finds the nearest `project.godot` above the tool event's working
directory. Director calls still select their own project through `project_path`.

This skill may come from a native client plugin or a project's `.agents/skills`
directory. Both copies describe the same tools; use whichever the client
discovers, and keep project-installed copies as the fallback for clients that do
not load Theatre's native plugin. Do not interpret duplicate discovery as a need
to register another MCP server.

## Working with Godot project files

Read Godot project files and inspect their diffs whenever raw serialized detail is useful.
Director's structured reads and `scene_diff` complement normal file inspection; they do not
make `.tscn`, `.tres`, `project.godot`, or related files off-limits for reading.

Edit GDScript (`.gd`) and shader source (`.gdshader`) directly with normal code tools, then
use `project_reload` when Godot-backed validation is useful. For structural scene, resource,
and project-setting mutations, prefer Director or the Godot editor so engine types, resource
references, UIDs, ownership, and serialization are handled through Godot. If the available
operations cannot express a structural change, do not automatically fall back to arbitrary
text mutation; inspect the project and choose or report the missing Godot-backed path.

Director changes any open target scene through its live root and native undo, for
individual and batch calls. Edits remain unsaved until `scene_save`. Read persistence
results to distinguish saved files from changed unsaved scenes, including partial
failures.
`scene_save` saves only its selected scene and retains undo; unrelated edited external
resources are not saved, and the native editor dirty marker may remain. Headless scene
and resource operations persist their target files.

## Tool Reference

### Scene Tools
| Tool | Purpose |
|---|---|
| `scene_create` | Create a new .tscn with a root node type |
| `scene_read` | Read full node tree with types, properties, hierarchy |
| `scene_save` | Save only the selected scene, retaining native undo |
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

### Project Tools
| Tool | Purpose |
|---|---|
| `autoload_add` | Register an autoload singleton in project.godot |
| `autoload_remove` | Remove an autoload singleton |
| `project_settings_set` | Set project.godot settings (main scene, window size, etc.) |
| `project_reload` | Restart daemon + validate scripts — returns parse errors |
| `engine_api` | Query one installed-engine class and focused members/defaults |
| `editor_run` | Start, stop, restart, or inspect a saved-scene run through a verified editor |
| `editor_status` | Editor project/process identity, open scenes, play state, and recent editor log |
| `feedback` | Status, retrieve, handle, delete, or clean up project-local human feedback without Godot |
| `uid_get` | Resolve a file's Godot UID |
| `uid_update_project` | Scan and register missing UIDs |
| `export_mesh_library` | Export MeshInstance3D nodes as MeshLibrary |

### Other Tools
| Tool | Purpose |
|---|---|
| `visual_shader_create` | Create VisualShader with node graph |
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

### Create Scripts → Reload → Register Autoload → Build Scene

```jsonc
// 1. Write scripts with the Write tool (not Director)

// 2. Reload project to validate scripts and restart daemon
{ "project_path": "/home/user/game" }
// → returns { errors: [...], warnings: [...], scripts_checked: 14, autoloads: {...} }
// Fix any errors before proceeding!

// 3. Register autoload
{ "project_path": "/home/user/game", "name": "EventBus", "script_path": "autoload/event_bus.gd" }

// 4. Now safe to build scenes that reference the script
{ "project_path": "/home/user/game", "scene_path": "scenes/main.tscn", ... }
```

### Run and Verify a Saved Scene

```jsonc
// Start without implicitly saving open editor work
{ "project_path": "/home/user/game", "action": "start",
  "scene_path": "res://scenes/main.tscn" }
```

A successful `editor_run` start or restart reports that Godot accepted the native
play request. It does not prove that Stage attached or that the scene completed
`_ready`. Call Stage `runtime_status` separately and compare project, scene, and
`run_id`. Stop is idempotent. Run control requires a verified open editor and does
not fall back to a headless backend.

### Discover an Engine Type

```jsonc
{ "project_path": "/home/user/game", "class_name": "CharacterBody3D",
  "category": "properties", "member": "floor_snap_length" }
```

Start with the default summary, then request one category or exact member. Defaults
may be JSON values, Director text serialization, text-only descriptions, or
unavailable. Do not treat every returned default as an authoring-ready value.

### Read Human Feedback

```jsonc
{ "project_path": "/home/user/game", "action": "status" }
{ "project_path": "/home/user/game", "action": "retrieve",
  "feedback_id": "feedback_..." }
{ "project_path": "/home/user/game", "action": "handle",
  "feedback_id": "feedback_..." }
```

Status and retrieval work without an editor or game. Retrieval does not consume
the item. Handling suppresses pending notices for all readers but preserves the
evidence; deletion is separate.

### Check Editor State

```jsonc
// See what's happening in the editor right now
{ "project_path": "/home/user/game" }
// → { editor_connected: true, active_scene: "scenes/player.tscn",
//     open_scenes: [...], game_running: false, autoloads: {...},
//     recent_log: [...], errors: [...], warnings: [...] }
```

### Set Project Settings

```jsonc
{
  "project_path": "/home/user/game",
  "settings": {
    "application/run/main_scene": "res://scenes/main/main.tscn",
    "application/config/name": "My Game",
    "display/window/size/viewport_width": 1280,
    "display/window/size/viewport_height": 720
  }
}
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

See [references/director-tools.md](references/director-tools.md) for generated parameter specifications. Let generated schemas and current tool discovery own the complete catalog.
