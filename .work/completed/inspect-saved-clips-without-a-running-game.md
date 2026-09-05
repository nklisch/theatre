---
id: inspect-saved-clips-without-a-running-game
kind: feature
tags: [stage, recording, cli]
parent: null
release: null
completed: 2026-09-05
---

# Inspect saved clips after the game closes

Successful capture publishes Godot’s resolved storage path for fresh-process retained inspection after shutdown. Saved list, markers, state, visuals and deletion do not require a live runtime when storage resolves. Markers survive between spatial samples, and visual artifacts report independently timed markers outside image coverage instead of failing or inventing pixels.
