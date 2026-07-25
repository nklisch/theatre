---
source_handle: theatre-repo
fetched: 2026-07-25
source_title: theatre repository (this project)
source_url: https://github.com/nklisch/theatre/tree/d8351e11ffcd36da9d78c254ea283209ff0b0458
---

This repository at commit `d8351e11ffcd36da9d78c254ea283209ff0b0458`
("normalize Workbench to schema with research infrastructure"), surveyed
2026-07-25 by direct reads of the recording and clip-analysis code.

## Attested details

1. `crates/stage-godot/src/recorder.rs` implements a dashcam with an
   in-memory ring buffer of spatial frames: every physics frame (interval
   configurable), a `Vec<FrameEntityData>` snapshot is MessagePack-encoded
   and held under a byte cap (default 1024 MB) with adaptive eviction.
2. The same recorder already captures viewport screenshots:
   `do_screenshot_capture()` calls `viewport.get_texture().get_image()`,
   downscales to a configurable max dimension (default 960 px), and encodes
   JPEG at configurable quality (default 0.75). Capture interval defaults
   to every 2.0 s; a separate ring holds them under a 64 MB byte cap.
3. On dashcam triggers (markers: human, agent, code, or system tier), a clip
   is saved: pre-window ring contents plus post-capture window, written to a
   per-clip SQLite database with `recording`, `frames`, `events`, `markers`,
   and `screenshots` tables (`recorder.rs` schema at line ~1288).
4. `crates/stage-server/src/clip_analysis.rs` reads clip databases and can
   return the screenshot nearest a frame or timestamp
   (`read_screenshot_near_frame`, `read_screenshot_near_time`,
   `list_screenshots`).
5. `crates/stage-server/src/mcp/clips.rs` exposes clip actions
   (`SnapshotAt`, `Trajectory`, `QueryRange`, `DiffFrames`, `FindEvent`,
   `Markers`, `ScreenshotAt`, `Screenshots`, …); `ScreenshotAt` returns the
   JPEG as MCP image content (base64, `image/jpeg`).
6. Stage MCP responses carry a `budget` block (`used`, `limit`, `hard_cap`)
   with detail-tier defaults (summary 500 / standard 1500 / full 3000,
   hard cap 5000) from `crates/stage-core/src/budget.rs`.
7. No difference-map, motion-history, storyboard-selection, or montage
   rendering code exists anywhere in the workspace (grep-confirmed).
