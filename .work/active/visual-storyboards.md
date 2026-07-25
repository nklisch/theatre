---
id: visual-storyboards
kind: feature
status: active
tags: [stage, recording, visual]
parent: null
blocked_by: []
related_to: []
research_refs: [.research/briefs/krometrail-visual-temporal.md]
mock_refs: []
created: 2026-07-25
updated: 2026-07-25
---

# Agent-facing visual storyboards for clips

## Intent

Let the agent ask "show me what happened" over a saved dashcam clip and get
back temporal visual artifacts — a storyboard montage, a motion-history
image, and a difference map — generated server-side from the clip's
screenshot frames. Extends the existing marker → clip → scrub debugging
loop with pixels, inspired by krometrail's temporal-vision system.

## Settled decisions (ideation 2026-07-25)

- **Primary job:** agent-facing artifacts via MCP image content. Not dock
  UI, not anomaly detection (both parked).
- **Scope:** saved clips only. Ad-hoc curiosity goes through deliberate clip
  saves; no live-ring TCP surface in v1.
- **Artifacts:** storyboard (3–12 informative frames, selection reasons,
  labeled montage + JSON manifest), motion history (recency-decayed movement
  image), difference map (frequency + timing panels vs a reference frame).
- **Density model:** the screenshot ring becomes always-on moderate density
  (low-res, ~15–20 fps target, JPEG encode off the main thread, byte-capped,
  typed gap ledger on drops) — replacing today's 2 s/960 px defaults.
  **Gate:** a perf probe on realistic scenes must show no gameplay hitch
  from dense readback; if it does, fall back to trigger-armed dense bursts.
- **Analysis location:** stage-server only (thin engine boundary).
  temporal-vision consumed as a path dependency
  (`../krometrail/crates/temporal-vision`) with a `version` key so it swaps
  to crates.io once published.
- **Generation:** on-demand from clip SQLite frames, never in the live path;
  deterministic output enables a content-addressed artifact cache
  (clip id + params hash).
- **Response shape:** compact JSON manifest always (selection reasons,
  omitted counts, gap/cadence honesty) + bounded composite PNG inline by
  default with an opt-out, mirroring the budget discipline of existing
  tools.

## Open implementation questions

- Ring capture config: exact resolution/interval/byte-cap after the perf
  probe; whether readback can downscale on-GPU before CPU readback.
- JPEG → RGBA8 decode step server-side (temporal-vision's sole input
  format); HDR viewport conversion if needed.
- MCP surface: new actions on the existing `clip` tool vs a separate tool.
- Where the artifact cache lives (per-clip SQLite table vs files alongside).

## Acceptance evidence

- Perf probe numbers (frame-time impact of dense capture) recorded in this
  item before ring defaults change.
- E2E journey: record clip with marker → request storyboard → assert
  montage image + manifest with selection reasons and gap honesty.
- Unit tests for artifact generation determinism (same clip + params →
  same bytes).
