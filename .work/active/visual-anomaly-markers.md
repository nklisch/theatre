---
id: visual-anomaly-markers
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

# Visual anomaly detection → auto markers/clips

Parked during visual-storyboards ideation (2026-07-25). Use difference-map
or changed-pixel-proportion spikes over the screenshot ring to auto-trigger
system-tier dashcam clips and markers — the machine watches pixels and
tells the agent where to look, complementing velocity-spike system markers.
Reuses the v1 analysis pipeline but in the hot path, so it needs the perf
profile settled first. Consider pairing with spatial gates (only analyze
when spatial watches are active) to bound cost.

## Settled requirements (2026-07-25)

- Detection runs in-engine: stage-godot computes a cheap changed-pixel metric
  on captured RGBA frames (pre-encode, pure Rust on worker-owned data) and
  fires a system-tier dashcam marker on sustained spikes. No live-ring TCP
  streaming; no post-hoc-only mode.
- Posture: conservative — high threshold + sustained-spike requirement,
  existing system-tier rate limiting applies. Never floods.
- Thresholds/config tunable via dashcam config (server-pushable).
