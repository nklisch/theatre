---
id: inspect-saved-clips-without-a-running-game
tags: [stage, recording, cli]
created: 2026-09-05
updated: 2026-09-05
---

# Inspect saved clips after the game closes

The normal human handoff is reproduce, mark/save, close the game, then ask the
agent to inspect the evidence. Saved evidence should remain useful at that point.

Observed with Stage 0.3.4 and the Voxlar combined starter on Godot 4.7.1,
2026-09-05: `clips list` showed current saved clips while the game ran. After the
human closed it, `clips markers` and `clips visual_artifact` for saved
`clip_213a3749` returned `connection_failed` and instructed the agent to run the
project on port 9077. This was a fresh CLI invocation, not an established MCP
session; whether persistent sessions retain offline access is untested.

The saved SQLite file remained readable. Read-only extraction found 150 spatial
frames and 74 JPEG screenshots; a contact sheet and player positions could be
recovered without running Godot. No evidence was deleted or rewritten.

Consider a project-selected local saved-clip analysis path, or a clearly
supported explicit recording-directory input. Avoid requiring a dummy game or
new background service just to inspect durable files. Preserve the distinction
between offline saved data and live observations. This is separate from
[live ring queries](live-ring-visual-queries.md).
