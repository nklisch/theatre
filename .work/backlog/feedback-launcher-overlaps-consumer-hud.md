---
id: feedback-launcher-overlaps-consumer-hud
tags: [stage, feedback, ui]
created: 2026-09-05
updated: 2026-09-05
---

# Check feedback launcher placement against consumer HUDs

During Voxlar windowed review with Theatre 0.3.4 and Godot 4.7.1,
the Stage feedback launcher and its “Share feedback” tooltip occupied the
same upper-left area as the game's status/header controls. This is a
non-blocking presentation concern; inspect existing visibility/placement
options before assuming another configuration surface is needed.

Evidence: Stage's completed-render viewport capture at
`/storage/voxlar-smooth-reconstruction/godot-integration/windowed/viewport.jpg`
with adjacent `viewport.json` records process/run and render provenance.
Consumer: Voxlar `70b288c`, combined starter, 1280×720 viewport.

An initial suspicion about Tab input was disproved: Stage Tab press/release
correctly changed the game's `_tool_selection_mode`. Native X11 window capture
had returned stale pixels. Do not treat key injection or feedback keyboard
interference as demonstrated defects from this session.
