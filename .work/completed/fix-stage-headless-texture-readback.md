---
id: fix-stage-headless-texture-readback
kind: story
tags: [stage, godot, headless, regression]
parent: null
release: null
completed: 2026-08-15
---

# Fix Stage headless texture readback

Stage now detects Godot's headless display server before viewport readback,
preserving spatial and nonvisual dashcam capture without emitting repeated
`texture_2d_get` errors. The screenshot E2E journey asserts the headless error
stream stays clean. Verified through the complete workspace build, Clippy, and
unit/integration/CLI/MCP/E2E suite, plus a live Voxlar spatial snapshot.
