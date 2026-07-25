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


## Design (k3, 2026-07-25)

Full design produced by the k3 design agent; the pinned interface between
surfaces is §3 (clip DB schema) + §6.2 (config JSON keys). Summary of
corrections it discovered: temporal-vision is v1.6.2 (`version = "1"`);
`screenshot_quality` was previously dead config; `stage.toml [dashcam]` was
loaded but never pushed to the addon (this design wires the push); headless
Godot produces no screenshots, so E2E asserts the honest-degradation contract.

1. **Ring capture (stage-godot)**: frame-based cadence replaces time-based
   (`screenshot_interval_frames: 4` = 15fps at 60 physics fps, quality 0.65
   now real, max dim 480, byte cap 32MB). Off-main-thread JPEG encode via a
   lazily-spawned worker thread with bounded sync_channel (cap 8): main
   thread does readback+resize+`convert(FORMAT_RGBA8)` (handles HDR),
   `try_send` raw RGBA8; `jpeg-encoder = "0.6"` encodes off-thread;
   `drain_encoded_shots` per physics tick. No `Gd<T>` crosses threads.
   Fallback: `dense_burst_*` config arms dense capture post-trigger only.
2. **Gap ledger**: coalescing `CaptureGap{start,end,reason,dropped}` capped
   at 256; surfaces in clip DB `screenshot_gaps` table, `dashcam_status`
   counters, and artifact manifests as temporal-vision `DeclaredGap`s.
   Byte-cap ring eviction is NOT ledgered (ring semantics, not loss).
3. **Clip DB schema (pinned)**: new `screenshot_gaps` (+index) and
   `artifacts` (cache_key PK, kind, params_json, manifest_json, dims, png
   BLOB, created_at_ms) tables in the writer schema; reader feature-detects
   like `screenshots_table_exists`; old clips get cadence-inferred gaps.
4. **Artifact pipeline (stage-server)**: new `clip_artifacts/` module
   (frames/params/cache/manifest). Deps: `temporal-vision` (path+version),
   `jpeg-decoder = "0.3"`, `sha2`. JPEG→RGB→RGBA8, modal-dimension epoch
   filter (honest `dimension_mismatch_dropped`), 4096-frame cap with
   `subsampled` flag, corrupt JPEG = hard error. PNG via temporal-vision's
   own renderer, RenderLimits 2048×2048/4MiB encoded. Cache key =
   sha256(kind | canonical params JSON | clip fingerprint | tv version);
   cached in the clip DB `artifacts` table via short-lived RW connection,
   best-effort with `cache: "unavailable"` degradation.
5. **MCP surface**: extend existing `clips` tool — one new action
   `visual_artifact` (params: `artifact` kind, `at_frame`/`at_time_ms`
   anchor defaulting to first marker else midpoint, `reference_frame`,
   `tile_limit` 3–12, `inline_image` default true), plus `config` action
   forwarding to the existing `dashcam_config` TCP method. Response =
   text manifest + optional PNG image block; budget over manifest text.
6. **Config**: `stage.toml [dashcam]` fields added to SessionConfig and
   actually pushed after handshake (fixes the dead-config gap);
   `dashcam_status` gains `capture_probe` (readback EMA/max, encode depth,
   physics-pacing EMA + rolling-window p95 proxy) and `screenshot_gaps`.
   Artifact defaults are constants in v1.
7. **Tests**: unit (gap ledger, schema counts 7 tables/6 indexes, cache-key
   stability, determinism = identical PNG bytes across runs, old-clip
   compat, manifest contract), integration via synthetic fixture clip DB
   (jpeg-encoder as dev-dep; all three kinds + degradation paths), E2E
   journey `journey_visual_artifact_contract` with dual-mode rendering/
   headless assertions, perf-probe runbook (baseline vs dense, pass =
   physics-pacing p95 delta ≤1.0ms, no drop growth).

## Open implementation questions

Resolved by the design above: ring config pending probe outcome; RGBA8
conversion in-engine at capture; MCP surface = new `clips` actions; cache
lives in per-clip SQLite `artifacts` table.

## Execution approach (2026-07-25)

Topology set by user: k3 designs, luna implements, glm-5.2 + k3 review.
Orchestrator (host) integrates, verifies, adjudicates, closes.

- **Design (k3)** — implementation-ready design for both surfaces
  - Produces: ring-capture redesign, artifact pipeline design, clip-DB
    schema changes (pinned interface), MCP surface, test plan, perf-probe plan
- **Implementation (luna)** — both write surfaces, split by crate
  - stage-godot: dense screenshot ring, off-thread encode, gap ledger, probe
  - stage-server: temporal-vision integration, artifact generation, cache,
    MCP actions, tests (unit + integration + E2E journey)
- **Verification (orchestrator)** — build, clippy, fmt, full test workspace
  with GODOT_BIN=~/godot/Godot_v4.7.1-stable_linux.x86_64; perf probe run
- **Review (glm-5.2, k3)** — independent cross-model review of the diff
- **Close** — adjudicate findings, reconcile foundations, completed stub

Preflight: Godot 4.7.1 headless available; temporal-vision present at
../krometrail/crates/temporal-vision; cargo 1.95; clean worktree.

## Acceptance evidence

- Perf probe numbers (frame-time impact of dense capture) recorded in this
  item before ring defaults change.
- E2E journey: record clip with marker → request storyboard → assert
  montage image + manifest with selection reasons and gap honesty.
- Unit tests for artifact generation determinism (same clip + params →
  same bytes).
