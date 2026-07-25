---
id: visual-storyboards
kind: feature
tags: [stage, recording, visual]
parent: null
release: null
completed: 2026-07-25
---

# Agent-facing visual storyboards for clips

The dashcam now captures dense viewport screenshots (default every 4 physics
frames, 480px, off-main-thread JPEG encode, byte-capped ring with an honest
gap ledger and always-on capture probe), and stage-server generates
deterministic temporal visual artifacts — storyboard montages, motion-history
images, and difference maps — from saved clips via the temporal-vision crate,
served through new `clips` MCP actions `visual_artifact` (manifest + bounded
PNG, content-addressed cache in the clip DB) and `config`. Perf probe gate
passed (dense capture: no pacing regression, zero drops); cross-model review
findings (gap time-base bug, inverted inferred gaps, cache and honesty fixes)
were fixed and re-verified: workspace build/clippy/fmt/tests green, all 7 E2E
journeys pass. Research: .research/briefs/krometrail-visual-temporal.md.
