---
id: live-ring-visual-queries
tags: [stage, recording, visual]
created: 2026-07-25
updated: 2026-07-25
---

# Visual queries over the live recorder ring

Parked during visual-storyboards ideation (2026-07-25). "Storyboard the
last N seconds" directly from the recorder's in-memory ring without saving
a clip. Requires a new TCP query to pull screenshot ranges from the
GDExtension and makes ring retention/eviction semantics user-visible. v1
deliberately scoped to saved clips only; revisit if the deliberate-clip-save
detour proves awkward in practice.
