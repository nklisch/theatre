---
id: stage-immediate-viewport
kind: feature
status: active
tags: [godot]
parent: agent-godot-development-loop
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-04
updated: 2026-09-04
---

# Immediate viewport observation

## Accepted outcome

Return current rendered viewport without saving a clip, with run/frame provenance. Explicitly report unavailable headless pixels while preserving spatial observation.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Design

**Primary lens:** new work, engine/server integration.

### Chosen approach

Add one Stage viewport observation tool and one typed engine query. Capture the
latest completed render on demand without enabling dashcam, saving a clip, or
changing existing capture cadence. Return an MCP image plus compact metadata:
actual runtime identity, dimensions, current physics frame and render counter.
The metadata must distinguish readback time from an exact rendered simulation
frame; pixels and a separate spatial query are not an atomic world snapshot.

Use Godot viewport readback and native JPEG encoding on the engine thread for
this explicitly requested operation. Default to a bounded image size, preserve
aspect ratio, and reject invalid size requests with actionable parameter errors.
The transport's existing message size limit remains authoritative. No additional
worker lifecycle or image cache is needed for an occasional bounded request.
Leave the measured continuous-recorder worker and capture path unchanged.

Headless display, missing viewport, or an empty image returns explicit visual
unavailability, not an empty image or unchanged-world claim. Spatial tools remain
available. Rust shared protocol types own the request and result. Server-side
handling converts the result into MCP image/text content and CLI JSON using the
existing multimodal convention.

### Alternatives and verification

Saved clips introduce unnecessary persistence and delay. Reading the rolling
buffer alone cannot work when recording is disabled and may return old frames.
A request-specific rendering scheduler or encoding service is unnecessary unless
bounded on-demand capture demonstrates unacceptable latency.

Verify schemas/parameter errors and image-content handling at the tool boundary.
Use real graphical Godot to capture and decode a recognizable viewport with
recording disabled; change the scene and observe a new image. Check a headless
run returns visual unavailability while a spatial query still succeeds. Keep
existing recorder tests intact and compare capture latency during the journey.

One standard Astra design review completed with no material findings. Output
size bounds encoding and transport, not the source viewport readback cost. Measure
latency on the real graphical viewport including the largest accepted request;
retain the continuous recorder worker unchanged unless evidence justifies change.

### Integrated review

One standard Astra implementation review found no material defects. Its minor
protocol-catalog omission is assigned to the diagnostics worker, which owns that
shared catalog update. The installed-engine and migration verification establish
working graphical/headless behavior; final combined gates and generated references
remain pending. No repeat review is required for the catalog correction.
