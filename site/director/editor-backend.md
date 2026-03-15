---
description: "Editor Plugin backend — Director operations that run through Godot's editor for full API access and undo support."
---

# Editor Plugin Backend

The editor plugin backend is Director's preferred mode of operation. When you have the Godot editor open with the Director addon enabled, all Director operations route through this backend automatically.

## How it works

When the Director addon is enabled in the Godot editor (**Project Settings → Plugins → Director → Enable**), the `plugin.gd` EditorPlugin starts a TCP listener on **port 6551** (localhost only).

When the `director` binary receives an MCP tool call, it attempts to connect to port 6551. If the connection succeeds, the operation is forwarded to the running editor over the TCP connection, executed inside the editor process, and the result is returned.

The editor process has access to:
- Full `EditorInterface` — can save resources, reload scenes, update the editor UI
- `ResourceSaver` — saves scenes and resources to disk correctly with UIDs
- `ProjectSettings` — can read and write project settings (layer names, etc.)
- All Godot engine APIs — the same environment available in `@tool` scripts

## Why the editor backend is best

**Instant feedback.** When Director creates a node or sets a property, you see the result immediately in the editor's scene tree and inspector. No need to reopen the file.

**Correct resource serialization.** The editor's `ResourceSaver` handles UIDs, import metadata, and embedded resource paths correctly. Hand-written `.tscn` files often have subtle serialization errors; the editor never does.

**EditorUndoRedoManager integration.** Operations submitted through the editor backend create undo history entries — you can Ctrl+Z to undo a Director change if it was wrong.

**No separate process.** Everything runs inside the already-open editor. No startup latency, no port management.

## Limitations

**Requires the editor to be open.** If you close the editor, this backend becomes unavailable. Director falls back to the daemon backend automatically.

**One project at a time.** The editor backend operates on the currently open project. If you call Director with a `project_path` that does not match the open project, the operation is forwarded to the daemon or one-shot backend instead.

**Not available in CI/CD.** Automated pipelines do not have a GUI editor running. Use the daemon backend for CI workflows.

## Port configuration

The default port is 6551. To change it:

In **Project Settings → Theatre → Director → Editor Port**, set a different port number.

Update the same port in your Director configuration if you change it:

```json
{
  "director": {
    "editor_port": 6552
  }
}
```

(Note: if you use a custom editor port, set the daemon port to something else to avoid conflicts.)

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
