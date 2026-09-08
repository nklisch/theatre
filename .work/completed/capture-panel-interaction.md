---
id: capture-panel-interaction
kind: feature
status: completed
completed: 2026-09-08
---
# Reliable, movable capture controls

Capture controls isolate their input from eager gameplay handlers using a
non-modal Godot child Window, without changing project embedding settings or
mouse mode. The header is draggable; minimizing leaves one restore button and
does not change recording. Placement is run-local and clamped on viewport resize.
Captured gameplay input passes through the panel.

Reproduced the original click failure before correction. Windows Godot 4.7.1
journeys verify embedded pointer dispatch, held/repeated clicks, preset selection,
drag, resize, pause, recording continuity, and native-window keyboard operation.
The workspace build, formatting, strict Clippy, ordinary tests and complete
ignored engine layers passed after correcting and rerunning the capture target.
Site, generated skills and Workbench/index checks passed. The bounded inline
review covered input ownership, focus, layout, capture invariants and documentation.

OS-level native-window mouse automation and Linux/macOS execution were not
verified. No consumer files, private preferences or ignore rules were changed.
