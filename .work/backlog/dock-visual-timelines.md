---
id: dock-visual-timelines
tags: [stage, addon, visual]
created: 2026-07-25
updated: 2026-07-25
---

# Human-facing filmstrips in the editor dock

Parked during visual-storyboards ideation (2026-07-25). Render temporal
artifacts (filmstrips, motion history) in the Godot editor dock for the
human reproducing a bug. Different consumer than the agent-facing v1: use
temporal-vision's plan-only outputs (MotionHistoryPlan, StoryboardSelection)
and render via native Godot UI (TextureRect/Control), not the crate's PNG
renderer. Note the hybrid architecture: plans are computed server-side
today, so this may need either a thin analysis path in-process or a server
round-trip from the dock.
