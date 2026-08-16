---
id: configure-runtime-dashcam-autostart
kind: story
tags: [stage, godot, configuration, performance]
parent: null
release: null
completed: 2026-08-15
---

# Configure runtime dashcam autostart

Stage now registers `theatre/stage/dashcam/enabled` as an enabled-by-default
Godot project setting. Runtime initialization applies it before the recorder
enters the scene tree, and runtime/TCP toggles keep recorder state and reported
configuration consistent. Disabling continuous capture leaves the Stage server
and on-demand observation available. Verified with 39 Godot addon tests, the
complete workspace build and Clippy pass, all unit/integration/CLI/MCP/E2E
journeys, and Voxlar's player smoke plus live Stage snapshot with recording
disabled.
