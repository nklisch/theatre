---
id: stage-first-run-setup
kind: feature
tags: [stage, cli, windows, reliability]
parent: windows-native-support
release: null
completed: 2026-08-09
---

# Make Stage initialization safe on first run

Made the Stage runtime parse without registered extension classes and report a
clear error when its GDExtension is unavailable. Initialization now resolves an
explicit Godot executable, environment configuration, or `PATH`, then performs
a bounded editor import without persisting a machine-local path.

A fresh Windows initialization completed plugin import and a Director read.
The missing-extension journey emitted the expected Stage diagnostic without a
GDScript parser failure.
