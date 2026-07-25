---
id: node-following-filmstrip
tags: [stage, recording, visual]
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
