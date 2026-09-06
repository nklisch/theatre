---
id: headless-editor-import-shutdown-abort
tags: [godot, verification, editor]
created: 2026-09-06
updated: 2026-09-06
---

# Investigate headless editor abort after isolated project import

While adding the project-switch runtime journey, Godot 4.7.1 aborted during the
initial editor pass on a temporary copy of tests/godot-project (source/addons,
excluding .godot, .stage, .theatre and tmp). Both `godot --headless --editor --quit
--path <copy>` and `godot --headless --import --path <copy>` ended with SIGABRT
(signal 6). Expected a successful import before running the game. Stdout reached
completed filesystem, script-class and editor-layout initialization; captured
stderr was empty. No Stage MCP connection had been attempted.

Local system core evidence for process 1153473 showed a signal-handler abort and
unsymbolized Godot frames including offsets 0x187697f, 0x18781d4 and 0x1880955.
There was no captured AccessKit duplicate-child diagnostic; relationship to the
separate native undo crash is unproven. Engine: 4.7.1.stable.official.a13da4feb;
Theatre addon: 0.5.0 plus the monitoring guard under development, Linux x86_64.
Memory/disk exhaustion was not observed. This is an environment/engine/addon
interaction to investigate, not a diagnosed Stage selection defect.

The runtime-switch test can copy generated extension/class discovery metadata
from the already-imported fixture, avoiding a new editor lifecycle while still
starting two real runtimes with distinct project roots. This does not establish
that fresh editor imports work or that the abort is fixed.
