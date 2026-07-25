---
id: visual-anomaly-markers
tags: [stage, recording, visual]
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
