---
id: node-following-filmstrip
kind: feature
tags: [stage, recording, visual]
parent: null
release: null
completed: 2026-07-25
---

# Region filmstrip that follows a node

New `visual_artifact` kind `node_filmstrip`: fixed-size crops tracking a
node's projected screen position across a clip, with honest per-tile
statuses (off-screen/behind camera/node absent/camera absent render as
padded tiles). The recorder now captures active-camera pose per frame
(camera_frames clip table; old clips degrade with no_camera_data);
projection math lives in stage-core; temporal-vision gained an additive
tracked-filmstrip API in the sibling repo. Review caught an R-vs-Rᵀ
projection bug (wrong for every rotated camera) and a manifest/renderer
tile-selection divergence; both fixed with rotated-camera unit tests and a
decircularized analytic fixture. Verified: workspace green, fixture
integration + determinism + cache tests, headless E2E 8/8. Caveat: the
live windowed journeys (tests/live-tests/test_node_filmstrip.rs) are
written but unverified — the host GPU driver was wedged during this work
(new Vulkan device creation fails system-wide); rerun `cargo test -p
live-tests -- --include-ignored` with GODOT_BIN after a reboot.
