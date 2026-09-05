# Godot Project File Guidance

## Inspect project files freely

Read Godot project files and inspect their diffs whenever that helps you understand the
project or review a change. This includes text-serialized scenes and resources such as
`.tscn` and `.tres`, project settings such as `project.godot`, and related Godot files.
Use Director's `scene_read`, `resource_read`, and `scene_diff` when structured output is
more useful than raw text.

## Choose the edit path that matches the file

Edit GDScript (`.gd`) and shader source (`.gdshader`) with normal code-editing tools.
After source changes, use Director's `project_reload` when Godot-backed validation is
useful.

For structural scene, resource, and project-setting changes, prefer **Director** MCP
tools (or CLI) or the Godot editor. These paths use Godot's APIs and serialization for
engine types, resource references, UIDs, scene ownership, and signals:

- `scene_create`, `node_add`, `node_remove`, `node_set_properties`, `node_reparent`
- `material_create`, `shape_create`, `resource_duplicate`
- `tilemap_set_cells`, `gridmap_set_cells`
- `animation_create`, `animation_add_track`
- `signal_connect`, `signal_disconnect`
- `physics_set_layers`, `project_settings_set`, `autoload_add`, `autoload_remove`
- `batch` for sequential operations in one Godot invocation

Do not automatically fall back to arbitrary text mutations when a structural operation
is unavailable. Inspect the file, choose an appropriate Godot-backed workflow, or explain
the missing operation instead.

Director changes any open target scene through its live root and native undo history.
Individual and batch changes remain unsaved until `scene_save`. That operation serializes
only the selected scene, retains undo, and does not save unrelated external resources. The
editor's native dirty marker may remain. Read each operation's persistence data and verify
saved content after partial failures. Detached headless scene and resource operations persist
their target files.

Use `engine_api` for focused ClassDB discovery when a property, type, signal, method, enum,
or default is uncertain. Use ordinary project-owned GDScript for unusual procedural
authoring that typed operations express poorly. Theatre does not provide a general
arbitrary-code execution operation.

Use **Director** `editor_run` with a verified open editor to start, stop, or restart a
selected saved scene. A launch request does not establish Stage readiness. Check
`runtime_status` for the actual project, run, current scene, and readiness.

Use **Stage** MCP tools to observe and interact with the running game:

- `runtime_status`, `runtime_diagnostics`, `viewport` — identify the run, inspect bounded
  process diagnostics, and capture the latest completed render on demand
- `spatial_snapshot`, `spatial_delta`, `spatial_query` — inspect the game world
- `spatial_inspect` — examine one node in depth
- `spatial_action` — teleport, pause, set properties, call methods, or run a bounded paused
  interaction sequence
- `scene_tree` — navigate the node hierarchy

A viewport read is independent of recording and is not atomic with spatial state. Runtime
actions are temporary. Interaction sequences release their held named actions during
supported cleanup, but they do not make gameplay deterministic.

If a tool result reports pending human feedback, use `feedback` status and retrieve the
matching item. Retrieval does not handle or delete evidence. Handle it explicitly after
addressing it; delete it only through a separate deliberate operation.
