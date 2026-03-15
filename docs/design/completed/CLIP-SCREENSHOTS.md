# Design: Clip Screenshots — Periodic Viewport Capture

## Overview

Add periodic viewport screenshot capture to the dashcam clip system. Screenshots
are captured at a configurable interval (independent of spatial frame capture),
stored as JPEG BLOBs in the clip's SQLite database, and delivered to agents as
MCP image content blocks. This gives agents visual context alongside the existing
spatial data when analyzing clips.

**Depends on:** M11 (Dashcam), M8 (Clip Analysis)

**Exit Criteria:**
- Screenshots are captured at a configurable interval while dashcam is active.
- Saved clips contain a `screenshots` table with JPEG BLOBs keyed by frame.
- `clips(action: "screenshot_at")` returns the nearest screenshot as an MCP
  image content block alongside frame metadata.
- `clips(action: "screenshots")` lists available screenshots in a clip with
  frame/timestamp metadata (no image data).
- Screenshot resolution is downscaled to a configurable max dimension for
  token efficiency.
- Existing clip actions (`snapshot_at`, `trajectory`, etc.) are unaffected.

---

## Architecture Decision: JPEG at Reduced Resolution

**Decision:** Capture viewport as JPEG at quality 75, downscaled to fit within
`screenshot_max_dimension` (default: 960px on the longest axis).

**Rationale:**
- For spatial debugging, agents need to see layout and game state, not
  pixel-perfect detail. JPEG at 75% quality preserves all meaningful visual
  information.
- A 960×540 JPEG is typically 15–30KB. At a 2-second interval over a 60-second
  clip, that's ~30 screenshots = ~450–900KB — manageable within SQLite.
- PNG would be 10× larger (~300KB each), bloating clips by 5–10MB. WebP has
  better compression but requires Godot 4.4+ and complicates the pipeline.
- Godot's `Image.save_jpg_to_buffer()` returns a `PackedByteArray` in one
  call — no temporary files needed.

**Consequence:** Screenshot quality is fixed at a level optimized for AI
consumption. Developers wanting pixel-perfect captures should use Godot's
built-in screenshot tools.

---

## Architecture Decision: Independent Capture Interval

**Decision:** Screenshots use a time-based interval (`screenshot_interval_sec`,
default: 2.0s) independent of the spatial `capture_interval`.

**Rationale:**
- Spatial frames are captured every physics tick (often 60fps). Screenshots
  at that rate would be ~900KB/sec — unsustainable.
- A 2-second interval provides enough visual context for debugging (30
  screenshots per minute) while keeping clip sizes reasonable.
- Time-based (not frame-based) ensures consistent capture regardless of
  `capture_interval` or physics tick rate.

**Consequence:** The recorder tracks `last_screenshot_ms` and compares against
`current_time_ms()` each physics tick. When the interval elapses and the
viewport is available, it captures a screenshot.

---

## Architecture Decision: SQLite Storage in `screenshots` Table

**Decision:** Add a `screenshots` table to the existing clip SQLite schema.
Screenshots are stored as BLOB rows keyed by `frame` (the physics frame at
capture time).

**Rationale:**
- Self-contained clips: one file per clip, easy to move/delete/archive.
- The frame key enables efficient "find nearest screenshot to frame N" queries
  using `ORDER BY ABS(frame - ?1) LIMIT 1`.
- WAL mode (already enabled) allows concurrent reads from the server while
  the addon writes.

**Schema addition:**
```sql
CREATE TABLE IF NOT EXISTS screenshots (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    image_data BLOB,
    width INTEGER,
    height INTEGER
);
CREATE INDEX IF NOT EXISTS idx_screenshots_timestamp ON screenshots(timestamp_ms);
```

---

## Architecture Decision: MCP Image Content Delivery

**Decision:** The `screenshot_at` action returns a `CallToolResult` with two
content blocks: a text block (JSON metadata) and an image block (base64 JPEG).

**Rationale:**
- MCP's `CallToolResult.content` supports `Vec<Content>` with mixed text and
  image blocks. This is the standard MCP way to deliver images to agents.
- The text block provides frame number, timestamp, resolution, and clip context
  so the agent can correlate the screenshot with spatial data.
- Returning the image inline avoids requiring filesystem access from the agent.

**Consequence:** The `screenshot_at` handler must return `Result<CallToolResult,
McpError>` instead of `Result<String, McpError>`. The rmcp `#[tool]` macro
supports this via `IntoCallToolResult`.

However, because the `clips` tool is a single MCP tool dispatching on `action`,
the existing handler already returns `Result<String, McpError>`. To support
mixed content, the `screenshot_at` action will be handled specially: the
`handle_clips` function will return `Result<CallToolResult, McpError>` and
wrap non-image actions in `CallToolResult::success(vec![Content::text(...)])`.

---

## Architecture Decision: Screenshot Ring Buffer

**Decision:** Screenshots are held in a separate `VecDeque<CapturedScreenshot>`
ring buffer, evicted by a configurable byte cap (`screenshot_byte_cap_mb`,
default: 64MB). When a clip flushes, screenshots are written to the
`screenshots` table alongside frame data.

**Rationale:**
- Screenshots are much larger than spatial frames (~20KB vs ~1KB). A separate
  byte cap prevents screenshots from evicting spatial frames or vice versa.
- The ring buffer holds screenshots for the pre-window duration. At 2-second
  intervals and 60s pre-window, that's ~30 screenshots ≈ 600KB — well within
  the default 64MB cap.
- During PostCapture, new screenshots are appended to a `post_screenshots`
  buffer (same pattern as spatial `post_buffer`).

---

## Implementation Units

### Unit 1: Screenshot Capture in GDExtension

**File**: `crates/stage-godot/src/recorder.rs`

```rust
/// A captured viewport screenshot.
#[derive(Clone)]
struct CapturedScreenshot {
    frame: u64,
    timestamp_ms: u64,
    jpeg_data: Vec<u8>,
    width: u32,
    height: u32,
}

// New fields in DashcamConfig:
pub struct DashcamConfig {
    // ... existing fields ...
    pub screenshot_enabled: bool,        // default: true
    pub screenshot_interval_sec: f64,    // default: 2.0
    pub screenshot_quality: f32,         // default: 0.75 (JPEG quality 0.0–1.0)
    pub screenshot_max_dimension: u32,   // default: 960
    pub screenshot_byte_cap_mb: u32,     // default: 64
}

// New fields in StageRecorder:
pub struct StageRecorder {
    // ... existing fields ...
    screenshot_ring: VecDeque<CapturedScreenshot>,
    screenshot_ring_bytes: usize,
    last_screenshot_ms: u64,
}

// New fields in DashcamState::PostCapture:
enum DashcamState {
    // ...
    PostCapture {
        // ... existing fields ...
        post_screenshots: Vec<CapturedScreenshot>,
    },
}
```

**New method — `do_screenshot_capture`:**
```rust
fn do_screenshot_capture(&mut self) -> Option<CapturedScreenshot> {
    // 1. Get viewport via self.base().get_viewport()
    // 2. Get texture: viewport.get_texture()
    // 3. Get image: texture.get_image()
    // 4. Downscale if needed: image.resize(max_dim, max_dim, INTERPOLATE_BILINEAR)
    //    preserving aspect ratio
    // 5. Encode: image.save_jpg_to_buffer(quality)
    // 6. Return CapturedScreenshot with frame from Engine::physics_frames()
}
```

**Integration into `physics_process`:**
```rust
fn physics_process(&mut self, _delta: f64) {
    // ... existing dashcam logic ...

    // Screenshot capture (independent of spatial capture interval)
    if self.dashcam_config.screenshot_enabled
        && !matches!(self.dashcam_state, DashcamState::Disabled)
    {
        let now_ms = current_time_ms();
        let interval_ms = (self.dashcam_config.screenshot_interval_sec * 1000.0) as u64;
        if now_ms >= self.last_screenshot_ms + interval_ms {
            if let Some(screenshot) = self.do_screenshot_capture() {
                self.screenshot_ingest(screenshot);
                self.last_screenshot_ms = now_ms;
            }
        }
    }
}
```

**Implementation Notes:**
- `get_viewport()` returns the node's viewport. During runtime this is the
  main game viewport. In editor context it may be an editor viewport — guard
  against this by checking `Engine::is_editor_hint()`.
- `Image.resize()` expects width and height. Compute the downscaled dimensions
  preserving aspect ratio: if width > height, new_width = max_dim, new_height =
  height * max_dim / width (and vice versa). Only resize if the larger dimension
  exceeds `screenshot_max_dimension`.
- Godot's `save_jpg_to_buffer(quality)` takes quality as `f32` in `[0.0, 1.0]`.
  The method is on `Image`, not on `Texture2D`.
- `get_texture().get_image()` performs a GPU→CPU readback. This is not free but
  at 2-second intervals the cost is negligible (~0.5ms per capture).

**Acceptance Criteria:**
- [ ] Screenshots are captured at the configured interval while dashcam is Buffering or PostCapture.
- [ ] Screenshots are not captured when dashcam is Disabled.
- [ ] Screenshots are downscaled to fit within `screenshot_max_dimension`.
- [ ] Screenshot ring buffer enforces `screenshot_byte_cap_mb`.
- [ ] `screenshot_enabled: false` disables screenshot capture entirely.

---

### Unit 2: Screenshot Storage in SQLite

**File**: `crates/stage-godot/src/recorder.rs`

Extend `flush_dashcam_clip_internal` to write screenshots alongside frames.

```rust
// In SCHEMA_SQL, add:
const SCHEMA_SQL: &str = "
    -- ... existing tables ...

    CREATE TABLE IF NOT EXISTS screenshots (
        frame INTEGER PRIMARY KEY,
        timestamp_ms INTEGER,
        image_data BLOB,
        width INTEGER,
        height INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_screenshots_timestamp ON screenshots(timestamp_ms);
";
```

**Changes to `flush_dashcam_clip_internal`:**
```rust
fn flush_dashcam_clip_internal(&mut self) -> Option<String> {
    // ... existing code to extract state, create DB, write frames/markers ...

    // Collect screenshots from ring + post buffers
    let all_screenshots: Vec<&CapturedScreenshot> = /* same pattern as all_frames */;

    // Write screenshots in the same transaction
    if let Ok(mut stmt) = tx.prepare_cached(
        "INSERT OR REPLACE INTO screenshots \
         (frame, timestamp_ms, image_data, width, height) \
         VALUES (?1, ?2, ?3, ?4, ?5)"
    ) {
        for s in &all_screenshots {
            let _ = stmt.execute(rusqlite::params![
                s.frame, s.timestamp_ms, &s.jpeg_data, s.width, s.height
            ]);
        }
    }

    // ... existing commit, signal, return ...
}
```

**Implementation Notes:**
- Only write screenshots whose `frame` falls within the clip's frame range
  (first_frame..=last_frame). The ring buffer may contain screenshots from
  before the pre-window.
- The `capture_config` JSON in the `recording` table should include screenshot
  config: `"screenshot_interval_sec": 2.0, "screenshot_max_dimension": 960`.

**Acceptance Criteria:**
- [ ] Flushed clips contain a `screenshots` table with JPEG BLOBs.
- [ ] Screenshot frames fall within the clip's frame range.
- [ ] `capture_config` JSON includes screenshot configuration.
- [ ] Clips without screenshot data (e.g., `screenshot_enabled: false`) have
      an empty `screenshots` table.

---

### Unit 3: Screenshot Reading in Clip Analysis

**File**: `crates/stage-server/src/clip_analysis.rs`

```rust
/// A screenshot read from a clip's SQLite database.
pub struct ClipScreenshot {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub jpeg_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Read the screenshot nearest to the given frame.
pub fn read_screenshot_near_frame(
    db: &Connection,
    frame: u64,
) -> Result<Option<ClipScreenshot>, McpError> {
    // SELECT frame, timestamp_ms, image_data, width, height
    //   FROM screenshots
    //   ORDER BY ABS(frame - ?1) LIMIT 1
}

/// Read the screenshot nearest to the given timestamp.
pub fn read_screenshot_near_time(
    db: &Connection,
    time_ms: u64,
) -> Result<Option<ClipScreenshot>, McpError> {
    // SELECT frame, timestamp_ms, image_data, width, height
    //   FROM screenshots
    //   ORDER BY ABS(timestamp_ms - ?1) LIMIT 1
}

/// List all screenshot metadata in a clip (no image data).
pub fn list_screenshots(
    db: &Connection,
) -> Result<Vec<ScreenshotMeta>, McpError> {
    // SELECT frame, timestamp_ms, width, height, LENGTH(image_data) as size_bytes
    //   FROM screenshots ORDER BY frame
}

pub struct ScreenshotMeta {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}
```

**Implementation Notes:**
- `read_screenshot_near_frame` uses `ORDER BY ABS(frame - ?1)` which is
  efficient with the PRIMARY KEY index on `frame`.
- `list_screenshots` returns metadata only (no BLOBs) to keep the response
  small when browsing.
- Handle the case where the `screenshots` table doesn't exist (older clips
  created before this feature): return empty results, not errors. Use
  `SELECT name FROM sqlite_master WHERE type='table' AND name='screenshots'`
  to check.

**Acceptance Criteria:**
- [ ] `read_screenshot_near_frame` returns the closest screenshot by frame number.
- [ ] `read_screenshot_near_time` returns the closest screenshot by timestamp.
- [ ] `list_screenshots` returns metadata for all screenshots without image data.
- [ ] Gracefully handles clips that predate the screenshots feature (no table).

---

### Unit 4: MCP Tool Actions

**File**: `crates/stage-server/src/mcp/clips.rs`

Add two new actions to the `clips` tool: `screenshot_at` and `screenshots`.

**Parameter additions:**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipsParams {
    /// Action to perform.
    /// ... existing actions ...
    /// "screenshot_at" — get the viewport screenshot nearest to a frame or timestamp.
    /// "screenshots" — list screenshot metadata in a clip.
    #[schemars(
        description = "Action: add_marker, save, status, list, delete, markers, \
                       snapshot_at, trajectory, query_range, diff_frames, find_event, \
                       screenshot_at, screenshots"
    )]
    pub action: String,

    // ... existing fields ...
}
```

**Return type change:**

The `handle_clips` function must change from returning `Result<String, McpError>`
to `Result<CallToolResult, McpError>` to support mixed text+image content.

```rust
use rmcp::model::{CallToolResult, Content};
use base64::Engine as Base64Engine;

pub async fn handle_clips(
    params: ClipsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<CallToolResult, McpError> {
    match params.action.as_str() {
        // ... existing actions wrapped in CallToolResult::success(vec![Content::text(...)]) ...

        "screenshot_at" => {
            let storage_path = resolve_clip_storage_path(state).await?;
            let clip_id = resolve_clip_id(&params, &storage_path)?;
            let db = open_clip_db(&storage_path, &clip_id)?;

            let screenshot = if let Some(frame) = params.at_frame {
                clip_analysis::read_screenshot_near_frame(&db, frame)?
            } else if let Some(time_ms) = params.at_time_ms {
                clip_analysis::read_screenshot_near_time(&db, time_ms)?
            } else {
                return Err(McpError::invalid_params(
                    "screenshot_at requires at_frame or at_time_ms", None
                ));
            };

            let Some(screenshot) = screenshot else {
                return Ok(CallToolResult::success(vec![
                    Content::text(json!({
                        "error": "no_screenshots",
                        "clip_id": clip_id,
                        "message": "This clip contains no screenshots"
                    }).to_string())
                ]));
            };

            let metadata = json!({
                "clip_id": clip_id,
                "frame": screenshot.frame,
                "timestamp_ms": screenshot.timestamp_ms,
                "width": screenshot.width,
                "height": screenshot.height,
                "size_bytes": screenshot.jpeg_data.len(),
            });

            let b64 = base64::engine::general_purpose::STANDARD
                .encode(&screenshot.jpeg_data);

            Ok(CallToolResult::success(vec![
                Content::text(metadata.to_string()),
                Content::image(b64, "image/jpeg"),
            ]))
        }

        "screenshots" => {
            let storage_path = resolve_clip_storage_path(state).await?;
            let clip_id = resolve_clip_id(&params, &storage_path)?;
            let db = open_clip_db(&storage_path, &clip_id)?;
            let list = clip_analysis::list_screenshots(&db)?;

            let result = json!({
                "clip_id": clip_id,
                "screenshots": list.iter().map(|s| json!({
                    "frame": s.frame,
                    "timestamp_ms": s.timestamp_ms,
                    "width": s.width,
                    "height": s.height,
                    "size_bytes": s.size_bytes,
                })).collect::<Vec<_>>(),
                "total": list.len(),
            });

            Ok(CallToolResult::success(vec![
                Content::text(result.to_string())
            ]))
        }

        // ... other actions ...
    }
}
```

**Implementation Notes:**
- The `base64` crate must be added to `stage-server/Cargo.toml`.
- All existing actions must be wrapped: their current `Result<String, McpError>`
  return values become `CallToolResult::success(vec![Content::text(s)])`.
- The `#[tool]` macro handler in `mcp/mod.rs` that calls `handle_clips` must
  also change its return type to `Result<CallToolResult, McpError>`.
- `log_activity` should record screenshot_at as a "query" action.

**Acceptance Criteria:**
- [ ] `clips(action: "screenshot_at", at_frame: N)` returns nearest screenshot as
      MCP image content with JSON metadata.
- [ ] `clips(action: "screenshot_at", at_time_ms: N)` finds by timestamp.
- [ ] `clips(action: "screenshots")` lists screenshot metadata without image data.
- [ ] Missing `at_frame`/`at_time_ms` for `screenshot_at` returns invalid_params error.
- [ ] Clips without screenshots return a clear "no_screenshots" message, not an error.
- [ ] All existing clip actions continue to work unchanged.

---

### Unit 5: Dashcam Config Extension

**File**: `crates/stage-godot/src/recorder.rs`

Extend `apply_dashcam_config` and `get_dashcam_config_json` to include
screenshot settings.

```rust
// In apply_dashcam_config, parse new keys:
// "screenshot_enabled": bool
// "screenshot_interval_sec": f64
// "screenshot_quality": f32
// "screenshot_max_dimension": u32
// "screenshot_byte_cap_mb": u32

// In get_dashcam_config_json, serialize new keys alongside existing ones.
```

**File**: `crates/stage-server/src/mcp/clips.rs`

The `status` action response already includes `config` — no changes needed
since the config JSON is passed through from the addon.

**Acceptance Criteria:**
- [ ] `dashcam_config` TCP method accepts screenshot config keys.
- [ ] `dashcam_status` response includes screenshot config in the config object.
- [ ] Invalid screenshot config values are rejected (quality out of range, etc.).

---

### Unit 6: Screenshot Status in Dashcam Status

**File**: `crates/stage-godot/src/recorder.rs`

Add screenshot buffer stats to the dashcam status query.

```rust
// New exported methods:
#[func]
pub fn get_screenshot_buffer_count(&self) -> u32 {
    self.screenshot_ring.len() as u32
}

#[func]
pub fn get_screenshot_buffer_kb(&self) -> u32 {
    (self.screenshot_ring_bytes / 1024) as u32
}
```

**File**: `crates/stage-godot/src/recording_handler.rs`

Extend `handle_dashcam_status` to include screenshot stats:

```rust
fn handle_dashcam_status(recorder: &mut Gd<StageRecorder>) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    // ... existing fields ...
    let screenshot_count = rec.get_screenshot_buffer_count();
    let screenshot_kb = rec.get_screenshot_buffer_kb();
    drop(rec);

    Ok(json!({
        // ... existing fields ...
        "screenshot_buffer_count": screenshot_count,
        "screenshot_buffer_kb": screenshot_kb,
    }))
}
```

**Acceptance Criteria:**
- [ ] `clips(action: "status")` includes `screenshot_buffer_count` and
      `screenshot_buffer_kb`.

---

## Implementation Order

1. **Unit 1: Screenshot Capture** — Core capture logic in GDExtension. No
   external dependencies. Must be done first as all other units depend on it.
2. **Unit 2: SQLite Storage** — Extend schema and flush logic. Depends on
   Unit 1 for `CapturedScreenshot` type and ring buffer.
3. **Unit 5: Config Extension** — Extend config parsing. Can be done alongside
   Unit 2 as it only touches config code.
4. **Unit 6: Status Extension** — Add screenshot stats. Can be done alongside
   Unit 2.
5. **Unit 3: Clip Analysis Reading** — Server-side SQLite reading. Depends on
   Unit 2 schema being defined.
6. **Unit 4: MCP Tool Actions** — MCP integration. Depends on Unit 3 for
   reading functions and requires the `clips` handler return type change.

## Dependencies

- `base64` crate added to `stage-server/Cargo.toml` (for JPEG→base64
  encoding in MCP responses).
- No new dependencies needed in `stage-godot` — Godot's `Image` class
  provides JPEG encoding natively.
- No changes to `stage-protocol` — screenshots don't flow over TCP.
  They're captured in GDExtension, stored in SQLite, and read directly by
  the server.

## Testing

### Unit Tests: `crates/stage-godot/src/recorder.rs`

Screenshot capture involves Godot's viewport API which is unavailable in
headless unit tests. Test the following without Godot:

- **Ring buffer eviction:** Create `CapturedScreenshot` values, add to ring,
  verify byte cap eviction.
- **Interval logic:** Verify `last_screenshot_ms` tracking and interval
  comparison logic.
- **Config parsing:** Verify `apply_dashcam_config` handles screenshot keys.

### Unit Tests: `crates/stage-server/src/clip_analysis.rs`

- **`read_screenshot_near_frame`:** Create in-memory SQLite DB with screenshots
  table, insert test rows, verify nearest-frame lookup.
- **`read_screenshot_near_time`:** Same setup, verify nearest-timestamp lookup.
- **`list_screenshots`:** Verify metadata-only listing.
- **Missing table:** Verify graceful handling when `screenshots` table doesn't
  exist (pre-feature clips).

### Test Harness Changes

**File**: `crates/stage-server/tests/support/mod.rs`

The `dispatch_tool` / `dispatch_tool_raw` helpers currently assume all tools
return `Result<String, McpError>`. When the `clips` handler changes to return
`Result<CallToolResult, McpError>`, the dispatch helper must be updated.

Add a `dispatch_tool_result` variant that returns `CallToolResult`:

```rust
pub async fn dispatch_tool_result(
    server: &StageServer,
    name: &str,
    params: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    // For "clips": call server.clips() and get CallToolResult
    // For other tools: call existing dispatch_tool_raw, wrap in
    //   CallToolResult::success(vec![Content::text(s)])
}
```

Also update `E2EHarness` with a new method for image-returning tools:

```rust
impl E2EHarness {
    /// Call a tool that may return mixed content (text + images).
    /// Returns the full CallToolResult for inspection.
    pub async fn expect_result(
        &mut self,
        step: u32,
        tool: &str,
        params: Value,
    ) -> CallToolResult { ... }
}
```

### E2E Journey Tests

**File**: `crates/stage-server/tests/e2e_journeys.rs`

#### Journey: `journey_screenshot_capture_and_retrieval`

Tests the full screenshot lifecycle: dashcam captures screenshots, clip is
saved, screenshots are listed and retrieved as MCP image content.

```rust
/// Journey: Screenshots are captured during dashcam buffering and can be
/// retrieved from saved clips as MCP image content.
///
/// Steps:
///   1. clips(status) → verify dashcam buffering with screenshot config
///   2. wait_frames(180) → accumulate ~3 seconds of buffer (expect 1-2 screenshots
///      at 2-second interval)
///   3. clips(save, "screenshot_test") → flush clip, get clip_id
///   4. clips(screenshots, clip_id) → list screenshot metadata
///      Assert: total >= 1, each entry has frame, timestamp_ms, width, height, size_bytes
///   5. clips(screenshot_at, clip_id, at_frame=<first screenshot frame>) → retrieve image
///      Assert: CallToolResult has 2 content blocks (text metadata + image)
///      Assert: image content has mime_type="image/jpeg"
///      Assert: text content has clip_id, frame, width, height
///   6. clips(screenshot_at, clip_id, at_time_ms=<first screenshot timestamp>) → by time
///      Assert: returns same screenshot (nearest match)
///   7. clips(delete, clip_id) → cleanup
#[tokio::test]
async fn journey_screenshot_capture_and_retrieval() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: verify dashcam is buffering and screenshot config is present
    let status = h.expect(1, "clips", json!({ "action": "status" })).await;
    assert_eq!(status["state"], json!("buffering"));
    assert!(
        status["screenshot_buffer_count"].as_u64().is_some(),
        "screenshot_buffer_count should be present in status"
    );

    // Step 2: accumulate buffer with screenshots (~3 seconds)
    h.wait_frames(180).await;

    // Step 3: save clip
    let save = h
        .expect(
            3,
            "clips",
            json!({ "action": "save", "marker_label": "screenshot_test" }),
        )
        .await;
    let clip_id = save["clip_id"]
        .as_str()
        .expect("save should return clip_id")
        .to_string();

    // Step 4: list screenshots in the saved clip
    let screenshots = h
        .expect(
            4,
            "clips",
            json!({ "action": "screenshots", "clip_id": clip_id }),
        )
        .await;
    let total = screenshots["total"].as_u64().expect("total should be present");
    assert!(
        total >= 1,
        "Clip should contain at least 1 screenshot after ~3s buffer. Got: {screenshots}"
    );
    let screenshot_list = screenshots["screenshots"]
        .as_array()
        .expect("screenshots array should be present");
    let first = &screenshot_list[0];
    assert!(first["frame"].as_u64().is_some(), "screenshot should have frame");
    assert!(first["timestamp_ms"].as_u64().is_some(), "screenshot should have timestamp_ms");
    assert!(first["width"].as_u64().is_some(), "screenshot should have width");
    assert!(first["height"].as_u64().is_some(), "screenshot should have height");
    assert!(first["size_bytes"].as_u64().unwrap_or(0) > 0, "screenshot should have non-zero size");

    let first_frame = first["frame"].as_u64().unwrap();
    let first_time = first["timestamp_ms"].as_u64().unwrap();

    // Step 5: retrieve screenshot by frame — returns CallToolResult with image content
    let result = h
        .expect_result(
            5,
            "clips",
            json!({ "action": "screenshot_at", "clip_id": clip_id, "at_frame": first_frame }),
        )
        .await;
    assert_eq!(
        result.content.len(),
        2,
        "screenshot_at should return 2 content blocks (text + image)"
    );
    // First block: text metadata
    let text_content = result.content[0]
        .as_text()
        .expect("First content block should be text");
    let meta: serde_json::Value = serde_json::from_str(&text_content.text)
        .expect("Text block should be valid JSON");
    assert_eq!(meta["clip_id"], json!(clip_id));
    assert!(meta["frame"].as_u64().is_some());
    assert!(meta["width"].as_u64().is_some());
    assert!(meta["height"].as_u64().is_some());
    // Second block: image
    let image_content = result.content[1]
        .as_image()
        .expect("Second content block should be an image");
    assert_eq!(image_content.mime_type, "image/jpeg");
    assert!(!image_content.data.is_empty(), "Image data should be non-empty base64");
    // Verify it decodes to valid JPEG (starts with FFD8 header)
    let jpeg_bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_content.data)
        .expect("Image data should be valid base64");
    assert!(
        jpeg_bytes.len() >= 2 && jpeg_bytes[0] == 0xFF && jpeg_bytes[1] == 0xD8,
        "Decoded data should be valid JPEG (FFD8 header)"
    );

    // Step 6: retrieve by timestamp — should find the same (or nearest) screenshot
    let result_by_time = h
        .expect_result(
            6,
            "clips",
            json!({ "action": "screenshot_at", "clip_id": clip_id, "at_time_ms": first_time }),
        )
        .await;
    assert_eq!(
        result_by_time.content.len(),
        2,
        "screenshot_at by time should also return 2 content blocks"
    );
    let time_meta: serde_json::Value = serde_json::from_str(
        &result_by_time.content[0].as_text().unwrap().text,
    )
    .unwrap();
    // Should be the same or very near frame
    let returned_frame = time_meta["frame"].as_u64().unwrap();
    assert!(
        (returned_frame as i64 - first_frame as i64).unsigned_abs() < 120,
        "Nearest screenshot by time should be close to the one by frame"
    );

    // Step 7: cleanup
    h.expect(
        7,
        "clips",
        json!({ "action": "delete", "clip_id": clip_id }),
    )
    .await;
}
```

#### Journey: `journey_screenshot_absent_in_old_clips`

Tests backward compatibility with clips that predate the screenshots feature.

```rust
/// Journey: Clips without screenshots (older format) handle screenshot queries gracefully.
///
/// This test creates a clip, then queries screenshots — since the test project
/// may have older clips from before the feature. The key assertion is that
/// screenshot queries never error on clips without screenshot data.
///
/// Steps:
///   1. wait_frames(5) → minimal buffer (screenshot interval unlikely to fire)
///   2. clips(save) → get clip_id with minimal/no screenshots
///   3. clips(screenshots, clip_id) → should return total=0 or list, never error
///   4. clips(screenshot_at, clip_id, at_frame=0) → should return no_screenshots message
///   5. clips(delete, clip_id) → cleanup
#[tokio::test]
async fn journey_screenshot_absent_in_old_clips() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: minimal buffer — too short for screenshot interval to fire
    h.wait_frames(5).await;

    // Step 2: save clip immediately
    let save = h
        .expect(2, "clips", json!({ "action": "save", "marker_label": "no_screenshots" }))
        .await;
    let clip_id = save["clip_id"].as_str().unwrap().to_string();

    // Step 3: list screenshots — should be empty, not an error
    let screenshots = h
        .expect(3, "clips", json!({ "action": "screenshots", "clip_id": clip_id }))
        .await;
    // May be 0 or very few — the key is no error
    assert!(
        screenshots["total"].as_u64().is_some(),
        "screenshots action should succeed even with no screenshots"
    );

    // Step 4: screenshot_at on a clip with no screenshots — graceful response
    let result = h
        .expect_result(
            4,
            "clips",
            json!({ "action": "screenshot_at", "clip_id": clip_id, "at_frame": 0 }),
        )
        .await;
    // Should return a text-only result with "no_screenshots" message, not an MCP error
    assert!(
        result.content.len() == 1,
        "screenshot_at with no screenshots should return single text content"
    );
    let text = result.content[0].as_text().expect("Should be text content");
    let body: serde_json::Value = serde_json::from_str(&text.text).unwrap();
    assert_eq!(body["error"], json!("no_screenshots"));

    // Step 5: cleanup
    h.expect(5, "clips", json!({ "action": "delete", "clip_id": clip_id })).await;
}
```

#### Journey: `journey_screenshot_status_fields`

Tests that dashcam status includes screenshot buffer metadata.

```rust
/// Journey: Dashcam status reports screenshot buffer stats.
///
/// Steps:
///   1. clips(status) → verify screenshot_buffer_count and screenshot_buffer_kb present
///   2. wait_frames(180) → accumulate screenshots
///   3. clips(status) → screenshot_buffer_count should have increased
#[tokio::test]
async fn journey_screenshot_status_fields() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: initial status
    let status1 = h.expect(1, "clips", json!({ "action": "status" })).await;
    let initial_count = status1["screenshot_buffer_count"]
        .as_u64()
        .expect("screenshot_buffer_count should be present");

    // Step 2: accumulate ~3 seconds
    h.wait_frames(180).await;

    // Step 3: buffer count should have grown
    let status2 = h.expect(3, "clips", json!({ "action": "status" })).await;
    let later_count = status2["screenshot_buffer_count"].as_u64().unwrap();
    assert!(
        later_count > initial_count,
        "Screenshot buffer count should grow over time. Initial: {initial_count}, Later: {later_count}"
    );
    assert!(
        status2["screenshot_buffer_kb"].as_u64().unwrap_or(0) > 0,
        "Screenshot buffer should have non-zero KB"
    );
}
```

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Run all tests (unit + integration + E2E)
theatre-deploy ~/dev/stage/tests/godot-project
cargo test --workspace

# Run only the new E2E screenshot journeys
cargo test -p stage-server --test e2e_journeys journey_screenshot -- --nocapture

# Lint
cargo clippy --workspace
cargo fmt --check
```
