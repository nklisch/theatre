# Design: Milestone 7 — Recording Capture

## Overview

M7 delivers the recording system's capture side: a `SpectatorRecorder` GDExtension class that captures spatial timelines to SQLite, the `recording` MCP tool (capture actions only — analysis is M8), dock recording controls, keyboard shortcuts (F8/F9), and in-game recording indicator.

**Depends on:** M1 (TCP, snapshot flow), M6 (dock, activity log, overlay infrastructure)

**Exit Criteria:** Human presses F8, plays game for 10 seconds, presses F9 at bug moment, presses F8 to stop. Recording appears in dock library and via `recording(action: "list")`. Agent can also start/stop recordings via MCP. Markers from all three sources are stored. Partial recording survives a game crash (up to last flush).

---

## Architecture Decision: Where SQLite Lives

**Decision: GDExtension writes SQLite directly.**

The `SpectatorRecorder` (Rust, running in Godot's process) owns the SQLite connection and writes frame data. The MCP server queries recordings via TCP methods to the addon (M7) and directly opens SQLite files for analysis (M8).

**Rationale:**
- Human can record without agent connected (F8). Addon must be self-sufficient for capture.
- Frame data stays in-process — no TCP transfer overhead for 60fps capture.
- Periodic flush to SQLite provides crash safety without network dependency.
- Server reads the same files for M8 analysis queries.

**Dependencies added:**
- `rusqlite` (bundled) in spectator-godot — SQLite writes
- `rmp-serde` in spectator-godot — MessagePack frame serialization

---

## Current State Analysis

### What exists:
1. **SpectatorCollector** — collects snapshot data via `collect_snapshot()`, returns `SnapshotResponse` with `Vec<EntityData>`
2. **plugin.gd** — registers recording Project Settings (`storage_path`, `max_frames`, `capture_interval`) but nothing reads them
3. **runtime.gd** — handles F10 pause, overlay infrastructure (CanvasLayer, toast container). No F8/F9 handling.
4. **dock.gd/dock.tscn** — connection status, session info, activity feed. No recording section.
5. **TCP protocol** — `Message::Query`/`Response`/`Event` variants support arbitrary methods. No recording-specific methods.
6. **activity.rs** — summary generation for all tools. Activity color mapping already includes `"recording"` as cyan in dock.gd.

### What M7 must add:
- **SpectatorRecorder** GDExtension class (new file: `crates/spectator-godot/src/recorder.rs`)
- **Recording query handler** (new file: `crates/spectator-godot/src/recording_handler.rs`)
- **Recording MCP tool** (new file: `crates/spectator-server/src/mcp/recording.rs`)
- **Dock recording section** (modify `dock.tscn` and `dock.gd`)
- **F8/F9 shortcuts + recording indicator** (modify `runtime.gd`)
- **Recording activity summaries** (modify `activity.rs`)

---

## Implementation Units

### Unit 1: Workspace Dependencies

**Files:** `Cargo.toml` (workspace root), `crates/spectator-godot/Cargo.toml`

```toml
# Cargo.toml (workspace root) — add to [workspace.dependencies]
rusqlite = { version = "0.33", features = ["bundled"] }
rmp-serde = "1"
```

```toml
# crates/spectator-godot/Cargo.toml — add to [dependencies]
rusqlite.workspace = true
rmp-serde.workspace = true
```

**Implementation Notes:**
- `bundled` feature compiles SQLite from source — no system dependency, cross-compilation friendly.
- `rmp-serde` provides `rmp_serde::to_vec()` / `from_slice()` for MessagePack (de)serialization of frame data.

**Acceptance Criteria:**
- [ ] `cargo build -p spectator-godot` succeeds with new dependencies
- [ ] GDExtension binary loads in Godot without errors

---

### Unit 2: SpectatorRecorder GDExtension Class

**File:** `crates/spectator-godot/src/recorder.rs` (new)

```rust
use godot::prelude::*;
use godot::classes::Node;
use rusqlite::Connection;
use serde::{Serialize, Deserialize};
use spectator_protocol::query::{EntityData, SnapshotResponse};

use crate::collector::SpectatorCollector;

/// In-memory captured frame awaiting flush to SQLite.
struct CapturedFrame {
    frame: u64,
    timestamp_ms: u64,
    data: Vec<u8>, // MessagePack-encoded Vec<FrameEntityData>
}

/// In-memory captured event awaiting flush to SQLite.
struct CapturedEvent {
    frame: u64,
    event_type: String,
    node_path: String,
    data: String, // JSON
}

/// In-memory marker awaiting flush to SQLite.
struct CapturedMarker {
    frame: u64,
    timestamp_ms: u64,
    source: String, // "human", "agent", "system"
    label: String,
}

/// Lightweight entity data for recording frames (subset of EntityData).
/// Serialized to MessagePack for compact storage.
#[derive(Serialize, Deserialize)]
struct FrameEntityData {
    path: String,
    class: String,
    position: Vec<f64>,
    rotation_deg: Vec<f64>,
    velocity: Vec<f64>,
    groups: Vec<String>,
    visible: bool,
    state: serde_json::Map<String, serde_json::Value>,
}

/// Recording capture configuration.
#[derive(Debug, Clone)]
struct CaptureConfig {
    capture_interval: u32,
    max_frames: u32,
    include_signals: bool,
    include_input: bool,
    groups: Vec<String>,       // empty = all
    properties: Vec<String>,   // empty = all exported
}

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorRecorder {
    base: Base<Node>,

    // Recording state
    recording: bool,
    recording_id: String,
    recording_name: String,
    started_at_frame: u64,
    started_at_ms: u64,
    frames_captured: u32,
    frame_counter: u32, // physics frames since recording start

    // Capture config
    capture_interval: u32,
    max_frames: u32,

    // Buffers (flushed to SQLite periodically)
    frame_buffer: Vec<CapturedFrame>,
    event_buffer: Vec<CapturedEvent>,
    marker_buffer: Vec<CapturedMarker>,
    flush_counter: u32, // frames since last flush

    // SQLite connection (open during recording)
    db: Option<Connection>,
    storage_path: String,

    // Collector reference for snapshot data
    collector: Option<Gd<SpectatorCollector>>,
}
```

#### Lifecycle

```rust
#[godot_api]
impl INode for SpectatorRecorder {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            recording: false,
            recording_id: String::new(),
            recording_name: String::new(),
            started_at_frame: 0,
            started_at_ms: 0,
            frames_captured: 0,
            frame_counter: 0,
            capture_interval: 1,
            max_frames: 36000,
            frame_buffer: Vec::new(),
            event_buffer: Vec::new(),
            marker_buffer: Vec::new(),
            flush_counter: 0,
            db: None,
            storage_path: String::new(),
            collector: None,
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        if !self.recording {
            return;
        }
        self.frame_counter += 1;

        // Capture at configured interval
        if self.frame_counter % self.capture_interval != 0 {
            return;
        }

        // Safety valve
        if self.frames_captured >= self.max_frames {
            self.stop_recording();
            tracing::warn!("Recording stopped: max_frames ({}) reached", self.max_frames);
            return;
        }

        self.capture_frame();

        // Periodic flush (every 60 captured frames ≈ every second at 60fps/interval=1)
        self.flush_counter += 1;
        if self.flush_counter >= 60 {
            self.flush_to_db();
            self.flush_counter = 0;
        }
    }
}
```

#### Exported API

```rust
#[godot_api]
impl SpectatorRecorder {
    #[signal]
    fn recording_started(recording_id: GString, name: GString);

    #[signal]
    fn recording_stopped(recording_id: GString, frames: u32);

    #[signal]
    fn marker_added(frame: u64, source: GString, label: GString);

    #[func]
    pub fn set_collector(&mut self, collector: Gd<SpectatorCollector>) {
        self.collector = Some(collector);
    }

    #[func]
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    #[func]
    pub fn get_recording_id(&self) -> GString {
        GString::from(&self.recording_id)
    }

    #[func]
    pub fn get_frames_captured(&self) -> u32 {
        self.frames_captured
    }

    #[func]
    pub fn get_recording_name(&self) -> GString {
        GString::from(&self.recording_name)
    }

    #[func]
    pub fn get_elapsed_ms(&self) -> u64 {
        if !self.recording { return 0; }
        let now_ms = current_time_ms();
        now_ms.saturating_sub(self.started_at_ms)
    }

    #[func]
    pub fn get_buffer_size_kb(&self) -> u32 {
        let bytes: usize = self.frame_buffer.iter().map(|f| f.data.len()).sum();
        (bytes / 1024) as u32
    }

    #[func]
    pub fn start_recording(
        &mut self,
        name: GString,
        storage_path: GString,
        capture_interval: u32,
        max_frames: u32,
    ) -> GString {
        // Returns recording_id or empty string on error
    }

    #[func]
    pub fn stop_recording(&mut self) -> Dictionary {
        // Returns metadata dict: { recording_id, name, frames_captured, duration_ms, ... }
    }

    #[func]
    pub fn add_marker(&mut self, source: GString, label: GString) {
        // Adds a marker at the current frame
    }

    #[func]
    pub fn list_recordings(&self, storage_path: GString) -> Array<Dictionary> {
        // Lists all .sqlite files in storage_path, reads metadata
    }

    #[func]
    pub fn delete_recording(&self, storage_path: GString, recording_id: GString) -> bool {
        // Deletes the .sqlite file for the given recording_id
    }

    #[func]
    pub fn get_recording_status(&self) -> Dictionary {
        // Returns current recording status as a dictionary
    }

    #[func]
    pub fn get_recording_markers(
        &self,
        storage_path: GString,
        recording_id: GString,
    ) -> Array<Dictionary> {
        // Returns markers for a recording by reading its SQLite file
    }
}
```

#### Internal Methods

```rust
impl SpectatorRecorder {
    /// Capture one frame of entity data from the collector.
    fn capture_frame(&mut self) {
        let Some(ref collector) = self.collector else { return };

        // Use collector's snapshot collection with minimal params
        let params = spectator_protocol::query::GetSnapshotDataParams {
            perspective: spectator_protocol::query::PerspectiveParam::Camera,
            radius: f64::MAX,       // capture everything
            include_offscreen: true, // capture everything
            groups: vec![],
            class_filter: vec![],
            detail: spectator_protocol::query::DetailLevel::Standard,
            expose_internals: false,
        };

        let snapshot = collector.bind().collect_snapshot(&params);

        // Convert to lightweight frame data
        let frame_entities: Vec<FrameEntityData> = snapshot.entities.iter().map(|e| {
            FrameEntityData {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position.clone(),
                rotation_deg: e.rotation_deg.clone(),
                velocity: e.velocity.clone(),
                groups: e.groups.clone(),
                visible: e.visible,
                state: e.state.clone(),
            }
        }).collect();

        // Serialize to MessagePack
        let data = match rmp_serde::to_vec(&frame_entities) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to serialize frame data: {e}");
                return;
            }
        };

        self.frame_buffer.push(CapturedFrame {
            frame: snapshot.frame,
            timestamp_ms: snapshot.timestamp_ms,
            data,
        });

        self.frames_captured += 1;
    }

    /// Flush buffered frames, events, and markers to SQLite.
    fn flush_to_db(&mut self) {
        let Some(ref db) = self.db else { return };

        // Batch insert frames
        if !self.frame_buffer.is_empty() {
            let tx = match db.unchecked_transaction() {
                Ok(tx) => tx,
                Err(e) => { tracing::error!("SQLite transaction error: {e}"); return; }
            };
            {
                let mut stmt = match tx.prepare_cached(
                    "INSERT OR REPLACE INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)"
                ) {
                    Ok(s) => s,
                    Err(e) => { tracing::error!("SQLite prepare error: {e}"); return; }
                };
                for f in &self.frame_buffer {
                    let _ = stmt.execute(rusqlite::params![f.frame, f.timestamp_ms, &f.data]);
                }
            }
            // Events
            {
                let mut stmt = match tx.prepare_cached(
                    "INSERT INTO events (frame, event_type, node_path, data) VALUES (?1, ?2, ?3, ?4)"
                ) {
                    Ok(s) => s,
                    Err(e) => { tracing::error!("SQLite prepare error: {e}"); return; }
                };
                for ev in &self.event_buffer {
                    let _ = stmt.execute(rusqlite::params![ev.frame, &ev.event_type, &ev.node_path, &ev.data]);
                }
            }
            // Markers
            {
                let mut stmt = match tx.prepare_cached(
                    "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)"
                ) {
                    Ok(s) => s,
                    Err(e) => { tracing::error!("SQLite prepare error: {e}"); return; }
                };
                for m in &self.marker_buffer {
                    let _ = stmt.execute(rusqlite::params![m.frame, m.timestamp_ms, &m.source, &m.label]);
                }
            }
            if let Err(e) = tx.commit() {
                tracing::error!("SQLite commit error: {e}");
            }
        }

        self.frame_buffer.clear();
        self.event_buffer.clear();
        self.marker_buffer.clear();
    }

    /// Create the SQLite database file and initialize schema.
    fn create_db(&mut self, path: &str) -> Result<(), String> {
        let db = Connection::open(path).map_err(|e| format!("SQLite open error: {e}"))?;
        db.execute_batch("PRAGMA journal_mode=WAL;").map_err(|e| format!("WAL error: {e}"))?;
        db.execute_batch(SCHEMA_SQL).map_err(|e| format!("Schema error: {e}"))?;
        self.db = Some(db);
        Ok(())
    }
}

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS recording (
    id TEXT PRIMARY KEY,
    name TEXT,
    started_at_frame INTEGER,
    ended_at_frame INTEGER,
    started_at_ms INTEGER,
    ended_at_ms INTEGER,
    scene_dimensions INTEGER,
    physics_ticks_per_sec INTEGER,
    capture_config TEXT
);

CREATE TABLE IF NOT EXISTS frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    data BLOB
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER,
    event_type TEXT,
    node_path TEXT,
    data TEXT,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);

CREATE TABLE IF NOT EXISTS markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER,
    timestamp_ms INTEGER,
    source TEXT,
    label TEXT,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);

CREATE INDEX IF NOT EXISTS idx_events_frame ON events(frame);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_node ON events(node_path);
CREATE INDEX IF NOT EXISTS idx_markers_frame ON markers(frame);
";

/// Get current time in milliseconds since Unix epoch.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```

#### start_recording Implementation

```rust
#[func]
pub fn start_recording(
    &mut self,
    name: GString,
    storage_path: GString,
    capture_interval: u32,
    max_frames: u32,
) -> GString {
    if self.recording {
        tracing::warn!("Recording already active");
        return GString::new();
    }

    let recording_id = format!("rec_{:08x}", rand_u32());
    let name_str = name.to_string();
    let recording_name = if name_str.is_empty() {
        format!("recording_{}", chrono_like_timestamp())
    } else {
        name_str
    };

    let storage = storage_path.to_string();
    // Ensure storage directory exists
    let dir_path = globalize_path(&storage);
    let _ = std::fs::create_dir_all(&dir_path);

    let db_path = format!("{}/{}.sqlite", dir_path, recording_id);
    if let Err(e) = self.create_db(&db_path) {
        tracing::error!("Failed to create recording database: {e}");
        return GString::new();
    }

    // Write recording metadata
    if let Some(ref db) = self.db {
        let config_json = serde_json::json!({
            "capture_interval": capture_interval,
            "max_frames": max_frames,
        });
        let _ = db.execute(
            "INSERT INTO recording (id, name, started_at_frame, started_at_ms, capture_config) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &recording_id,
                &recording_name,
                current_physics_frame(),
                current_time_ms(),
                config_json.to_string(),
            ],
        );
    }

    self.recording = true;
    self.recording_id = recording_id.clone();
    self.recording_name = recording_name;
    self.started_at_frame = current_physics_frame();
    self.started_at_ms = current_time_ms();
    self.frames_captured = 0;
    self.frame_counter = 0;
    self.flush_counter = 0;
    self.capture_interval = capture_interval.max(1);
    self.max_frames = max_frames;
    self.storage_path = storage;

    self.base_mut().emit_signal(
        "recording_started",
        &[
            GString::from(&self.recording_id).to_variant(),
            GString::from(&self.recording_name).to_variant(),
        ],
    );

    GString::from(&recording_id)
}
```

#### stop_recording Implementation

```rust
#[func]
pub fn stop_recording(&mut self) -> Dictionary {
    if !self.recording {
        return Dictionary::new();
    }

    // Final flush
    self.flush_to_db();

    // Update recording metadata
    let ended_frame = current_physics_frame();
    let ended_ms = current_time_ms();
    if let Some(ref db) = self.db {
        let _ = db.execute(
            "UPDATE recording SET ended_at_frame = ?1, ended_at_ms = ?2 WHERE id = ?3",
            rusqlite::params![ended_frame, ended_ms, &self.recording_id],
        );
    }

    // Close the database
    self.db = None;

    // Build result metadata
    let mut result = Dictionary::new();
    result.set("recording_id", GString::from(&self.recording_id));
    result.set("name", GString::from(&self.recording_name));
    result.set("frames_captured", self.frames_captured);
    result.set("duration_ms", ended_ms.saturating_sub(self.started_at_ms));
    result.set("started_at_frame", self.started_at_frame);
    result.set("ended_at_frame", ended_frame);

    let frames_captured = self.frames_captured;
    let recording_id = self.recording_id.clone();

    self.recording = false;
    self.recording_id.clear();
    self.recording_name.clear();
    self.frame_buffer.clear();
    self.event_buffer.clear();
    self.marker_buffer.clear();

    self.base_mut().emit_signal(
        "recording_stopped",
        &[
            GString::from(&recording_id).to_variant(),
            frames_captured.to_variant(),
        ],
    );

    result
}
```

#### add_marker Implementation

```rust
#[func]
pub fn add_marker(&mut self, source: GString, label: GString) {
    let frame = current_physics_frame();
    let timestamp_ms = current_time_ms();

    self.marker_buffer.push(CapturedMarker {
        frame,
        timestamp_ms,
        source: source.to_string(),
        label: label.to_string(),
    });

    self.base_mut().emit_signal(
        "marker_added",
        &[
            frame.to_variant(),
            source.to_variant(),
            label.to_variant(),
        ],
    );
}
```

#### list_recordings Implementation

```rust
#[func]
pub fn list_recordings(&self, storage_path: GString) -> Array<Dictionary> {
    let dir_path = globalize_path(&storage_path.to_string());
    let mut result = Array::new();

    let entries = match std::fs::read_dir(&dir_path) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
            continue;
        }

        if let Ok(db) = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            if let Ok(mut stmt) = db.prepare(
                "SELECT id, name, started_at_frame, ended_at_frame, \
                 started_at_ms, ended_at_ms FROM recording LIMIT 1"
            ) {
                if let Ok(Some(row)) = stmt.query_row([], |row| {
                    let id: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let start_frame: i64 = row.get(2)?;
                    let end_frame: Option<i64> = row.get(3)?;
                    let start_ms: i64 = row.get(4)?;
                    let end_ms: Option<i64> = row.get(5)?;
                    Ok((id, name, start_frame, end_frame, start_ms, end_ms))
                }).ok() {
                    let (id, name, start_frame, end_frame, start_ms, end_ms) = row;

                    // Count frames
                    let frame_count: i64 = db
                        .query_row("SELECT COUNT(*) FROM frames", [], |r| r.get(0))
                        .unwrap_or(0);

                    // Count markers
                    let marker_count: i64 = db
                        .query_row("SELECT COUNT(*) FROM markers", [], |r| r.get(0))
                        .unwrap_or(0);

                    let duration_ms = end_ms.unwrap_or(0) - start_ms;

                    // File size
                    let size_kb = std::fs::metadata(&path)
                        .map(|m| m.len() / 1024)
                        .unwrap_or(0);

                    let mut dict = Dictionary::new();
                    dict.set("id", GString::from(&id));
                    dict.set("name", GString::from(&name));
                    dict.set("frames", frame_count as u32);
                    dict.set("duration_ms", duration_ms);
                    dict.set("frame_range_start", start_frame);
                    dict.set("frame_range_end", end_frame.unwrap_or(start_frame));
                    dict.set("markers_count", marker_count as u32);
                    dict.set("size_kb", size_kb as u32);
                    dict.set("created_at_ms", start_ms);
                    result.push(&dict);
                }
            }
        }
    }

    result
}
```

#### Helper Functions

```rust
/// Get current physics frame number. Must be called from main thread.
fn current_physics_frame() -> u64 {
    // Access Godot's Engine singleton
    godot::classes::Engine::singleton().get_physics_frames()
}

/// Resolve `user://` paths to absolute filesystem paths.
fn globalize_path(godot_path: &str) -> String {
    godot::classes::ProjectSettings::singleton()
        .globalize_path(godot_path.into())
        .to_string()
}

/// Generate a simple random u32 for recording IDs.
fn rand_u32() -> u32 {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (t.as_nanos() & 0xFFFF_FFFF) as u32
}

/// Generate a timestamp string like "2026-03-06_14-30-05".
fn chrono_like_timestamp() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple: use seconds since epoch as the name. Readable timestamps
    // would require a time library; this is sufficient for auto-naming.
    format!("{secs}")
}
```

**Implementation Notes:**

- The recorder does NOT run as a Godot `@tool` script — it only runs during gameplay.
- `physics_process` is used (not `_process`) to align with physics tick boundaries.
- `collect_snapshot` is called with `radius: f64::MAX` and `include_offscreen: true` to capture everything in the scene.
- MessagePack is ~40-60% smaller than JSON for typical frame data. A 50-entity frame at ~3KB MessagePack over 1800 frames (30s at 60fps) = ~5.4MB — well within SQLite's capabilities.
- `unchecked_transaction()` is used instead of `transaction()` because the connection is single-threaded (main thread only) and we don't need interior mutability checks.
- `current_physics_frame()` reads from `Engine::get_physics_frames()` via the Godot singleton, which is safe on the main thread.

**Acceptance Criteria:**
- [ ] `SpectatorRecorder` class available in Godot after loading GDExtension
- [ ] `start_recording()` creates a `.sqlite` file in the storage path with correct schema
- [ ] `physics_process` captures frames at the configured interval
- [ ] `stop_recording()` returns metadata and closes the database
- [ ] Max frames safety valve stops recording when limit reached
- [ ] Periodic flush writes frames to SQLite every 60 captured frames
- [ ] `add_marker()` stores markers with source and label
- [ ] `list_recordings()` reads metadata from all `.sqlite` files in storage path
- [ ] `delete_recording()` removes the `.sqlite` file
- [ ] Signals emitted on start, stop, and marker add

---

### Unit 3: Recording Query Handler (GDExtension)

**File:** `crates/spectator-godot/src/recording_handler.rs` (new)

The TCP server dispatches recording-related query methods to the recorder. This handler translates between TCP JSON messages and the recorder's `#[func]` API.

```rust
use godot::prelude::*;
use serde_json::{json, Value};

use crate::recorder::SpectatorRecorder;

/// Handle recording-related TCP queries dispatched from the TCP server.
pub fn handle_recording_query(
    recorder: &mut Gd<SpectatorRecorder>,
    method: &str,
    params: &Value,
) -> Result<Value, (String, String)> {
    match method {
        "recording_start" => handle_start(recorder, params),
        "recording_stop" => handle_stop(recorder),
        "recording_status" => handle_status(recorder),
        "recording_list" => handle_list(recorder, params),
        "recording_delete" => handle_delete(recorder, params),
        "recording_marker" => handle_marker(recorder, params),
        "recording_markers" => handle_get_markers(recorder, params),
        _ => Err(("method_not_found".into(), format!("Unknown recording method: {method}"))),
    }
}

fn handle_start(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    if rec.is_recording() {
        return Err(("recording_active".into(), "A recording is already active".into()));
    }
    drop(rec);

    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let storage_path = params.get("storage_path").and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");
    let interval = params.get("capture_interval").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let max_frames = params.get("max_frames").and_then(|v| v.as_u64()).unwrap_or(36000) as u32;

    let id = recorder.bind_mut().start_recording(
        GString::from(name),
        GString::from(storage_path),
        interval,
        max_frames,
    );

    if id.is_empty() {
        return Err(("internal_error".into(), "Failed to start recording".into()));
    }

    let rec = recorder.bind();
    Ok(json!({
        "recording_id": id.to_string(),
        "name": rec.get_recording_name().to_string(),
        "started_at_frame": current_physics_frame(),
    }))
}

fn handle_stop(recorder: &mut Gd<SpectatorRecorder>) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    if !rec.is_recording() {
        return Err(("no_recording_active".into(), "No recording is active".into()));
    }
    drop(rec);

    let meta = recorder.bind_mut().stop_recording();

    Ok(json!({
        "recording_id": meta.get("recording_id").map(|v| v.to_string()).unwrap_or_default(),
        "name": meta.get("name").map(|v| v.to_string()).unwrap_or_default(),
        "frames_captured": meta.get("frames_captured").map(|v: Variant| v.to::<u32>()).unwrap_or(0),
        "duration_ms": meta.get("duration_ms").map(|v: Variant| v.to::<u64>()).unwrap_or(0),
        "frame_range": [
            meta.get("started_at_frame").map(|v: Variant| v.to::<u64>()).unwrap_or(0),
            meta.get("ended_at_frame").map(|v: Variant| v.to::<u64>()).unwrap_or(0),
        ],
    }))
}

fn handle_status(recorder: &mut Gd<SpectatorRecorder>) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    Ok(json!({
        "recording_active": rec.is_recording(),
        "recording_id": rec.get_recording_id().to_string(),
        "name": rec.get_recording_name().to_string(),
        "frames_captured": rec.get_frames_captured(),
        "duration_ms": rec.get_elapsed_ms(),
        "buffer_size_kb": rec.get_buffer_size_kb(),
    }))
}

fn handle_list(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let storage_path = params.get("storage_path").and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let recordings = recorder.bind().list_recordings(GString::from(storage_path));

    let list: Vec<Value> = recordings.iter_shared().map(|dict| {
        json!({
            "id": dict.get("id").map(|v| v.to_string()).unwrap_or_default(),
            "name": dict.get("name").map(|v| v.to_string()).unwrap_or_default(),
            "frames": dict.get("frames").map(|v: Variant| v.to::<u32>()).unwrap_or(0),
            "duration_ms": dict.get("duration_ms").map(|v: Variant| v.to::<i64>()).unwrap_or(0),
            "frame_range": [
                dict.get("frame_range_start").map(|v: Variant| v.to::<i64>()).unwrap_or(0),
                dict.get("frame_range_end").map(|v: Variant| v.to::<i64>()).unwrap_or(0),
            ],
            "markers_count": dict.get("markers_count").map(|v: Variant| v.to::<u32>()).unwrap_or(0),
            "size_kb": dict.get("size_kb").map(|v: Variant| v.to::<u32>()).unwrap_or(0),
            "created_at_ms": dict.get("created_at_ms").map(|v: Variant| v.to::<i64>()).unwrap_or(0),
        })
    }).collect();

    Ok(json!({ "recordings": list }))
}

fn handle_delete(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params.get("recording_id").and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "recording_id is required".to_string()))?;
    let storage_path = params.get("storage_path").and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let ok = recorder.bind().delete_recording(
        GString::from(storage_path),
        GString::from(id),
    );

    if ok {
        Ok(json!({ "result": "ok", "deleted": id }))
    } else {
        Err(("recording_not_found".into(), format!("Recording '{id}' not found")))
    }
}

fn handle_marker(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    if !rec.is_recording() {
        return Err(("no_recording_active".into(), "No recording is active to add a marker to".into()));
    }
    drop(rec);

    let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("agent");
    let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("");

    recorder.bind_mut().add_marker(GString::from(source), GString::from(label));

    Ok(json!({
        "result": "ok",
        "frame": current_physics_frame(),
        "source": source,
        "label": label,
    }))
}

fn handle_get_markers(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params.get("recording_id").and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "recording_id is required".to_string()))?;
    let storage_path = params.get("storage_path").and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let markers = recorder.bind().get_recording_markers(
        GString::from(storage_path),
        GString::from(id),
    );

    let list: Vec<Value> = markers.iter_shared().map(|dict| {
        json!({
            "frame": dict.get("frame").map(|v: Variant| v.to::<u64>()).unwrap_or(0),
            "timestamp_ms": dict.get("timestamp_ms").map(|v: Variant| v.to::<u64>()).unwrap_or(0),
            "source": dict.get("source").map(|v| v.to_string()).unwrap_or_default(),
            "label": dict.get("label").map(|v| v.to_string()).unwrap_or_default(),
        })
    }).collect();

    Ok(json!({ "recording_id": id, "markers": list }))
}

fn current_physics_frame() -> u64 {
    godot::classes::Engine::singleton().get_physics_frames()
}
```

#### TCP Server Integration

**File:** `crates/spectator-godot/src/tcp_server.rs` (modify)

Add recorder reference and dispatch recording methods:

```rust
// Add field to SpectatorTCPServer:
recorder: Option<Gd<SpectatorRecorder>>,

// Add func:
#[func]
pub fn set_recorder(&mut self, recorder: Gd<SpectatorRecorder>) {
    self.recorder = Some(recorder);
}

// In the query dispatch method (where incoming Query messages are handled):
// Add recording method routing:
fn handle_query(&mut self, id: &str, method: &str, params: &serde_json::Value) {
    // ... existing method dispatch ...
    if method.starts_with("recording_") {
        if let Some(ref mut recorder) = self.recorder {
            match crate::recording_handler::handle_recording_query(recorder, method, params) {
                Ok(data) => self.send_response(id, data),
                Err((code, message)) => self.send_error(id, &code, &message),
            }
            return;
        } else {
            self.send_error(id, "internal_error", "Recorder not available");
            return;
        }
    }
    // ... existing dispatch continues ...
}
```

**Implementation Notes:**
- Recording queries are routed to `recording_handler` based on the `recording_` method prefix.
- The `storage_path` parameter defaults to the Project Settings value. The server passes it through from the MCP tool's params.
- The handler does NOT access Godot's scene tree directly — it delegates to the recorder's `#[func]` methods.

**Acceptance Criteria:**
- [ ] TCP queries with `recording_*` methods dispatch to the recording handler
- [ ] `recording_start` → creates recording, returns ID
- [ ] `recording_stop` → stops recording, returns metadata
- [ ] `recording_status` → returns current state
- [ ] `recording_list` → returns all recordings
- [ ] `recording_delete` → deletes a recording file
- [ ] `recording_marker` → adds a marker to the active recording
- [ ] `recording_markers` → returns markers for a recording
- [ ] Error responses for: already recording, no recording active, recording not found

---

### Unit 4: Recording MCP Tool

**File:** `crates/spectator-server/src/mcp/recording.rs` (new)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tcp::{SessionState, query_addon};
use super::{serialize_params, finalize_response};
use spectator_core::budget::resolve_budget;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    /// Action to perform.
    /// "start" — begin recording.
    /// "stop" — end recording.
    /// "status" — check if recording.
    /// "list" — list saved recordings.
    /// "delete" — remove a recording.
    /// "markers" — list markers in a recording.
    /// "add_marker" — add an agent marker to the active recording.
    pub action: String,

    /// Name for the recording (start only). Auto-generated if omitted.
    pub recording_name: Option<String>,

    /// Capture configuration (start only).
    pub capture: Option<CaptureConfig>,

    /// Recording to query (markers, delete). Uses most recent if omitted.
    pub recording_id: Option<String>,

    /// Marker label (add_marker only).
    pub marker_label: Option<String>,

    /// Frame to attach marker to (add_marker only). Defaults to current frame.
    pub marker_frame: Option<u64>,

    /// Token budget for the response.
    pub token_budget: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureConfig {
    /// Capture every N physics frames. Default 1.
    pub capture_interval: Option<u32>,
    /// Maximum frames to capture. Default 36000.
    pub max_frames: Option<u32>,
    /// Whether to capture signal emissions. Default true.
    pub include_signals: Option<bool>,
    /// Whether to capture input events. Default false.
    pub include_input: Option<bool>,
}

/// Handle the recording MCP tool.
pub async fn handle_recording(
    params: RecordingParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        "start" => handle_start(&params, state, budget_limit, hard_cap).await,
        "stop" => handle_stop(state, budget_limit, hard_cap).await,
        "status" => handle_status(state, budget_limit, hard_cap).await,
        "list" => handle_list(state, budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,
        other => Err(McpError::invalid_params(
            format!("Unknown recording action: '{other}'. Valid: start, stop, status, list, delete, markers, add_marker"),
            None,
        )),
    }
}

async fn handle_start(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let capture = params.capture.as_ref();
    let query_params = json!({
        "name": params.recording_name.as_deref().unwrap_or(""),
        "capture_interval": capture.and_then(|c| c.capture_interval).unwrap_or(1),
        "max_frames": capture.and_then(|c| c.max_frames).unwrap_or(36000),
    });

    let data = query_addon(state, "recording_start", query_params).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_stop(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_stop", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_status(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_status", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_list(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_list", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_delete(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = params.recording_id.as_deref().ok_or_else(|| {
        McpError::invalid_params("recording_id is required for delete".to_string(), None)
    })?;
    let data = query_addon(state, "recording_delete", json!({ "recording_id": id })).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_markers(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let query = json!({
        "recording_id": params.recording_id,
    });
    let data = query_addon(state, "recording_markers", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_add_marker(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let query = json!({
        "source": "agent",
        "label": params.marker_label.as_deref().unwrap_or(""),
    });
    let data = query_addon(state, "recording_marker", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}
```

#### MCP Tool Registration

**File:** `crates/spectator-server/src/mcp/mod.rs` (modify)

Add to the module declarations:

```rust
pub mod recording;
```

Add to the `#[tool_router]` impl block:

```rust
/// Capture and analyze play sessions. Start/stop recording, manage markers,
/// list saved recordings.
#[tool(description = "Capture and manage play session recordings. Actions: 'start' (begin recording with optional name and capture config), 'stop' (end recording, get metadata), 'status' (check recording state), 'list' (list saved recordings), 'delete' (remove a recording by recording_id), 'markers' (list markers in a recording), 'add_marker' (add an agent marker to the active recording with marker_label).")]
pub async fn recording(
    &self,
    Parameters(params): Parameters<recording::RecordingParams>,
) -> Result<String, McpError> {
    let summary = crate::activity::recording_summary(&params);
    let result = recording::handle_recording(params, &self.state).await;
    self.log_activity("recording", &summary, "recording").await;
    result
}
```

**Acceptance Criteria:**
- [ ] `recording` MCP tool appears in tool listing
- [ ] `recording(action: "start")` begins a recording session
- [ ] `recording(action: "stop")` ends recording, returns metadata with frame count and duration
- [ ] `recording(action: "status")` returns recording state
- [ ] `recording(action: "list")` returns all saved recordings with metadata
- [ ] `recording(action: "delete", recording_id: "...")` removes a recording
- [ ] `recording(action: "markers")` lists markers for a recording
- [ ] `recording(action: "add_marker", marker_label: "...")` adds an agent marker
- [ ] Budget block present on all responses
- [ ] Error responses: `recording_active`, `no_recording_active`, `recording_not_found`

---

### Unit 5: Activity Log — Recording Summaries

**File:** `crates/spectator-server/src/activity.rs` (modify)

Add recording summary generation:

```rust
use crate::mcp::recording::RecordingParams;

pub fn recording_summary(params: &RecordingParams) -> String {
    match params.action.as_str() {
        "start" => {
            let name = params.recording_name.as_deref().unwrap_or("(auto)");
            format!("Started recording: {name}")
        }
        "stop" => "Stopped recording".into(),
        "status" => "Checking recording status".into(),
        "list" => "Listing recordings".into(),
        "delete" => {
            let id = params.recording_id.as_deref().unwrap_or("?");
            format!("Deleted recording {id}")
        }
        "markers" => {
            let id = params.recording_id.as_deref().unwrap_or("current");
            format!("Listing markers for {id}")
        }
        "add_marker" => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Added marker: {label}")
        }
        other => format!("Recording: {other}"),
    }
}
```

**Acceptance Criteria:**
- [ ] Activity feed shows recording actions with descriptive summaries
- [ ] Dock displays recording entries in cyan color (already mapped in dock.gd)

---

### Unit 6: Dock Recording Section

**File:** `addons/spectator/dock.tscn` (modify)

Insert a recording section between the Session section and the Activity section:

```
[node name="HSeparator3" type="HSeparator" parent="."]

[node name="RecordingSection" type="VBoxContainer" parent="."]

[node name="RecordingHeader" type="Label" parent="RecordingSection"]
text = "Recording"

[node name="RecordingControls" type="HBoxContainer" parent="RecordingSection"]

[node name="RecordBtn" type="Button" parent="RecordingSection/RecordingControls"]
unique_name_in_owner = true
text = "Record"

[node name="StopBtn" type="Button" parent="RecordingSection/RecordingControls"]
unique_name_in_owner = true
text = "Stop"
disabled = true

[node name="MarkerBtn" type="Button" parent="RecordingSection/RecordingControls"]
unique_name_in_owner = true
text = "Marker"
disabled = true

[node name="RecordingStats" type="Label" parent="RecordingSection"]
unique_name_in_owner = true
text = ""
visible = false

[node name="HSeparator4" type="HSeparator" parent="."]

[node name="RecordingsHeader" type="Label" parent="."]
text = "Recordings"

[node name="RecordingLibrary" type="VBoxContainer" parent="."]
unique_name_in_owner = true
```

**File:** `addons/spectator/dock.gd` (modify)

Add recording control logic:

```gdscript
# New node references
@onready var record_btn: Button = %RecordBtn
@onready var stop_btn: Button = %StopBtn
@onready var marker_btn: Button = %MarkerBtn
@onready var recording_stats: Label = %RecordingStats
@onready var recording_library: VBoxContainer = %RecordingLibrary

# New state
var _recorder: Object  # SpectatorRecorder
var _recording_active := false
var _library_dirty := true  # refresh on next update


func _ready() -> void:
    collapse_btn.pressed.connect(_toggle_collapse)
    record_btn.pressed.connect(_on_record_pressed)
    stop_btn.pressed.connect(_on_stop_pressed)
    marker_btn.pressed.connect(_on_marker_pressed)
    _update_status()


func _on_record_pressed() -> void:
    if not _recorder or not is_instance_valid(_recorder):
        return
    var storage_path: String = ProjectSettings.get_setting(
        "spectator/recording/storage_path", "user://spectator_recordings/")
    var interval: int = ProjectSettings.get_setting(
        "spectator/recording/capture_interval", 1)
    var max_frames: int = ProjectSettings.get_setting(
        "spectator/recording/max_frames", 36000)

    # TODO: Optional name dialog. For now, empty string = auto-name.
    var id: String = _recorder.start_recording("", storage_path, interval, max_frames)
    if not id.is_empty():
        _recording_active = true
        _update_recording_controls()


func _on_stop_pressed() -> void:
    if not _recorder or not is_instance_valid(_recorder):
        return
    _recorder.stop_recording()
    _recording_active = false
    _library_dirty = true
    _update_recording_controls()


func _on_marker_pressed() -> void:
    if not _recorder or not is_instance_valid(_recorder):
        return
    # TODO: Optional text input dialog. For now, empty label.
    _recorder.add_marker("human", "")


func _update_recording_controls() -> void:
    record_btn.disabled = _recording_active
    stop_btn.disabled = not _recording_active
    marker_btn.disabled = not _recording_active
    recording_stats.visible = _recording_active


func _try_acquire_runtime() -> void:
    # ... existing code to get _tcp_server ...
    # Also acquire recorder reference:
    if not _recorder or not is_instance_valid(_recorder):
        var rt_script := load("res://addons/spectator/runtime.gd")
        if rt_script:
            var rt = rt_script.get("instance")
            if rt and is_instance_valid(rt):
                var rec = rt.get("recorder")
                if rec and is_instance_valid(rec):
                    _recorder = rec
                    _recorder.recording_stopped.connect(_on_recording_stopped)


func _on_recording_stopped(_id: String, _frames: int) -> void:
    _recording_active = false
    _library_dirty = true
    _update_recording_controls()


func _update_status() -> void:
    # ... existing status update code ...

    # Recording stats (during recording)
    if _recording_active and _recorder and is_instance_valid(_recorder):
        var elapsed_ms: int = _recorder.get_elapsed_ms()
        var elapsed_sec := elapsed_ms / 1000.0
        var frames: int = _recorder.get_frames_captured()
        var buffer_kb: int = _recorder.get_buffer_size_kb()
        recording_stats.text = "  %s  |  Frame %d  |  %d KB" % [
            _format_elapsed(elapsed_sec), frames, buffer_kb,
        ]

    # Refresh recording library
    if _library_dirty and _recorder and is_instance_valid(_recorder):
        _refresh_library()
        _library_dirty = false


func _refresh_library() -> void:
    # Clear existing entries
    for child in recording_library.get_children():
        child.queue_free()

    if not _recorder or not is_instance_valid(_recorder):
        return

    var storage_path: String = ProjectSettings.get_setting(
        "spectator/recording/storage_path", "user://spectator_recordings/")
    var recordings: Array = _recorder.list_recordings(storage_path)

    # Sort by created_at_ms descending (most recent first)
    recordings.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
        return a.get("created_at_ms", 0) > b.get("created_at_ms", 0)
    )

    for rec: Dictionary in recordings:
        var entry := HBoxContainer.new()
        var name_label := Label.new()
        name_label.text = rec.get("name", "?")
        name_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
        entry.add_child(name_label)

        var dur_sec: float = rec.get("duration_ms", 0) / 1000.0
        var dur_label := Label.new()
        dur_label.text = "%0.1fs" % dur_sec
        entry.add_child(dur_label)

        var del_btn := Button.new()
        del_btn.text = "x"
        var rec_id: String = rec.get("id", "")
        del_btn.pressed.connect(func() -> void:
            _delete_recording(rec_id)
        )
        entry.add_child(del_btn)

        recording_library.add_child(entry)


func _delete_recording(recording_id: String) -> void:
    if not _recorder or not is_instance_valid(_recorder):
        return
    var storage_path: String = ProjectSettings.get_setting(
        "spectator/recording/storage_path", "user://spectator_recordings/")
    _recorder.delete_recording(storage_path, recording_id)
    _library_dirty = true


static func _format_elapsed(seconds: float) -> String:
    var mins := int(seconds) / 60
    var secs := fmod(seconds, 60.0)
    return "%02d:%04.1f" % [mins, secs]
```

**Implementation Notes:**
- The dock acquires the recorder reference the same way it acquires `_tcp_server` — via `runtime.gd`'s static `instance` var.
- Recording library refreshes when `_library_dirty` is set (on stop, on delete). Not every second — listing recordings reads disk.
- Delete button uses a closure to capture the recording ID.
- The record/stop/marker buttons directly call the recorder's `#[func]` methods (no TCP round-trip needed — the dock is in the same process as the GDExtension).

**Acceptance Criteria:**
- [ ] Record button starts a recording; disabled while recording
- [ ] Stop button stops the recording; disabled when not recording
- [ ] Marker button adds a human marker; disabled when not recording
- [ ] Stats line shows elapsed time, frame count, buffer size during recording
- [ ] Library section lists saved recordings (name, duration, delete button)
- [ ] Delete button removes a recording and refreshes the list
- [ ] Recording stopped by max_frames safety valve updates controls correctly

---

### Unit 7: Keyboard Shortcuts & Recording Indicator

**File:** `addons/spectator/runtime.gd` (modify)

Add recorder setup, F8/F9 handling, and recording indicator overlay:

```gdscript
# New fields
var recorder: SpectatorRecorder
var _recording_dot: ColorRect

# In _ready(), after existing setup:
func _ready() -> void:
    # ... existing code ...

    recorder = SpectatorRecorder.new()
    add_child(recorder)
    recorder.set_collector(collector)
    recorder.recording_stopped.connect(_on_recording_stopped)

    tcp_server.set_recorder(recorder)

    _setup_overlay()

# Add to _setup_overlay():
func _setup_overlay() -> void:
    # ... existing pause label and toast container ...

    # Recording indicator (red dot, top-left corner)
    _recording_dot = ColorRect.new()
    _recording_dot.color = Color(0.9, 0.1, 0.1)
    _recording_dot.custom_minimum_size = Vector2(16, 16)
    _recording_dot.set_anchors_preset(Control.PRESET_TOP_LEFT)
    _recording_dot.offset_left = 10
    _recording_dot.offset_top = 10
    _recording_dot.visible = false
    _overlay.add_child(_recording_dot)


# Updated _shortcut_input:
func _shortcut_input(event: InputEvent) -> void:
    if not event.is_pressed() or event.is_echo():
        return
    if event is InputEventKey:
        match event.keycode:
            KEY_F8:
                _toggle_recording()
                get_viewport().set_input_as_handled()
            KEY_F9:
                _drop_marker()
                get_viewport().set_input_as_handled()
            KEY_F10:
                _toggle_pause()
                get_viewport().set_input_as_handled()


func _toggle_recording() -> void:
    if not recorder:
        return
    if recorder.is_recording():
        recorder.stop_recording()
        _set_recording_indicator(false)
    else:
        var storage_path: String = ProjectSettings.get_setting(
            "spectator/recording/storage_path", "user://spectator_recordings/")
        var interval: int = ProjectSettings.get_setting(
            "spectator/recording/capture_interval", 1)
        var max_frames: int = ProjectSettings.get_setting(
            "spectator/recording/max_frames", 36000)
        var id: String = recorder.start_recording("", storage_path, interval, max_frames)
        if not id.is_empty():
            _set_recording_indicator(true)


func _drop_marker() -> void:
    if not recorder or not recorder.is_recording():
        return
    recorder.add_marker("human", "")
    # Brief visual flash for marker
    if _recording_dot:
        _recording_dot.color = Color.YELLOW
        get_tree().create_timer(0.3).timeout.connect(func() -> void:
            if _recording_dot:
                _recording_dot.color = Color(0.9, 0.1, 0.1)
        )


func _set_recording_indicator(visible: bool) -> void:
    if not ProjectSettings.get_setting(
            "spectator/display/show_recording_indicator", true):
        return
    if _recording_dot:
        _recording_dot.visible = visible


func _on_recording_stopped(_id: String, _frames: int) -> void:
    _set_recording_indicator(false)


# Updated _exit_tree:
func _exit_tree() -> void:
    instance = null
    if recorder and recorder.is_recording():
        recorder.stop_recording()
    if tcp_server:
        tcp_server.stop()
```

**Implementation Notes:**
- F8 toggles recording using the same Project Settings values as the dock buttons.
- F9 drops a human marker and briefly flashes the recording dot yellow as visual feedback.
- The recording dot is a `ColorRect` on the CanvasLayer at layer 128, top-left corner.
- On `_exit_tree` (game stop), any active recording is stopped to ensure the final flush.
- The recorder is created as a child of the runtime autoload, alongside the collector and TCP server.

**Acceptance Criteria:**
- [ ] F8 starts/stops recording
- [ ] F9 drops a human marker (only while recording)
- [ ] Red dot visible in top-left corner during recording
- [ ] Red dot hidden when `show_recording_indicator` is false
- [ ] F9 causes brief yellow flash on the recording dot
- [ ] Stopping the game while recording performs a clean stop (final flush)

---

### Unit 8: GDExtension Library Registration

**File:** `crates/spectator-godot/src/lib.rs` (modify)

Add `mod recorder` and `mod recording_handler`:

```rust
mod collector;
mod tcp_server;
mod query_handler;
mod action_handler;
mod recorder;
mod recording_handler;

use godot::prelude::*;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
```

**Acceptance Criteria:**
- [ ] `SpectatorRecorder` class registered with Godot when GDExtension loads
- [ ] `cargo build -p spectator-godot` compiles without errors

---

## File Inventory

### New Files

| File | Purpose |
|---|---|
| `crates/spectator-godot/src/recorder.rs` | SpectatorRecorder GDExtension class — frame capture, SQLite storage |
| `crates/spectator-godot/src/recording_handler.rs` | TCP query handler for recording methods |
| `crates/spectator-server/src/mcp/recording.rs` | Recording MCP tool handler |

### Modified Files

| File | Changes |
|---|---|
| `Cargo.toml` (workspace) | Add `rusqlite`, `rmp-serde` to workspace dependencies |
| `crates/spectator-godot/Cargo.toml` | Add `rusqlite`, `rmp-serde` dependencies |
| `crates/spectator-godot/src/lib.rs` | Add `mod recorder`, `mod recording_handler` |
| `crates/spectator-godot/src/tcp_server.rs` | Add `recorder` field, `set_recorder()`, recording method dispatch |
| `crates/spectator-server/src/mcp/mod.rs` | Add `pub mod recording`, `recording` tool in `#[tool_router]` |
| `crates/spectator-server/src/activity.rs` | Add `recording_summary()` function |
| `addons/spectator/runtime.gd` | Add recorder creation, F8/F9 handling, recording indicator |
| `addons/spectator/dock.tscn` | Add recording section and library section |
| `addons/spectator/dock.gd` | Add recording controls, library display, recorder signals |

### Unchanged

| File | Reason |
|---|---|
| `crates/spectator-protocol/` | No protocol changes — recording uses existing `Query`/`Response` message types |
| `crates/spectator-core/` | No core logic changes — recording is capture/storage, not spatial analysis |
| `addons/spectator/plugin.gd` | Recording settings already registered (M6 pre-work) |

---

## Implementation Order

1. **Unit 1: Dependencies** — add rusqlite, rmp-serde (enables compilation of everything else)
2. **Unit 2: SpectatorRecorder** — core recorder class (independent)
3. **Unit 8: Library registration** — register recorder module (enables Godot to see the class)
4. **Unit 3: Recording query handler** — TCP dispatch for recording methods (depends on Unit 2)
5. **Unit 5: Activity summaries** — recording summary generation (independent)
6. **Unit 4: Recording MCP tool** — server-side tool (depends on Units 3, 5)
7. **Unit 7: Keyboard shortcuts & indicator** — F8/F9 + recording dot (depends on Unit 2)
8. **Unit 6: Dock recording section** — UI controls + library (depends on Unit 2)

Units 2 and 5 can be built in parallel. Units 4, 6, and 7 can be built in parallel once Units 2-3 are done.

**Minimum viable demo:** Units 1 + 2 + 8 + 7 give keyboard-driven recording (F8 to record, F9 to mark) with SQLite storage. Units 3 + 4 + 5 add agent-accessible recording via MCP. Unit 6 adds dock controls.

---

## Testing

### Unit Tests: `crates/spectator-godot/src/recorder.rs`

Testing the recorder is challenging because it depends on Godot's runtime. Focus on testing the non-Godot parts:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_entity_data_roundtrips_msgpack() {
        let entity = FrameEntityData {
            path: "enemies/scout_02".into(),
            class: "CharacterBody3D".into(),
            position: vec![12.4, 0.0, -8.2],
            rotation_deg: vec![0.0, 135.0, 0.0],
            velocity: vec![1.2, 0.0, -0.8],
            groups: vec!["enemies".into()],
            visible: true,
            state: serde_json::Map::new(),
        };
        let packed = rmp_serde::to_vec(&entity).unwrap();
        let unpacked: FrameEntityData = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(unpacked.path, "enemies/scout_02");
        assert_eq!(unpacked.position, vec![12.4, 0.0, -8.2]);
    }

    #[test]
    fn schema_sql_is_valid() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();
        // Verify tables exist
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 4); // recording, frames, events, markers
    }

    #[test]
    fn schema_indexes_created() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 4);
    }

    #[test]
    fn frame_data_insert_and_read() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();

        let entities = vec![FrameEntityData {
            path: "test/node".into(),
            class: "Node3D".into(),
            position: vec![1.0, 2.0, 3.0],
            rotation_deg: vec![0.0, 90.0, 0.0],
            velocity: vec![0.0, 0.0, 0.0],
            groups: vec![],
            visible: true,
            state: serde_json::Map::new(),
        }];
        let data = rmp_serde::to_vec(&entities).unwrap();

        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![100u64, 1667u64, &data],
        ).unwrap();

        let read_data: Vec<u8> = db.query_row(
            "SELECT data FROM frames WHERE frame = 100",
            [],
            |r| r.get(0),
        ).unwrap();

        let read_entities: Vec<FrameEntityData> = rmp_serde::from_slice(&read_data).unwrap();
        assert_eq!(read_entities.len(), 1);
        assert_eq!(read_entities[0].path, "test/node");
    }

    #[test]
    fn markers_insert_and_query() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();

        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![100u64, 1667u64, "human", "bug here"],
        ).unwrap();
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![200u64, 3334u64, "agent", "root cause"],
        ).unwrap();

        let mut stmt = db.prepare("SELECT source, label FROM markers ORDER BY frame").unwrap();
        let markers: Vec<(String, String)> = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?))
        }).unwrap().flatten().collect();

        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0], ("human".into(), "bug here".into()));
        assert_eq!(markers[1], ("agent".into(), "root cause".into()));
    }

    #[test]
    fn msgpack_size_is_compact() {
        // Verify MessagePack is meaningfully smaller than JSON
        let entities: Vec<FrameEntityData> = (0..50).map(|i| FrameEntityData {
            path: format!("enemies/scout_{i:02}"),
            class: "CharacterBody3D".into(),
            position: vec![i as f64 * 2.0, 0.0, i as f64 * -1.5],
            rotation_deg: vec![0.0, (i * 45) as f64, 0.0],
            velocity: vec![1.0, 0.0, -0.5],
            groups: vec!["enemies".into()],
            visible: true,
            state: {
                let mut m = serde_json::Map::new();
                m.insert("health".into(), serde_json::Value::from(100 - i));
                m
            },
        }).collect();

        let msgpack = rmp_serde::to_vec(&entities).unwrap();
        let json = serde_json::to_vec(&entities).unwrap();

        assert!(msgpack.len() < json.len(), "MessagePack should be smaller than JSON");
        // Typically 40-60% of JSON size
        let ratio = msgpack.len() as f64 / json.len() as f64;
        assert!(ratio < 0.7, "Expected >30% reduction, got {:.0}% reduction", (1.0 - ratio) * 100.0);
    }
}
```

### Integration Tests: `crates/spectator-server/src/mcp/recording.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_params_deserializes() {
        let json = serde_json::json!({
            "action": "start",
            "recording_name": "test_rec",
            "capture": {
                "capture_interval": 2,
                "max_frames": 1000,
            }
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "start");
        assert_eq!(params.recording_name.as_deref(), Some("test_rec"));
        assert_eq!(params.capture.as_ref().unwrap().capture_interval, Some(2));
    }

    #[test]
    fn recording_params_minimal_start() {
        let json = serde_json::json!({ "action": "start" });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "start");
        assert!(params.recording_name.is_none());
        assert!(params.capture.is_none());
    }

    #[test]
    fn recording_params_add_marker() {
        let json = serde_json::json!({
            "action": "add_marker",
            "marker_label": "bug here",
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "add_marker");
        assert_eq!(params.marker_label.as_deref(), Some("bug here"));
    }
}
```

---

## Edge Cases & Design Decisions

### Game crash during recording
- Frames are flushed to SQLite every 60 captured frames (~1 second at 60fps/interval=1).
- On crash, frames since the last flush are lost.
- The SQLite WAL journal ensures the database is not corrupted.
- The `ended_at_frame` field in the `recording` table is NULL for incomplete recordings.
- `list_recordings` includes incomplete recordings (they have frames up to the last flush).

### Recording without agent connected
- Human can start/stop via F8 and dock buttons without any MCP server connected.
- The recorder operates entirely within the GDExtension + GDScript layer.
- Recordings are stored to disk and available when the agent connects later.

### Multiple game sessions
- Each recording is a separate `.sqlite` file identified by `recording_id`.
- Stopping and restarting the game does not affect saved recordings.
- The storage directory persists across sessions.

### Performance budget
- Frame capture target: <1ms (MessagePack serialization of 50 entities ~0.1ms, SQLite batch write ~2ms but only every 60 frames).
- The collector's `collect_snapshot()` is the most expensive part (~2-5ms for 100 nodes). This is already within the per-frame budget specified in SPEC.md.

### Storage path resolution
- `user://spectator_recordings/` is a Godot path. The GDExtension resolves it via `ProjectSettings::globalize_path()` to get an absolute filesystem path.
- The MCP server does not need to resolve Godot paths — all recording operations go through TCP queries to the addon.
- For M8 (analysis), the server will need the absolute path. The `recording_list` response could include it, or the server can query the addon for the resolved path.

### Marker sources
- **Human**: F9 key or dock marker button. Source = `"human"`.
- **Agent**: `recording(action: "add_marker")`. Source = `"agent"`.
- **System**: Velocity spike detection, anomaly markers. Deferred to M8 (requires frame analysis that doesn't exist yet).

### Recording naming
- If no name is provided, the recorder generates one from the current timestamp.
- The `chrono_like_timestamp()` function uses Unix seconds. A human-readable format like `recording_2026-03-06_14-30` would require a time library. For M7, the seconds-based name is sufficient; it can be improved in M11 (polish).

---

## Verification Checklist

```bash
# Build
cargo build --workspace
cargo clippy --workspace
cargo test --workspace

# Verify GDExtension loads
cp target/debug/libspectator_godot.so addons/spectator/bin/linux/

# Manual testing in Godot:
# 1. Enable addon → no errors
# 2. Play game → F8 starts recording (red dot visible)
# 3. F9 drops marker (yellow flash)
# 4. F8 stops recording (red dot gone)
# 5. Dock shows recording in library
# 6. Delete button removes recording
# 7. Agent calls recording(action: "list") → sees recording
# 8. Agent calls recording(action: "start") → starts new recording
# 9. Agent calls recording(action: "add_marker", marker_label: "test")
# 10. Agent calls recording(action: "stop") → metadata returned
# 11. Agent calls recording(action: "markers") → sees human + agent markers
# 12. Stop game while recording → recording auto-stops cleanly
# 13. Kill game process while recording → partial recording recoverable
```
