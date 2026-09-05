---
id: responsive-dashcam-capture
kind: feature
tags: [stage, recording, performance]
parent: null
release: null
completed: 2026-09-05
research_refs: [.research/briefs/responsive-dashcam-capture.md]
---

# Keep capture responsive during play

Automatic Compatibility/OpenGL capture downsamples the existing viewport on the GPU and retrieves completed pixels asynchronously. Spatial only and explicit synchronous recovery remain available. Admission is bounded before expensive work; old generations retain resource ownership without publishing stale pixels or counting losses twice. Live exported-property filtering runs inside Godot without caching state or changing tracking policy.

The bounded Voxlar release-extension check improved Lightweight frame-pacing p95 from 63.744 to 42.537 ms and measured image work from 6.993 to 0.855 ms. The user reports substantially improved usability. Recording still has overhead; these measurements are not a universal budget.

Verification passed workspace build, formatting, warnings-denied linting, 611 ordinary tests, all 326 real-engine tests, documentation build, installation and consumer deployment. Rust 1.98 checks and hosted CI passed, including native Windows tests, Apple Silicon/Windows release builds and Godot CI journeys. Independent integrated review found no blocking issue; its duplicate-gap accounting finding was corrected and regression-tested. Windows/macOS graphical runtime qualification remains unperformed, and the intermittent AccessKit undo crash remains unrepaired.

Voxlar retains its Compatibility renderer, nested-project MCP selection and explicit recording-disabled startup setting. Native payloads and addon scripts match the installed templates.
