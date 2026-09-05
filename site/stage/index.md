---
description: "Observe and interact with a running Godot game through spatial state, current viewport images, diagnostics, input, and retained clips."
---

# Stage

Stage gives an agent direct evidence from a running Godot game. It combines
structured scene state with explicit debug actions, current viewport images,
bounded process diagnostics, and retained clip analysis.

Stage does not stay read-only. `spatial_action` can change the current game for
debugging, including property changes, method calls, input, pausing, and bounded
interaction sequences. These changes are temporary and do not save project files.

## Start with identity

Call `runtime_status` before a run-sensitive workflow. It identifies the actual
Godot project, process, run, current scene, and readiness. A TCP connection or a
Director launch request alone does not establish that the scene completed its
ready notification.

Then use a summary `spatial_snapshot` and narrow the question with inspection,
queries, filters, or a smaller radius. Use persistent MCP when a workflow needs
a delta baseline, watches, or session configuration across calls. Each one-shot
CLI invocation has fresh session state.

## Capability groups

- **Live identity and health:** `runtime_status` and `runtime_diagnostics`.
- **Current visual evidence:** `viewport`, independent of recording.
- **Structured state:** snapshots, deltas, focused inspection, hierarchy, and
  spatial queries.
- **Debug controls:** pause, frame advancement, temporary mutations, input, and
  bounded paused interaction sequences.
- **Retained evidence:** dashcam markers, saved clips, temporal queries, and
  generated visual artifacts.
- **Human feedback:** project-local runtime or editor evidence that can be
  retrieved after the engine exits.

The [generated Stage reference](/api/) owns the complete current tool and
parameter catalog.

## Current viewport and diagnostics

`viewport` returns a bounded JPEG of the latest completed root-viewport render.
It does not enable recording or save a clip. Its run identity and readback
counters describe provenance, but the pixels are not atomic with a separate
spatial query. Headless and empty-pixel outcomes leave structured observation
available.

`runtime_diagnostics` returns bounded errors, warnings, script errors, and shader
errors captured after the Stage autoload registered its Logger. Reads do not
consume entries. Diagnostics survive client reconnects but not a game restart.
They do not replace project validation or a source debugger.

## Human feedback

The runtime **Share feedback** control lets a developer review a captured root
viewport, add an optional note, and queue it with pointer and run context. Editor
feedback uses the same project-local queue with the active 2D or 3D viewport and
selection context.

A `feedback_notice` on a later result means evidence is pending. Use `feedback`
status, retrieve the matching item, and handle it only after addressing it.
Retrieval is non-destructive. Handling suppresses notices for every reader but
keeps the item. Deletion remains separate and explicit.

## Limits

Stage reports the engine state that its collector exposes. A snapshot is current
to a collected physics frame, not a frozen world. Interaction sequences release
their held named actions during supported completion and cleanup, but they do not
make gameplay deterministic. A stopped or natively hung engine cannot execute
cleanup callbacks.

Capture has real main-thread cost. Current viewport reads are bounded and
on-demand; the continuous recorder keeps its separate capture path. Use the
smallest evidence that answers the question rather than assuming an unmeasured
universal overhead.
