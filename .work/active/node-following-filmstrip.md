---
id: node-following-filmstrip
kind: feature
status: active
tags: [stage, recording, visual]
parent: null
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-07-25
updated: 2026-07-25
---

# Region filmstrip that follows a node

Parked during visual-storyboards ideation (2026-07-25). temporal-vision's
region filmstrip ("same crop across evenly-spaced frames") becomes the
killer artifact if the crop can track a node's world→screen projection —
"filmstrip following the enemy as it moves." Requires projecting node world
positions into viewport/screen space per frame and handling off-screen
frames (temporal-vision's SignedPixelRect + PaddingInsets already model
out-of-image regions). Depends on the visual-storyboards pipeline existing.

## Settled requirements (2026-07-25)

- Crop semantics: fixed-size crop (fraction of frame) centered on the node's
  projected screen position each frame; off-screen frames become padded tiles
  (temporal-vision SignedPixelRect/PaddingInsets already models this).
- Not tight-to-AABB (v1). Node→screen projection needs per-frame camera pose
  and node world position — design must find where that data comes from
  (clip spatial frames + camera capture, possibly a recorder addition).
- Delivered as a new visual_artifact kind on the clips tool.

## Design (k3, 2026-07-25)

- Camera data is NOT in clips today → recorder captures active camera pose
  per spatial frame (position + quaternion + projection/fov/ortho/keep_aspect,
  ~60B, always-on no flag) into new `camera_frames` clip table (schema v8);
  CameraFrameData wire type in stage-protocol; old clips degrade with honest
  no_camera_data error.
- New stage-core/src/projection.rs (pure): Godot-convention world→screen
  projection (quaternion basis, -Z forward, perspective keep-height/width +
  orthogonal), OnScreen/OffScreen(finite px)/BehindCamera.
- temporal-vision extension (additive, krometrail repo):
  generate_tracked_region_filmstrip — per-tile moving SignedPixelRects,
  union-crop normalization, header honestly says TRACKING <label> | PER-FRAME
  REGION. Rejected server-side crop baking (would render false "TRACKING
  NONE" claim). TV_VERSION cache salt bumped to "2".
- MCP: new artifact kind `node_filmstrip`; node param required (exact path
  match); new crop_fraction param (default 0.25); tiles via temporal-vision
  select_indices evenly-spaced (informative selection is blind to the
  tracked subject); per-tile statuses (on_screen/off_screen/behind_camera/
  node_absent/camera_absent) + projection counts in manifest; 2D scenes
  rejected honestly; camera switches counted.
- Errors: no_camera_data, unsupported_scene_2d, node_not_found (with
  sample_paths), node_not_projectable — all content-level degraded JSON.
- Tests: projection math unit tests with known Godot setups; camera table
  round-trip; fixture-clip with painted dots at analytically projected
  pixels asserting tile centers; determinism + cache; live-tests journeys
  (moving node, off-screen padding, unknown node).
