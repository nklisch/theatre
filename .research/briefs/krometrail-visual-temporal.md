---
id: krometrail-visual-temporal
kind: research-brief
summary: What theatre can borrow from krometrail's temporal visual analysis for agent-facing clip storyboards
question: What can theatre borrow from krometrail's temporal visual analysis for agent-facing clip storyboards?
status: current
work_refs: [visual-storyboards]
source_handles: [krometrail-repo, theatre-repo]
researched: 2026-07-25
updated: 2026-07-25
---

# Krometrail visual-temporal systems as inspiration for theatre

## Scope

Explored the krometrail workspace (local checkout, v1.6.2) as *inspiration,
not prescription* for adding agent-facing visual-temporal artifacts
(storyboards, motion history, difference maps) to theatre's clip system.
Sources: three read-only exploration passes over the krometrail repo plus a
direct survey of theatre's recording internals. Attestations:
`krometrail-repo`, `theatre-repo`.

## Findings

### temporal-vision is directly consumable and principle-aligned

The crate is pure, synchronous, and browser-agnostic — five dependencies, no
async, no I/O [krometrail-repo]{1}. It consumes borrowed RGBA8 slices with
caller-owned generic IDs [krometrail-repo]{2}, so stage-server can decode
clip JPEGs and hand over byte slices without copies. Its generators emit
plan-only outputs (selection, accumulation, difference data) as well as
rendered PNGs, and all pixel math is integer-only and deterministic across
worker counts [krometrail-repo]{5} [krometrail-repo]{7}.

This lands squarely inside theatre's thin-engine-boundary principle: all
analysis lives server-side; the Godot layer only produces pixels.

### Storyboard selection solves the token-economy problem

`select.rs` picks 3–12 informative frames by a fixed-priority policy with
stated reasons, rather than uniform sampling [krometrail-repo]{4}. One
labeled montage image amortizes image-token cost across a whole clip window;
the JSON manifest (selection reasons, omitted anchors, gap honesty) stays
cheap text. Artifact kinds available: storyboard, difference map, region
filmstrip, motion history, before/during/after [krometrail-repo]{3}.

### The capture lessons are about honesty, not plumbing

Krometrail's hard constraints (CDP acks, base64, `everyNthFrame` ceiling)
do not exist in-process in Godot [krometrail-repo]{8}. The portable
mechanisms:

- bounded queue + `try_send` with a **typed gap ledger** persisted and
  reported ("frames X–Y dropped, reason") instead of silent loss
  [krometrail-repo]{8};
- derived artifacts generated **offline** from retained frames, cached by
  content key, never computed in the live path [krometrail-repo]{9};
- per-frame SQLite writes in the live path are a durability tax krometrail
  pays because evidence retention is its product [krometrail-repo]{10} —
  theatre already avoids this with in-memory rings flushed on clip save
  [theatre-repo]{1} [theatre-repo]{3}.

### MCP surface patterns worth borrowing over time

Registry-as-data tool gating, one structured response envelope, detail
tiers with exact omitted counts plus continuation pointers, and a per-call
`inline_images` knob [krometrail-repo]{11}. Theatre already has budget
blocks and detail tiers [theatre-repo]{6}; the image-inclusion knob and
omitted-count discipline are the immediate borrowable pieces.

### Theatre is closer than expected

The dashcam already captures downscaled JPEGs into a ring (default 2 s,
960 px, q0.75, 64 MB cap), flushes them into clip SQLite, and serves them
as MCP image content [theatre-repo]{2} [theatre-repo]{4} [theatre-repo]{5}.
The gap is capture *density* (2 s is too sparse for motion analysis) and the
absence of any diff/motion/storyboard computation [theatre-repo]{7}.

## Verification

All claims traced to two local-repository attestations with observed commit
hashes. No external sources were needed; the question is about two specific
codebases under the author's control.

## Disconfirming evidence

Searched for reasons this integration is a bad idea:

- **GPU readback cost is real.** `get_texture().get_image()` is a blocking
  full-res readback on the main thread [theatre-repo]{2}; dense capture
  could hitch gameplay, violating observational-by-default. Mitigation
  chosen in the work item: low resolution, off-thread encode, byte cap, gap
  ledger, and a perf-probe gate with trigger-armed bursts as fallback.
- **temporal-vision's sole input format is RGBA8 sRGB straight**
  [krometrail-repo]{2}. Godot HDR viewports may need conversion at the
  boundary; unverified for all rendering paths.
- **krometrail's renderer exists because it assumes no host renderer**; for
  agent-facing PNG artifacts it is directly usable, but for any future
  human-facing dock UI the plan-only outputs should feed native Godot UI
  instead [krometrail-repo]{6}.

## Consequence for design

Settled in `visual-storyboards`: v1 generates storyboard, motion-history,
and difference-map artifacts on demand from saved clips, server-side, via a
temporal-vision path dependency, with content-addressed caching and
manifest-plus-bounded-image responses. Region filmstrip, anomaly detection,
dock UI, live-ring queries, and an evaluation corpus are parked in the
backlog.
