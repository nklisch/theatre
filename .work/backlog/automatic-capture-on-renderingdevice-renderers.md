---
id: automatic-capture-on-renderingdevice-renderers
tags: [stage, capture, rendering]
created: 2026-09-05
updated: 2026-09-05
---

# Retain the automatic-capture limitation on RenderingDevice renderers

Automatic image capture currently supports Compatibility/OpenGL. Forward+ continues spatial recording but reports visuals unavailable; this is an intentional current capability boundary, not a startup failure or a confirmed asynchronous-capture defect. Additional renderer support is outside the responsive-dashcam repair.

The complete engine run exposed four older CLI/MCP visual-journey failures because their fixture defaulted to Forward+. After 120 frames, `clips(action: "status")` reported no buffered screenshots. A saved clip's `node_filmstrip` request for `Enemies/DefinitelyMissing` consequently returned `no_screenshots`, rather than reaching the test's expected `node_not_found` check.

The dedicated regression now verifies Godot 4.7.1 Forward+ on Vulkan 1.4.329 with the NVIDIA RTX 4070. The current capture status is:

```json
{
  "available": false,
  "backend": "unavailable",
  "pending": false,
  "reason": "Native asynchronous capture requires Compatibility/OpenGL; synchronous recovery is explicit"
}
```

Explicitly setting `screenshot_readback: "synchronous"` restores valid rendered images on the same renderer, at the cost of blocking readback. Spatial frames remain available before recovery. Evidence lives in `forward_plus_auto_is_unavailable_until_explicit_synchronous_recovery` in `crates/stage-server/tests/native_readback_engine.rs` and its existing Godot journey. Automatic-image live fixtures now explicitly select Compatibility; consumer renderer settings were not changed.

This preserves a concrete limitation for any later request to extend automatic capture to RenderingDevice renderers, without selecting a backend design or requiring that extension for the current delivery.
