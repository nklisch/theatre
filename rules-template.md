# Godot Project File Guidance

## Installation and launch intent

Machine installation, project wiring, and per-run observation/capture are separate
decisions. If Theatre is missing and would help, explain the needed scope and ask
the user before installing or enabling it in this project. Do not treat a failed
connection or `--yes` as installation consent. Once approved, noninteractive
project setup uses `theatre init . --yes --accept-project-install`; existing addons
and MCP configuration are preserved unless replacement is explicitly authorized.

Ordinary launches need neither Stage observation nor recording. Enable only the
evidence needed for an autonomous task, expressly requested by the user, or needed
for a specific human capture request. Do not change persistent preferences to
satisfy one run. Respect explicit local preferences and explicit per-run off.

Use Director `editor_run.launch` or `theatre run` to select observation independently
from startup/play capture. Human review normally uses `operator=human` and
`play_start=manual`: startup can finish and save, but play does not collect hours
of idle data while the person is away. This does not pause the game's simulation.
Minimal captures explicit metrics only; Light adds scoped spatial samples;
Standard adds images; Heavy allows whole-scene, higher-frequency capture.
Light and Standard require scene-relative node paths or explicit `group:name`
selectors. Images, sampling intervals, bounds, retention, and saving are separate
options, not hidden side effects of operator choice.

For asynchronous loading, locate the project's actual readiness transition and
explicitly add the following call there, only after all prerequisites succeed:

```gdscript
var stage := get_node_or_null("/root/StageRuntime")
if stage != null:
    stage.notify_ready()
```

Use `readiness=project` (the default). Do not use a scene's `_ready()` callback,
frame rate, or a quiet timeout as a substitute for application readiness. Do not
notify after canceled/failed loading. Duplicate notifications are safe. Only
simple synchronous scenes should explicitly select `readiness=scene`, whose
weaker boundary is scene initialization, not completion of background work.
Missing readiness stops startup at its capture bound and never starts play.

Human controls and `clips` support Start new, Continue, Stop, Keep, and Discard.
Continue starts a separate immutable segment with the same recording identity;
it never fills the uncaptured gap or retroactively increases evidence detail.
Unkept play drafts live in memory and are lost on exit. Keep before closing.
Metrics, saved clips, and their markers can be inspected after the game stops.

Shared defaults belong in `[launch]` in `stage.toml`. Private preferences can use
the OS application-config directory's `theatre/settings.toml`, or per-project
`stage.local.toml`. Before creating that local project file, request approval for
the narrow `stage.local.toml` ignore entry and verify the project ignores it.
Never commit private configuration or machine paths. Keep repository-owned paths
relative and use installed executable names through PATH. On Windows these names
resolve to `theatre.exe`, `director.exe`, and `stage.exe`; do not replace Unix
instructions with Windows-only paths or suffixes.

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
selected saved scene. Live Stage tools require explicit `launch.observe=true`.
A launch request does not establish Stage readiness. Check
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
