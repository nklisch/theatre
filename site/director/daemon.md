---
description: "How Director uses its supervised headless Godot daemon and one-shot fallback when no verified editor is available."
---

# Headless Daemon Backend

Director uses a persistent headless Godot daemon when no verified editor backend
is available. The Rust server supervises this process and sends native operation
requests over the configured daemon port.

## Backend order

For supported operations, Director tries:

1. A verified editor connection on the configured editor port.
2. The supervised headless daemon on the configured daemon port.
3. A one-shot headless Godot subprocess.

Fresh editor identity failure can continue to the requested project's headless
backend. A transport failure after editor dispatch has an unknown outcome and is
not replayed. `editor_run` is editor-only and does not use headless fallback.

## Persistence

The daemon and one-shot paths load detached scene or resource contexts and persist
their target files through Godot serialization. They do not share live objects or
native undo history with an open editor.

The editor path behaves differently: mutations against an open scene use its live
root and native undo, then remain unsaved until `scene_save`. Read the operation's
persistence result instead of inferring it from the selected backend.

## Process ownership

The Director server owns daemon startup, reuse, and termination. Callers do not
need to start a separate daemon for ordinary MCP or CLI use. `project_reload`
stops a stale daemon and runs a fresh Godot-backed validation pass so later
operations can see changed scripts.

Standalone Godot resolution uses `GODOT_BIN`, then `GODOT_PATH`, then `godot` on
`PATH`. The daemon port can be selected with `DIRECTOR_DAEMON_PORT`. Keep the
client and listener configuration aligned.

## Limits

Headless Godot provides engine types and resource serialization, but not editor
state, editor-native undo, scene tabs, or graphical run control. Use the editor
backend when those capabilities matter. Do not infer fixed latency or speedup
from backend type alone; project import state and operation cost vary.
