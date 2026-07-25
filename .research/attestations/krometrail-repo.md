---
source_handle: krometrail-repo
fetched: 2026-07-25
source_title: krometrail repository (local checkout)
source_url: https://github.com/nklisch/krometrail/tree/3414f57b83787aaa21fb3b6cc112ca03f4df60e8
---

Local checkout of the krometrail repository at commit
`3414f57b83787aaa21fb3b6cc112ca03f4df60e8` ("Release v1.6.2"), explored
2026-07-25 via direct file reads and three read-only code-search passes
covering the `temporal-vision` crate, the CDP capture pipeline, and the
remaining workspace systems.

## Attested details

1. `crates/temporal-vision` is a pure, synchronous pixel-analysis library:
   no async runtime, no I/O, no browser dependencies. Its `Cargo.toml`
   declares only `serde`, `schemars`, `thiserror`, `png`, and `sha2`.
2. temporal-vision input is `Frame<Id, Pixels: AsRef<[u8]>>` — RGBA8 sRGB
   straight-alpha raw bytes, accepted as borrowed slices (`frame.rs`). A
   `FrameSequence` validates non-decreasing timestamps, common dimensions,
   unique ids, and carries `Marker` point events and `DeclaredGap` known-
   missing intervals (`sequence.rs`).
3. Artifact kinds defined in `provenance.rs`: `BeforeDuringAfter`,
   `Storyboard`, `DifferenceMap`, `RegionFilmstrip`, `MotionHistory`.
4. `select.rs` storyboard selection admits 3..=12 frames via a fixed-priority
   policy (pre-anchor, peak-baseline-change, final frame, first change,
   marker/gap boundaries, then fill by information gain/coverage), recording
   per-frame `SelectionReason`s.
5. `measure.rs` performs integer-only weighted-linear-RGB pixel change
   classification (weights 13933/46871/4732 summing to 65536) with a
   configurable noise floor; `SharedAdjacentAnalysis` caches adjacent-pair
   results for reuse across generators.
6. `motion_history.rs` builds an integer exponential-decay accumulation
   split into continuity segments at declared gaps; plan-only entry point
   `build_motion_history_plan()` returns data without rendering.
7. `parallel.rs` implements custom `std::thread::scope` parallelism (no
   Rayon), hard cap 16 workers, with deterministic in-order merge so output
   bytes are identical regardless of worker count.
8. The capture pipeline (`crates/krometrail-cdp/src/capture/pipeline.rs`)
   uses `Page.startScreencast` with ack-token backpressure (Chrome withholds
   the next frame until ack), a bounded mpsc queue (default capacity 4, hard
   cap 16) with `try_send`, and a typed `CaptureGapReason` ledger
   (`IngestionQueueSaturated`, `PersistenceRejected`, `TargetHidden`, etc.)
   persisted to a `capture_gaps` SQLite table.
9. `krometrail-ffmpeg` does not record live video; it transcodes already-
   retained frames into MP4 on demand, gated by a single-encode semaphore,
   with output cached by content key.
10. `krometrail-store` writes per-frame SQLite `IMMEDIATE` transactions on
    the live append path behind a global `mutations` mutex; binary payloads
    live in append-only segment files rotated at 120 s or 128 MiB with
    sealed-footer CRC durability.
11. `krometrail-mcp` gates tools from a declarative capability registry and
    operation registry macros; every tool returns one structured envelope
    (`tool`, `status`, `result`, `warnings`, `images`, `resources`,
    `error`, `diagnostics`) with concise/expanded/full detail tiers carrying
    per-tier count and byte caps, exact omitted counts, and continuation
    pointers; image inclusion is per-call opt-in/opt-out via an
    `inline_images` knob with per-tool defaults.
12. `crates/temporal-evaluation` is a feature-gated, non-runtime benchmark
    corpus for evaluating LLM temporal-defect perception (case families,
    prompt sets, scoring rubrics, SHA256-pinned fixtures).
