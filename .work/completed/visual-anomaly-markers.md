---
id: visual-anomaly-markers
kind: feature
tags: [stage, recording, visual]
parent: null
release: null
completed: 2026-07-25
---

# Visual anomaly detection → auto system-tier markers/clips

The encode worker now computes a strided-lattice changed-pixel metric on
captured frames, and a dual-gate detector (absolute + EMA-relative,
sustained-N, cooldown) fires system-tier dashcam clips with
"visual_anomaly:" labels — conservative by default, tunable via dashcam
config end-to-end (stage.toml, clips config, status block with honest
active/inactive reasons). Cross-model review caught that the EMA chased the
spike making the detector unfireable with defaults; fixed with
baseline-freeze semantics plus a default-settings regression test.
Verified: workspace green, E2E journey passing (quiet → no triggers,
hair-trigger → system clip, headless honest).
