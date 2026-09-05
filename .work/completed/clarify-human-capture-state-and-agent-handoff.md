---
id: clarify-human-capture-state-and-agent-handoff
kind: feature
tags: [stage, recording, feedback, ux]
parent: null
release: null
completed: 2026-09-05
---

# Make capture state and agent handoff obvious

Native controls provide Start/Stop, the configured marker shortcut, immediate human acknowledgement, separate Mark and Save now actions, and saved references. Mark retains its post-window. Stop reports its save outcome separately from disabled recording, including persistence failure. New evidence identifies its run and qualifies scene provenance as scene_at_save because buffers can span scenes. Discovery orders by persisted creation time rather than artifact-cache writes.

Integrated verification passed 607 workspace tests and 320 engine tests, formatting, warnings-denied linting, deployment and the documentation build. Actual Pi status/watch calls accepted the normalized schemas. The separately deferred intermittent accessibility crash is not claimed fixed.
