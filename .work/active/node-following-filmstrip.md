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
