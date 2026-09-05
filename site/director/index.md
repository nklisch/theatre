---
description: "Build Godot scenes, resources, and project settings through native operations and focused engine API discovery."
---

# Director

Director gives your AI agent the ability to build and modify Godot scenes, resources, tilemaps, and animations — through Godot's own API, not by hand-editing text files.

## Native authoring and source inspection

Godot scene and resource files are readable. Read and diff them when that helps
explain project structure or inspect a change. For structural mutations, prefer
Director: Godot validates native types and manages resource references, UIDs,
node ownership, and serialization.

Direct text edits do not necessarily corrupt a scene, but they bypass the native
operation's validation. Do not use an automatic arbitrary-text fallback when an
operation fails. Narrow the failure and validate the result with Godot. Edit
GDScript and shader source with ordinary coding tools.

## Operations by domain

Director groups native operations by domain:

| Domain | Operations |
|---|---|
| **Scenes and nodes** | Read, create, compose, inspect, diff, save, and change hierarchy or properties |
| **Resources and media** | Read or create resources, tile/grid cells, animations, visual shaders, and collision shapes |
| **Project wiring** | Signals, groups, scripts, metadata, physics layers, autoloads, settings, UIDs, and reload validation |
| **Engine and editor** | Focused ClassDB discovery, verified editor identity/status, and selected-scene run control |
| **Human feedback** | Read and manage retained project-local evidence without launching Godot |

Director operations identify the selected Godot project with an absolute `project_path`.
The [generated Director reference](/api/director) owns the complete current tool
and parameter catalog.

## Discover the engine API

Use `engine_api` with a class name to start with a summary. Narrow to properties,
methods, signals, or enums when needed. Exact member selection and paginated
results avoid dumping the engine API into context. Returned metadata identifies
the actual engine version. Defaults may be structured, text-only, or unavailable;
do not assume every default is an authoring-ready JSON value.

## Three backends

Director routes operations to whichever backend is available:

### Editor plugin (port 6551) — preferred

When the Director addon is running in the open Godot editor, Director connects
on port 6551 and verifies the editor's actual project before dispatch. Open-scene
changes use live roots and native undo, and remain unsaved until `scene_save`.

**Best for**: Any time you have the editor open.

### Headless daemon (port 6550) — fallback

A Godot headless process (`godot --headless --script addons/director/daemon.gd`) can be running in the background. It listens on port 6550 and processes operations using Godot's resource system without a GUI.

**Best for**: CI/CD pipelines, batch operations, working without the editor open.

### One-shot (subprocess) — last resort

If neither TCP backend is reachable, Director can spawn a temporary headless
Godot process for supported operations. Editor-only run control does not use this
fallback.

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

## Save and run deliberately

Open-scene changes stay unsaved until `scene_save`. That operation serializes only
the selected scene, retains native undo, and does not flush unrelated external
resources. Godot's native dirty marker may remain. Batch operations are sequential,
not atomic, and preserve earlier or partial effects when a later entry fails.

`editor_run` starts, stops, or restarts a selected saved scene through a verified
open editor. Launch temporarily suppresses Godot's automatic pre-run save and
restores the previous setting immediately. Use Stage `runtime_status` afterward
to establish the actual run and readiness.

## Why not just edit `.tscn` files?

Director uses Godot's API because:

1. **Resource UIDs**: Godot 4 uses UIDs (`uid://...`) for resource references. Hand-editing creates broken references.
2. **Default values**: `@export` properties have defaults set by Godot, not hardcoded in `.tscn`. Only Godot's API correctly initializes them.
3. **Validation**: Godot validates every operation — invalid property types, missing node paths, and type mismatches are caught immediately with clear error messages.
4. **Signals and metadata**: Signal connections and node metadata have special serialization that is easy to corrupt by hand.

The rule is: if you would not hand-edit the `.tscn` directly, let Director do it through Godot.

For unusual procedural construction that typed operations express poorly, use an
ordinary project-owned GDScript and validate its result through Godot. A focused
experiment showed both a generated Director batch and a script can survive
semantic reload for representative construction. It did not justify broad speed
or compactness claims, and Theatre does not expose a general arbitrary-code
execution tool.

## Combining with Stage

Director builds; Stage verifies. The flagship workflow:

1. Use Director to create or modify a scene.
2. Save the selected scene explicitly when the editor path left it unsaved.
3. Use `editor_run` with an open editor, or launch the scene through Godot.
4. Use Stage `runtime_status`, structured observations, viewport evidence, and
   diagnostics to verify the result.

This loop lets the agent inspect and verify the running result without requiring
the developer to narrate every state change.
