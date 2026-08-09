---
id: portable-cli-setup
kind: feature
tags: [cli, windows, portability]
parent: windows-native-support
release: null
completed: 2026-08-09
---

# Make CLI setup portable across operating systems

Added target-aware executable naming for native binaries while preserving the
platform-specific GDExtension filename. Generated MCP configuration now uses
the portable `stage` and `director` commands resolved through `PATH`, with
additive PowerShell and POSIX guidance in the CLI and current documentation.

Shared CLI fixtures and assertions are platform-aware, genuine Unix-only tests
remain gated to Unix, and native Windows CLI coverage now runs in CI. A source
install and initialization journey produced the expected Windows executables,
`stage_godot.dll`, and path-free MCP configuration.
