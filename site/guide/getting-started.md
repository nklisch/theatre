---
description: "Enable only the Theatre observation or capture needed for your current Godot investigation."
---

# Getting Started

Theatre installs tools for authoring and inspecting Godot projects. Installation,
project wiring and runtime activation are separate choices. Ask before adding
missing Theatre components to a project. After [Installation](/guide/installation),
ordinary game launches leave Stage observation and recording off.

## Observe a selected scene

With the project's Godot editor open and Director enabled, request live access:

```sh
theatre run scenes/review.tscn --observe on
```

Use your actual saved scene path relative to the project. The CLI delegates to
Director's existing editor-owned launch. It does not modify the main scene or
save unrelated editor work. Equivalent Director calls use
`editor_run.launch: {"observe": true}`.

1. Call `runtime_status` and verify the actual project, run and current scene.
2. Use `scene_tree` to find a node path.
3. Request `spatial_snapshot` at summary detail.
4. Narrow the investigation with `spatial_inspect`, `spatial_query`, a group or
   a smaller token budget.
5. Treat each observation as current engine evidence, not a frozen world snapshot.

Observation does not implicitly retain history. Director's scene authoring tools
and already saved clips remain usable without an active Stage listener.

## Integrate readiness before phased capture

Projects with asynchronous loading must explicitly tell Stage when the intended
activity is ready:

```gdscript
# At the actual successful initial-loading completion transition:
var stage := get_node_or_null("/root/StageRuntime")
if stage != null:
    stage.notify_ready()
```

Use this checklist:

1. Identify all work required before the intended review activity is usable.
2. Add the call at the existing transition after that work succeeds.
3. Do not notify on failure, cancellation, an arbitrary delay, or the first
   completed task. A scene's `_ready()` is not proof that async work finished.
4. Test success, failure, cancellation and repeated notification. Duplicate
   calls are safe; later background tasks must not reopen startup capture.
5. For truly synchronous scenes only, explicitly choose `--readiness scene`
   instead of implementing a project signal.

The default is project-provided readiness. A missing notification leaves capture
waiting or stopped at its bound, never silently ready. `runtime_status.ready`
reports Godot node readiness; `clips(status).ready` reports the capture boundary.

## Request evidence for human review

```sh
theatre run scenes/review.tscn --startup minimal --play standard --play-scope group:review --operator human
```

Use a real group or selected node paths. Startup records before the target scene
loads and saves at readiness. Human play then waits without recording until the
person clicks Start new or Continue. Continue adds a separate segment to the
same recording; the waiting gap remains uncaptured. The game itself is not
automatically paused.

If only timing and counters are needed, use `--play minimal`.
Select `--observe on` separately if the agent also needs live access.
See [Recording](/stage/recording) for bounds, providers, saving policies and
private local overrides.

## Common issues

**Connection error:** first check whether observation was deliberately enabled.
An ordinary off launch is not a broken installation. Then check the running
project, port and firewall.

**Waiting for readiness:** locate the project's completion transition and add
the notification. Do not hide an async loading problem with the scene fallback.

**Light or Standard rejects missing scope:** supply scene-relative node paths
or `group:name`; use Heavy only when broad capture is genuinely needed.

**Images unavailable:** inspect capture capability. Auto never silently switches
to blocking readback; metrics and spatial evidence can still be usable.

**Missing tools:** verify the project configuration and that `stage` and
`director` resolve through PATH (`.exe` on Windows). Ask before installation.

Next: [First debugging session](/guide/first-session),
[Dashcam workflow](/stage/dashcam), [Human feedback](/guide/human-feedback).
