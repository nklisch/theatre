---
description: "Editor Plugin backend — Director operations that run through Godot's editor for full API access and undo support."
---

# Editor Plugin Backend

The editor plugin backend is Director's preferred mode of operation. When you have the Godot editor open with the Director addon enabled, all Director operations route through this backend automatically.

## How it works

When the Director addon is enabled in the Godot editor (**Project Settings → Plugins → Director → Enable**), the `plugin.gd` EditorPlugin starts a TCP listener on **port 6551**.

The `director` binary connects through the configured local address and verifies
the editor's canonical project before dispatch. The GDScript listener does not
set an explicit bind address and has no authentication. Keep it off untrusted
networks.

The editor process has access to:
- Full `EditorInterface` — can save resources, reload scenes, update the editor UI
- `ResourceSaver` — saves scenes and resources to disk correctly with UIDs
- `ProjectSettings` — can read and write project settings (layer names, etc.)
- All Godot engine APIs — the same environment available in `@tool` scripts

## Why the editor backend is best

**Instant feedback.** When Director creates a node or sets a property, you see the result immediately in the editor's scene tree and inspector. No need to reopen the file.

**Correct resource serialization.** The editor's `ResourceSaver` handles UIDs, import metadata, and embedded resource paths correctly. Hand-written `.tscn` files often have subtle serialization errors; the editor never does.

**EditorUndoRedoManager integration.** Individual and batch operations against an
open scene create native undo entries. They preserve existing human history and
remain unsaved until an explicit `scene_save`.

**Selected save.** `scene_save` serializes only the selected scene and retains
undo. It does not flush unrelated external resources, and Godot's native dirty
marker may remain.

**Native run control.** `editor_run` starts, stops, or restarts a selected saved
scene without implicitly saving open work. Stage `runtime_status` separately
establishes runtime readiness.

**No separate process.** Everything runs inside the already-open editor.

## Limitations

**Requires the editor to be open.** If you close the editor, this backend becomes unavailable. Director falls back to the daemon backend automatically.

**One project at a time.** The editor backend operates on its verified current
project. A mismatch is rejected before editor dispatch and can use a supported
headless fallback for the requested project. After dispatch, a lost response has
an unknown outcome and is never replayed automatically.

**Not available in CI/CD.** Automated pipelines do not have a GUI editor running. Use the daemon backend for CI workflows.

## Port configuration

The editor plugin and Director client resolve the port in this order:

1. `DIRECTOR_EDITOR_PORT` in both processes' environment
2. `connection/editor_port` under the `[director]` section of `project.godot`
3. the default, `6551`

Set the project value through **Project Settings → Director → Connection → Editor
Port**, or edit the equivalent project setting:

```ini
[director]

connection/editor_port=6552
```

When using `DIRECTOR_EDITOR_PORT`, give the editor and the agent process the same
value. Keep a custom editor port distinct from the daemon port.

## Verifying the backend is active

When the editor backend is listening, the Godot output panel shows:

```
[Director] Editor plugin listening on port 6551
```

You can also check the Director dock (right side of the editor), which shows:

```
● Director: Editor backend active (port 6551)
```

## Editor dock

When the Director addon is enabled, a Director dock panel appears. It shows:

- **Backend status**: Editor / Daemon / One-shot
- **Recent operations**: log of recent Director tool calls and their results
- **Active port**: which port the editor backend is listening on

The dock is informational only — all Director operations are triggered by the AI agent, not through the dock UI.
