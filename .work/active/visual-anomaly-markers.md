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

## Design (k3, 2026-07-25)

- Metric: strided-lattice changed-pixel proportion (≤16k samples, integer
  luma via temporal-vision weights, noise floor 24) computed IN THE ENCODE
  WORKER on RawShot RGBA pre-encode (<0.5ms/frame, no Gd off-thread).
  EncodedShot carries FrameAnalysis{proportion, reset, analysis_ms} back.
- Trigger: pure AnomalyDetector state machine on main thread — anomalous iff
  proportion >= 0.30 (absolute) AND >= 4.0×max(ema, 0.02 floor) (relative);
  sustained 4 consecutive anomalous frames; 30s fresh-trigger cooldown plus
  existing system-tier merge rate limiting. Fires via
  on_dashcam_marker_with_tier(system) → all existing clip machinery free.
- Fades/cuts rule: cuts die on sustained-N, fades die on noise floor,
  genuine sustained strobes are intended triggers. Resolution change →
  lattice reset, streak zeroed, never a trigger.
- Config: 6 anomaly_* DashcamConfig fields (defaults conservative, enabled
  by default), echoed/parsed like existing; stage.toml [dashcam] fields +
  handshake push (dashcam_explicit pattern); clips(config) works unchanged.
- Status: dashcam_status gains "anomaly" block (active+reason honesty,
  frames_analyzed/skipped, ema, streak, triggers, suppressed_cooldown,
  analysis_ms_ema/max in capture_probe).
- Trigger label: "visual_anomaly: change 0.47 vs baseline 0.06 (7.8x)";
  list_recordings SQL prefers human/agent labels but surfaces anomaly label
  otherwise.
- Tests: pure-Rust unit tests for lattice + state machine (no Godot), config
  parse, E2E journey (quiet→no triggers; hair-trigger config→triggers with
  system clip + label; headless honesty), tcp_mock additions.
