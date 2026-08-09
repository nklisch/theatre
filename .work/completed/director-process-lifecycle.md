---
id: director-process-lifecycle
kind: feature
tags: [director, windows, processes, reliability]
parent: windows-native-support
release: null
completed: 2026-08-09
---

# Own the Director Godot process lifecycle

Introduced an owned-child boundary with bounded output collection, shutdown,
termination, and reaping. Windows Godot processes run beneath an internal
supervisor assigned to a kill-on-close job, so terminating the supervisor also
terminates its descendants. Daemon readiness now requires a successful ping,
and fallback errors preserve both daemon and one-shot context.

The Windows descendant regression, all 193 Director Godot tests, the native
Director journey, and post-run process checks passed without surviving owned
Godot processes.
