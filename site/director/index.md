---
description: "Director provides 43 operations for building Godot scenes, nodes, resources, tilemaps, animations, and project settings via MCP."
---

# Director

Director gives your AI agent the ability to build and modify Godot scenes, resources, tilemaps, and animations — through Godot's own API, not by hand-editing text files.

## The `.tscn` problem

Godot stores scenes in `.tscn` files — a custom text format that is readable but fragile. Editing `.tscn` by hand means:

- Getting node syntax exactly right
- Managing resource references and UIDs correctly
- No validation until Godot loads the file (silent corruption is possible)
- No support for `@export` defaults, which are only set through Godot's API

An AI agent that edits `.tscn` files directly will produce scenes that load with errors, missing references, or corrupted resource data. Director solves this by routing all scene modifications through Godot's own API — the same path the editor uses.

## Operations by domain

Director supports 43 operations across 9 domains:

| Domain | Operations |
|---|---|
| **Scenes** | Create, read, list, instance scene, diff two scenes |
| **Nodes** | Add, remove, set properties, get property, move, rename, find, set groups, set script, set meta |
| **Resources** | Read, create material/shape/style_box, duplicate |
| **TileMap / GridMap** | Set cells, get cells, clear; GridMap set/get cells, clear |
| **Animation** | Create animation, add track with keyframes |
| **Shaders** | Create visual shader, set shader code, get/set shader parameters |
| **Physics layers** | Set layer/mask names, set layer/mask bits |
| **Wiring** | Connect signals, disconnect signals, list signals, set export values |
| **Project** | Add/remove autoloads, set project settings, reload & validate, editor status |

All operations accept `project_path` as the first parameter — the absolute path to your Godot project directory.

## Three backends

Director routes operations to whichever backend is available:

### Editor plugin (port 6551) — preferred

When the Director addon is running in the open Godot editor, it listens on port 6551. Operations execute using the full editor API, including resource saving, scene import processing, and script reloading. Changes appear immediately in the editor.

**Best for**: Any time you have the editor open.

### Headless daemon (port 6550) — fallback

A Godot headless process (`godot --headless --script addons/director/daemon.gd`) can be running in the background. It listens on port 6550 and processes operations using Godot's resource system without a GUI.

**Best for**: CI/CD pipelines, batch operations, working without the editor open.

### One-shot (subprocess) — last resort

If neither TCP backend is reachable, Director spawns a temporary Godot process, runs the operation, and exits. Slower (one process startup per batch), but always available.

**Best for**: When neither the editor nor daemon is running and you only need a few operations.

### You do not pick the backend

The `director` binary tries port 6551, then 6550, then one-shot. You just call the MCP tool — Director handles routing automatically. If the editor is open, it uses the editor. If not, it falls back gracefully.

## `project_path` is always first

Every Director operation requires `project_path` — the absolute path to your Godot project. This tells Director which project to operate on when you have multiple projects open.

```json
{
  "op": "node_add",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "parent_path": "Level",
  "node_type": "StaticBody3D",
  "node_name": "Platform_5"
}
```

## Why not just edit `.tscn` files?

Director uses Godot's API because:

1. **Resource UIDs**: Godot 4 uses UIDs (`uid://...`) for resource references. Hand-editing creates broken references.
2. **Default values**: `@export` properties have defaults set by Godot, not hardcoded in `.tscn`. Only Godot's API correctly initializes them.
3. **Validation**: Godot validates every operation — invalid property types, missing node paths, and type mismatches are caught immediately with clear error messages.
4. **Signals and metadata**: Signal connections and node metadata have special serialization that is easy to corrupt by hand.

The rule is: if you would not hand-edit the `.tscn` directly, let Director do it through Godot.

## Combining with Stage

Director builds; Stage verifies. The flagship workflow:

1. Use Director to create or modify a scene
2. Press F5 to run the game
3. Use Stage to verify the result spatially — positions are correct, nodes are reachable, physics works

This loop — build → run → verify → adjust — is faster and more reliable than the traditional editor-only workflow because the agent can do the "inspect and verify" step without requiring manual observation.
