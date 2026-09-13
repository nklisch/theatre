---
name: theatre-stage
description: >
  Spatial debugging for running Godot games via Stage MCP tools or CLI.
  ACTIVATE when: user mentions game state, node positions, spatial bugs,
  physics issues, pathfinding problems, collision debugging, AI behavior,
  signal tracing, "take a snapshot", "what's happening in the game",
  clip/recording analysis, watch subscriptions, or any task that requires
  observing or interacting with a live Godot game world. Also activate
  for frame-by-frame debugging, teleporting nodes, pausing/advancing time,
  or injecting input. Do NOT activate for editing .tscn files or creating
  scenes — use theatre-director for that.
---

# Stage — Spatial Debugging for Godot

Stage is part of the **Theatre** toolkit (alongside Director). It observes and interacts with a running Godot game through spatial state, current viewport images, bounded runtime diagnostics, retained clips, and explicit debug actions.

**Two interfaces, different session lifetimes:**

| Interface | When to use | Example |
|---|---|---|
| MCP tools | Agent has MCP connection to stage | `spatial_snapshot(detail: "summary")` |
| CLI | Agent uses bash, no MCP server running | `stage spatial_snapshot '{"detail":"summary"}'` |

**CLI basics:**
```bash
stage <tool> '<json-params>'           # direct invocation
echo '{"detail":"summary"}' | stage spatial_snapshot  # stdin pipe
stage --help                           # list all tools
stage --version                        # {"version": "<installed version>"}
```

CLI tool results are JSON to stdout. Errors are JSON to stdout with exit code 1 (runtime) or 2 (usage). Logs go to stderr.

Each CLI call starts a fresh session. `spatial_delta`, all `spatial_watch`
operations, `spatial_config` updates, and actions with `return_delta: true`
return `persistent_session_required` (exit 2) before connecting or acting.
A snapshot in one CLI invocation cannot establish another invocation's baseline.
`spatial_config '{}'` still reads project defaults; put reusable defaults in
`stage.toml`. Ordinary snapshots, inspection, queries, actions without delta,
and addon-owned clip operations remain available.

For stateful workflows, configure your MCP client with command `stage` and
arguments `["serve"]`. Keep snapshot, watch/config, action, and delta calls in
that same MCP session. `stage serve` speaks MCP over stdio; it is not a shell
session command for passing subsequent CLI calls into. In shell-only workflows,
act without `return_delta`, then inspect or snapshot the result explicitly.

**Prerequisite for live tools:** Stage must be installed/enabled in the project and this run must explicitly enable `observe`. An ordinary launch intentionally has no listener or recording. A failed connection is not permission to install or activate anything. Use `runtime_status` to diagnose identity/connection and request only the evidence needed for the task. Project-local `feedback` and retained clips remain available without a running game.

## Explicit launches and human review

Ask the user before installing or enabling missing Theatre components in a project.
Machine installation, project wiring, and activation for a run are separate choices.
`--yes` selects setup defaults, not consent: after user approval, project setup uses
`theatre init . --yes --accept-project-install`. Preserve existing configuration;
request deliberate approval before `--overwrite-existing`.

On Windows, explicitly select among these workflows. Use
`--presentation automated` for agent visual review. For background tests, prefer a
meaningful headless path when it retains the evidence under test; otherwise use
automated. Use `--presentation deferred` to prepare requested human testing, then
confirm the project's actual readiness and tell the user in the normal response
that the scene is ready, including the checks to perform. Use
`--presentation interactive` only for an expressly requested human-visible review.
Omission is legacy behavior, not an automatic safety choice. If an automated or
deferred result is degraded, preserve its launch/running facts, stop the run before
fallback, and do not announce a degraded deferred run as ready without a new user
decision. Linux and macOS retain their established headless, graphical, and wrapper
workflows; automated and deferred are unsupported there.

Default to ordinary launches without observation or recording unless an autonomous
task needs evidence, the user requests it, or you request a specific human capture.
Honor local defaults and explicit per-run off. Do not rewrite preferences for one run.
Use `theatre run scene.tscn` through the existing editor, or Director `editor_run`
with the same typed `launch` object. For example:

```json
{"action":"start","scene_path":"scenes/review.tscn","launch":{"startup":{"preset":"minimal"},"play":{"preset":"standard","scope":["group:review"]},"operator":"human","readiness":"project"}}
```

Supply Director's project selection separately. Add `observe:true` only when live
agent tools are needed. Human capture works without a listener. Human play defaults
to manual Start; requested startup capture automatically stops and saves at readiness.
Hours away from the game must not become play evidence. This does not pause gameplay.
Agent play defaults to start at readiness; local/per-run `play_start` can override it.

Minimal collects metrics only; Light adds scoped spatial state; Standard adds images;
Heavy permits whole-scene, higher-frequency state/images. Light/Standard require
explicit node paths or `group:name` scope. Images, bounds, retention and saving are
independent options. Never promise a preset's performance from its name.

For asynchronous loading, identify the project's successful readiness transition
and explicitly integrate this call there before using phased capture:

```gdscript
var stage := get_node_or_null("/root/StageRuntime")
if stage != null:
    stage.notify_ready()
```

Keep `readiness=project`, the default. Do not notify for failed/canceled loading,
or guess completion from `_ready()`, elapsed time, frame rate, or one finished job.
The safe absent/off call and duplicate notifications have no capture side effects.
Only synchronous scenes should explicitly select `readiness=scene`; it means scene
initialization, not background-work completion. Missing readiness hits the startup
bound without silently starting play. Startup cannot profile internals of synchronous
work that blocks the main thread; it retains available samples and elapsed time.

Custom metrics use `register_metric_provider(name, Callable)` returning a Dictionary
and `unregister_metric_provider(name)`. Callbacks run only in active metrics capture.
Use `register_metadata_provider(name, Callable)` for small per-segment context,
and unregister it on teardown. It snapshots at explicit Start/phase boundaries;
register before the desired boundary. Do not include secrets or machine paths.
Keep callbacks cheap, bounded, and free of private information. Project-specific
providers, readiness conditions and input choices belong to that project, not Theatre.

Shared defaults use `[launch]` in `stage.toml`. Private overrides use the OS config
directory's `theatre/settings.toml`, then `stage.local.toml`; per-run options win.
Before creating `stage.local.toml`, obtain approval for its narrow `.gitignore` rule
and verify it is ignored. Never commit private preferences or machine paths. Use
repository-relative paths for repository content and command names through PATH.
Windows resolves the executable suffix; retain separate Unix instructions.

## Set Up and Select a Godot Project

After user approval, install Theatre once, then initialize each Godot project so it has the
addons, plugin registration, and Stage autoload:

```bash
theatre init .
```

Respect the target repository's instructions and generators. If a generator owns
`project.godot`, plugin registration, or another initialized file, change that
owner and regenerate rather than treating the generated copy as authoritative.

Stage initially selects its project from `THEATRE_PROJECT_DIR` when the server
starts. In persistent MCP, call `project_select` with an absolute `project_path`
to switch without restarting. Its optional `port` overrides the new project's
`stage.toml` port; absent both it uses 9077, not the old target's startup port.

**Every selection discards watches, snapshot/delta baselines, spatial indexes,
session overrides and cached clip location, even for the same project.** Nothing
is restored when switching back. Take a fresh `spatial_snapshot` before deltas or
indexed queries and recreate watches/overrides as needed. Read the switch result:
selected path/port is not proof of a connected, ready game. An unavailable target
stays selected and reconnecting, never falling back. Selection does not start or
stop games, delete recordings, switch Director, or update client-hook environments.

Startup configuration remains useful for the initial target:
```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "stage",
      "args": ["serve"],
      "env": {"THEATRE_PROJECT_DIR": "/absolute/path/to/godot-project"}
    }
  }
}
```

When an agent starts at a repository root whose MCP config already registers
Stage and Director for a nested sandbox, keep using that root config. Do not also
load the nested project's generated `.mcp.json` or duplicate its generated agent
rules. Use `project_select` when switching sandboxes. Editing the startup
environment alone does not retarget an existing process.

An MCP entry's `env` applies to the Stage server process only. If the optional
native client plugin should surface feedback for a nested project while the
client runs from the repository root, also launch the client with the same
absolute selection when starting Claude or Codex, for example:

```bash
THEATRE_PROJECT_DIR=/absolute/path/to/project claude ...
THEATRE_PROJECT_DIR=/absolute/path/to/project codex
```

The hook
honors that explicit selection and does not fall through to a different ancestor
project. Without it, the hook uses the nearest `project.godot` above the tool
event's working directory. A Stage `project_select` changes Stage's feedback
queue, not that independent hook selection.

For one Stage shell call, override selection without changing persistent configuration:

```bash
THEATRE_PROJECT_DIR=/absolute/path/to/godot-project stage runtime_status '{}'
```

The selected game's Stage listener must use the same port as the client. If two
projects share the default ports, stop the old running game before starting the
new one; otherwise Stage may reject the connection as a project mismatch. After
switching, call `runtime_status` and verify the reported project, scene, and run.
Director project selection is independent: pass its absolute `project_path` on
every Director call.

This skill may come from a native client plugin or a project's `.agents/skills`
directory. Both copies describe the same tools; use whichever the client
discovers, and keep project-installed copies as the fallback for clients that do
not load Theatre's native plugin. Do not interpret duplicate discovery as a need
to register another MCP server.

## When to Use Which Tool

```
"Which project and run is connected?"     → runtime_status
"What errors occurred in this run?"        → runtime_diagnostics
"Show the latest completed render"         → viewport
"What's in the scene right now?"          → spatial_snapshot
"What changed since last time?"           → spatial_delta
"What's near X? Can A see B?"             → spatial_query
"Tell me everything about this node"      → spatial_inspect
"Alert me when health drops below 20"     → spatial_watch
"Teleport/pause/run bounded input"         → spatial_action
"How is this scene structured?"           → scene_tree
"Configure what to track"                 → spatial_config
"Mark this moment / save a clip"          → clips
"What did the developer share?"           → feedback
```

## Standard Opening Move

When live observation is needed and enabled, start cheap and drill down:

```
1. runtime_status()                              → verify project, run, scene, readiness
2. spatial_snapshot(detail: "summary")         → cheap scene overview
3. spatial_snapshot(expand: "enemies")          → focused entity details
4. spatial_inspect(node: "enemies/scout_02")    → deep dive
```

If a result includes `feedback_notice`, check `feedback(action: "status")` and
retrieve the matching item before continuing past the human's observation.
Retrieval is non-destructive; handle the item explicitly after addressing it.

Never start with `detail: "full"` on the full scene — that's expensive and usually unnecessary.

## spatial_snapshot — Scene Overview

```jsonc
// Minimum: what's in the scene?
{ "detail": "summary" }

// Standard view with filters
{
  "detail": "standard",
  "groups": ["enemies"],
  "radius": 30.0,
  "perspective": "camera"
}

// Drill into a summary cluster
{ "expand": "enemies", "detail": "standard" }

// From a specific node's perspective
{
  "perspective": "node",
  "focal_node": "player",
  "detail": "standard",
  "radius": 20.0
}
```

**`detail` tiers:**
- `summary` (~200t): clusters with counts, nearest/farthest, brief state summary. Use first.
- `standard` (~400-800t): per-entity positions, bearings, state, recent signals. Use for most debugging.
- `full` (~1000t+): adds full transforms, physics, children, scripts, static listings. Use only when needed.

**Filtering reduces tokens and noise:**
- `groups: ["enemies"]` — only nodes in the "enemies" group
- `class_filter: ["CharacterBody3D"]` — only that class
- `radius: 20.0` — only within 20 units

## spatial_delta — What Changed?

Use in persistent MCP after taking an action or advancing time. Compares against the baseline established by `spatial_snapshot` in that same session, then updated by deltas (including action-returned deltas). One-shot CLI delta calls are rejected.

```jsonc
// See what changed (all defaults)
{}

// Filtered delta
{ "groups": ["enemies"], "radius": 30.0 }
```

Parameters: `perspective` (camera/point), `radius` (default 50.0), `groups`, `class_filter`, `token_budget`.

Response includes: `from_frame`, `to_frame`, and any non-empty of: `moved`, `state_changed`, `entered`, `exited`, `signals_emitted`, `watch_triggers`.

**The act-then-delta pattern** — use `return_delta: true` on actions instead of a separate delta call:
```jsonc
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "position": [5.0, 0.0, -3.0],
  "return_delta": true
}
```

## spatial_query — Targeted Spatial Questions

```jsonc
// What's near the player?
{ "query_type": "nearest", "from": "player", "k": 5, "groups": ["enemies"] }

// Can the enemy see the player?
{ "query_type": "raycast", "from": "enemies/scout_02", "to": "player" }

// Full relationship between two nodes
{ "query_type": "relationship", "from": "enemies/scout_02", "to": "player" }

// Navmesh path distance
{ "query_type": "path_distance", "from": "enemies/guard_01", "to": "player" }

// All enemies within 15 units of player
{ "query_type": "radius", "from": "player", "radius": 15.0, "groups": ["enemies"] }
```

`from` and `to` accept either a **node path** (`"player"`) or a **world position** (`[10.0, 0.0, 5.0]`).

## spatial_inspect — Deep Single Node

```jsonc
// Everything about a node
{ "node": "enemies/scout_02" }

// Specific categories only (cheaper)
{ "node": "enemies/scout_02", "include": ["physics", "state"] }

// Available categories:
// transform, physics, state, children, signals, script, spatial_context, resources
```

**Useful include combos:**
- `["physics"]` — velocity, on_floor, collision_layer/mask
- `["state"]` — all exported vars
- `["children"]` — immediate children with key properties
- `["signals"]` — connected signals + recent emissions
- `["spatial_context"]` — nearby entities, areas, camera visibility

## spatial_watch — Subscribe to Changes

```jsonc
// Watch a node for all changes
{ "action": "add", "watch": { "node": "enemies/scout_02", "track": ["all"] } }

// Conditional watch — fires when health < 20
{
  "action": "add",
  "watch": {
    "node": "enemies/scout_02",
    "conditions": [{ "property": "health", "operator": "lt", "value": 20 }],
    "track": ["position", "state"]
  }
}

// Watch entire group
{ "action": "add", "watch": { "node": "group:enemies", "track": ["position", "state"] } }

// List active watches
{ "action": "list" }

// Remove all
{ "action": "clear" }
```

Watch triggers arrive in `spatial_delta` responses under `watch_triggers`.

**Note:** Watches require a persistent MCP session. One-shot CLI watch operations are rejected; they cannot access another session's subscriptions.

## spatial_action — Debugging Manipulation

```jsonc
// Pause the game
{ "action": "pause", "paused": true }

// Advance 30 frames while paused
{ "action": "advance_frames", "frames": 30 }

// Teleport a node
{
  "action": "teleport",
  "node": "enemies/scout_02",
  "position": [5.0, 0.0, -3.0],
  "rotation_deg": 180,
  "return_delta": true
}

// Change a property
{ "action": "set_property", "node": "enemies/scout_02", "property": "collision_mask", "value": 7 }

// Call a method
{ "action": "call_method", "node": "enemies/scout_02", "method": "take_damage", "args": [50] }

// Emit a signal
{ "action": "emit_signal", "node": "enemies/scout_02", "signal": "health_changed", "args": [10] }

// Spawn a scene
{
  "action": "spawn_node",
  "scene_path": "res://enemies/scout.tscn",
  "parent": "enemies",
  "name": "test_scout",
  "position": [10.0, 0.0, 0.0]
}

// Advance half a second while paused
{ "action": "advance_time", "seconds": 0.5 }

// Remove a node
{ "action": "remove_node", "node": "enemies/scout_02" }

// Simulate input action
{ "action": "action_press", "input_action": "jump" }
{ "action": "action_release", "input_action": "jump" }

// Inject key event
{ "action": "inject_key", "keycode": "space", "pressed": true }

// Inject mouse button event
{ "action": "inject_mouse_button", "button": "left", "pressed": true, "position": [400, 300] }

// Run a bounded InputMap sequence while already paused
{
  "action": "interaction_sequence",
  "steps": [
    { "press": [{ "action_name": "move_right" }], "frames": 20 },
    { "press": [{ "action_name": "jump" }], "frames": 1 },
    { "release": ["jump", "move_right"], "frames": 10 }
  ]
}
```

An interaction sequence accepts a bounded step and frame count, keeps the game
paused, and releases sequence-held actions on supported completion and cleanup
paths. It does not make gameplay deterministic. If the engine is stopped or hung
inside a native debugger, its cleanup callback cannot run.

## Current Runtime Evidence

Use `runtime_status` before a run-sensitive workflow. It reports the actual
project, process, `run_id`, current scene, and readiness. A Director launch
request and a TCP connection do not by themselves establish readiness.

Use `runtime_diagnostics` for bounded errors, warnings, script errors, and shader
errors captured after the Stage autoload registered its Logger. Reads do not
consume diagnostics. The queue survives client reconnects but not a game restart.
It does not recover early engine initialization output, suppressed log streams,
or unavailable release backtraces.

Use `viewport` for a bounded JPEG of the latest completed root-viewport render.
It does not start recording, save a clip, or use the recorder. Readback counters
show provenance but do not make pixels atomic with a separate spatial query.
Headless or empty-pixel responses leave spatial observation available.

## feedback — Human Context

```jsonc
{ "action": "status" }
{ "action": "retrieve", "feedback_id": "feedback_..." }
{ "action": "handle", "feedback_id": "feedback_..." }
```

Feedback is retained under the selected project's `.theatre/feedback` directory
and remains readable after the game exits. It can contain runtime or editor
selection/pointer context, an optional JPEG, and a note. Retrieval does not handle
or delete evidence. Handling suppresses pending notices for every reader but
keeps retrieval available; deletion is a separate explicit action.

## scene_tree — Navigate Hierarchy

```jsonc
// Top-level structure
{ "action": "roots" }

// Immediate children
{ "action": "children", "node": "enemies" }

// Recursive tree (depth 3 default)
{ "action": "subtree", "node": "enemies", "depth": 4 }

// Find nodes by class
{ "action": "find", "find_by": "class", "find_value": "CharacterBody3D" }

// Find nodes by script
{ "action": "find", "find_by": "script", "find_value": "res://enemies/scout_ai.gd" }

// Parent chain
{ "action": "ancestors", "node": "enemies/scout_02/NavAgent" }
```

## spatial_config — Session Setup

Call at the start of a persistent MCP session to tune what Stage tracks.
One-shot CLI accepts an empty configuration read, not updates; use `stage.toml`
for defaults shared by future invocations:

```jsonc
{
  "static_patterns": ["walls/*", "terrain/*", "props/*"],
  "state_properties": {
    "enemies": ["health", "alert_level", "current_target"],
    "CharacterBody3D": ["velocity"],
    "*": ["visible"]
  },
  "cluster_by": "group",
  "bearing_format": "cardinal",
  "token_hard_cap": 3000,
  "poll_interval": 1,
  "expose_internals": false
}
```

`state_properties` controls which exported vars appear in snapshot `state` blocks.

## clips — Mark, Save, Analyze

Capture is explicitly launched and divided into segments. Stop a manual segment,
then Keep or Discard its draft. Continue starts a separate segment under the same
recording ID, preserving the uncaptured gap. Start creates a new recording ID.
Unkept drafts are memory-only and are lost on exit; Keep before closing.
Rolling retention and `save=on_trigger` explicitly enable marker-triggered saving.

```jsonc
// Check phase, readiness, effective settings, bounds and retained draft
{ "action": "status" }

// Start deliberately after readiness (use continue for the same recording)
{ "action": "start" }

// Annotate a moment; manual saving does not secretly become automatic
{ "action": "add_marker", "marker_label": "wall_clip_repro" }

// Finalize and retain the manual draft
{ "action": "stop" }
{ "action": "keep", "marker_label": "reproduction note" }

// Metrics remain inspectable after the game stops
{ "action": "metrics", "clip_id": "clip_001a2b3c" }

// List saved clips
{ "action": "list" }

// See markers in a clip
{ "action": "markers", "clip_id": "clip_001a2b3c" }
// Note: marker entries have a "source" field: "human" (F9), "agent" (MCP add_marker),
// "system" (opt-in anomaly marker), or "code" (StageRuntime.marker() in game script).
// Code markers may be "system" tier (rate-limited), "deliberate" (triggers only with explicit on_trigger saving),
// or "silent" (annotation only — attached to clips triggered by other means).

// Spatial state at a frame (omit clip_id for most recent)
{ "action": "snapshot_at", "at_frame": 4582, "detail": "standard" }

// Find when enemy got within 0.5m of wall
{
  "action": "query_range",
  "from_frame": 4570, "to_frame": 4600,
  "node": "enemies/guard_01",
  "condition": { "type": "proximity", "target": "walls/*", "threshold": 0.5 }
}

// Compare before/after
{ "action": "diff_frames", "frame_a": 4575, "frame_b": 4585 }

// Search for events
{
  "action": "find_event",
  "event_type": "signal",
  "event_filter": "health_changed",
  "node": "enemies/guard_01",
  "from_frame": 4500, "to_frame": 5000
}

// Delete a clip
{ "action": "delete", "clip_id": "clip_001a2b3c" }

// Node trajectory over time
{ "action": "trajectory", "node": "enemies/guard_01", "from_frame": 4500, "to_frame": 5000 }

// Screenshot at a frame
{ "action": "screenshot_at", "at_frame": 4582 }

// List available screenshots
{ "action": "screenshots", "clip_id": "clip_001a2b3c" }

// Build a temporal visual artifact from saved clip screenshots
// artifact: "storyboard", "motion_history", "difference_map", or "node_filmstrip"
{ "action": "visual_artifact", "artifact": "storyboard", "at_frame": 4582 }

// Stage next-segment settings; never starts capture or rewrites an active segment
{ "action": "config", "config": { "preset": "minimal" } }
```

## Common Debugging Workflows

### Collision / Wall Clipping
```
1. spatial_config(static_patterns: ["walls/*"])
2. spatial_watch(node: "enemies/guard_01", track: ["position", "physics"])
3. [human starts capture, reproduces bug, marks, then Stops and Keeps]
4. clips(action: "markers") → find the marked frame
5. clips(action: "query_range", condition: { type: "proximity", target: "walls/*", threshold: 0.5 })
6. spatial_inspect(node: "enemies/guard_01", include: ["physics"])
```

### Pathfinding Issues
```
1. spatial_query(query_type: "path_distance", from: "guard_01", to: "player")
2. spatial_inspect(node: "guard_01", include: ["children", "spatial_context"])
3. spatial_query(query_type: "relationship", from: "guard_01", to: "walls/segment_04")
```

### AI State Machine Debugging
```
1. spatial_config(state_properties: { enemies: ["state", "alert_level", "current_target"] })
2. spatial_snapshot(groups: ["enemies"], detail: "standard")
3. spatial_watch(node: "guard_01", conditions: [{ property: "alert_level", operator: "changed" }])
4. spatial_delta() → catch state transitions
5. spatial_inspect(node: "guard_01", include: ["state", "signals"])
```

### Physics Debugging (frame-by-frame)
```
1. spatial_action(action: "pause", paused: true)
2. spatial_inspect(node: "scout_02", include: ["physics"])
3. spatial_action(action: "advance_frames", frames: 1)
4. spatial_delta() → see exactly what changed
5. Repeat 3-4
```

## Reading Spatial Output

**Bearings** — relative to perspective entity's facing:
`ahead`, `ahead_left`, `ahead_right`, `left`, `right`, `behind`, `behind_left`, `behind_right`

**Elevation** (3D only): `level` (±2m), `above_5m`, `below_2m`

**`relative` block** on each entity:
```jsonc
{ "distance": 7.2, "bearing": "ahead_left", "bearing_deg": 322, "elevation": "level", "occluded": false }
```

**`global_position`** — world position (`[x, y, z]` 3D, `[x, y]` 2D).

## Error Reference

| Error | Meaning | Fix |
|---|---|---|
| `connection_failed` (CLI) / "not connected" connection error (MCP) | Game not running or addon not enabled | Check `runtime_status`; start the selected scene through Director or Godot |
| `unknown_tool` | Invalid tool name (CLI only) | Check `stage --help` |
| `invalid_json` | Bad JSON params (CLI only) | Fix JSON syntax |
| `persistent_session_required` | One-shot call needs retained server state (CLI only) | Use the same `stage serve` MCP session; for one-shot actions omit `return_delta` |
| `scene_not_loaded` | Between scene transitions | Wait for scene to load |
| `node_not_found` | Path doesn't exist | Use `scene_tree(action: "find")` |
| Query timeout — "Addon did not respond within …" | Game frozen or at a breakpoint | Check if the game is paused |
| Capture off or waiting | This launch did not enable capture, or the project has not declared readiness | Select explicit launch options and integrate readiness; configuration alone never starts recording |
| Truncated snapshot (success response with `pagination.truncated: true`) | Snapshot hit its token budget; entities omitted | Read `pagination.showing`/`total`, reduce radius, add filters, use `summary`, or raise `token_budget` |
