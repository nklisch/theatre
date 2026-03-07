use godot::obj::Gd;
use godot::prelude::*;
use rusqlite::Connection;
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};
use spectator_protocol::recording::FrameEntityData;

use crate::collector::SpectatorCollector;

// ---------------------------------------------------------------------------
// In-memory buffer types
// ---------------------------------------------------------------------------

struct CapturedFrame {
    frame: u64,
    timestamp_ms: u64,
    data: Vec<u8>, // MessagePack-encoded Vec<FrameEntityData>
}

struct CapturedEvent {
    frame: u64,
    event_type: String,
    node_path: String,
    data: String, // JSON
}

struct CapturedMarker {
    frame: u64,
    timestamp_ms: u64,
    source: String, // "human", "agent", "system"
    label: String,
}

// FrameEntityData is defined in spectator-protocol and imported above.

// ---------------------------------------------------------------------------
// SpectatorRecorder GDExtension class
// ---------------------------------------------------------------------------

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
    frame_counter: u32,

    // Capture config
    capture_interval: u32,
    max_frames: u32,

    // Buffers (flushed to SQLite periodically)
    frame_buffer: Vec<CapturedFrame>,
    event_buffer: Vec<CapturedEvent>,
    marker_buffer: Vec<CapturedMarker>,
    flush_counter: u32,

    // SQLite connection (open during recording)
    db: Option<Connection>,
    storage_path: String,

    // Collector reference for snapshot data
    collector: Option<Gd<SpectatorCollector>>,
}

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

        if !self.frame_counter.is_multiple_of(self.capture_interval) {
            return;
        }

        if self.frames_captured >= self.max_frames {
            self.stop_recording();
            tracing::warn!("Recording stopped: max_frames ({}) reached", self.max_frames);
            return;
        }

        self.capture_frame();

        self.flush_counter += 1;
        if self.flush_counter >= 60 {
            self.flush_to_db();
            self.flush_counter = 0;
        }
    }
}

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
        if !self.recording {
            return 0;
        }
        let now_ms = current_time_ms();
        now_ms.saturating_sub(self.started_at_ms)
    }

    #[func]
    pub fn get_buffer_size_kb(&self) -> u32 {
        let bytes: usize = self.frame_buffer.iter().map(|f| f.data.len()).sum();
        (bytes / 1024) as u32
    }

    /// Start a new recording. Returns the recording_id, or empty string on error.
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
        let dir_path = globalize_path(&storage);
        let _ = std::fs::create_dir_all(&dir_path);

        let db_path = format!("{}/{}.sqlite", dir_path, recording_id);
        if let Err(e) = self.create_db(&db_path) {
            tracing::error!("Failed to create recording database: {e}");
            return GString::new();
        }

        let now_ms = current_time_ms();
        let now_frame = current_physics_frame();

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
                    now_frame,
                    now_ms,
                    config_json.to_string(),
                ],
            );
        }

        self.recording = true;
        self.recording_id = recording_id.clone();
        self.recording_name = recording_name.clone();
        self.started_at_frame = now_frame;
        self.started_at_ms = now_ms;
        self.frames_captured = 0;
        self.frame_counter = 0;
        self.flush_counter = 0;
        self.capture_interval = capture_interval.max(1);
        self.max_frames = max_frames;
        self.storage_path = storage;

        let id_var = GString::from(&self.recording_id).to_variant();
        let name_var = GString::from(&self.recording_name).to_variant();
        self.base_mut().emit_signal("recording_started", &[id_var, name_var]);

        GString::from(&recording_id)
    }

    /// Stop the active recording. Returns metadata dict, or empty dict if not recording.
    #[func]
    pub fn stop_recording(&mut self) -> VarDictionary {
        if !self.recording {
            return VarDictionary::new();
        }

        self.flush_to_db();

        let ended_frame = current_physics_frame();
        let ended_ms = current_time_ms();
        if let Some(ref db) = self.db {
            let _ = db.execute(
                "UPDATE recording SET ended_at_frame = ?1, ended_at_ms = ?2 WHERE id = ?3",
                rusqlite::params![ended_frame, ended_ms, &self.recording_id],
            );
        }

        self.db = None;

        let mut result = VarDictionary::new();
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

    /// Add a marker at the current frame.
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

    /// List all recordings in the given storage path.
    #[func]
    pub fn list_recordings(&self, storage_path: GString) -> Array<VarDictionary> {
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
            )
                && let Ok(mut stmt) = db.prepare(
                    "SELECT id, name, started_at_frame, ended_at_frame, \
                     started_at_ms, ended_at_ms FROM recording LIMIT 1",
                ) {
                    let row_result = stmt.query_row([], |row| {
                        let id: String = row.get(0)?;
                        let name: String = row.get(1)?;
                        let start_frame: i64 = row.get(2)?;
                        let end_frame: Option<i64> = row.get(3)?;
                        let start_ms: i64 = row.get(4)?;
                        let end_ms: Option<i64> = row.get(5)?;
                        Ok((id, name, start_frame, end_frame, start_ms, end_ms))
                    });

                    if let Ok((id, name, start_frame, end_frame, start_ms, end_ms)) = row_result {
                        let frame_count: i64 = db
                            .query_row("SELECT COUNT(*) FROM frames", [], |r| r.get(0))
                            .unwrap_or(0);

                        let marker_count: i64 = db
                            .query_row("SELECT COUNT(*) FROM markers", [], |r| r.get(0))
                            .unwrap_or(0);

                        let duration_ms = end_ms.unwrap_or(start_ms) - start_ms;

                        let size_kb = std::fs::metadata(&path)
                            .map(|m| m.len() / 1024)
                            .unwrap_or(0);

                        let mut dict = VarDictionary::new();
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

        result
    }

    /// Delete the recording file for the given recording_id. Returns true on success.
    #[func]
    pub fn delete_recording(&self, storage_path: GString, recording_id: GString) -> bool {
        let dir_path = globalize_path(&storage_path.to_string());
        let file_path = format!("{}/{}.sqlite", dir_path, recording_id);
        std::fs::remove_file(&file_path).is_ok()
    }

    /// Return current recording status as a dictionary.
    #[func]
    pub fn get_recording_status(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();
        dict.set("recording_active", self.recording);
        dict.set("recording_id", GString::from(&self.recording_id));
        dict.set("name", GString::from(&self.recording_name));
        dict.set("frames_captured", self.frames_captured);
        dict.set("duration_ms", self.get_elapsed_ms());
        dict.set("buffer_size_kb", self.get_buffer_size_kb());
        dict
    }

    /// Return all markers for a recording by reading its SQLite file.
    #[func]
    pub fn get_recording_markers(
        &self,
        storage_path: GString,
        recording_id: GString,
    ) -> Array<VarDictionary> {
        let dir_path = globalize_path(&storage_path.to_string());
        let file_path = format!("{}/{}.sqlite", dir_path, recording_id);
        let mut result = Array::new();

        let db = match Connection::open_with_flags(
            &file_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(db) => db,
            Err(_) => return result,
        };

        let mut stmt = match db.prepare(
            "SELECT frame, timestamp_ms, source, label FROM markers ORDER BY frame",
        ) {
            Ok(s) => s,
            Err(_) => return result,
        };

        let rows = stmt.query_map([], |row| {
            let frame: i64 = row.get(0)?;
            let timestamp_ms: i64 = row.get(1)?;
            let source: String = row.get(2)?;
            let label: String = row.get(3)?;
            Ok((frame, timestamp_ms, source, label))
        });

        if let Ok(rows) = rows {
            for row in rows.flatten() {
                let (frame, timestamp_ms, source, label) = row;
                let mut dict = VarDictionary::new();
                dict.set("frame", frame);
                dict.set("timestamp_ms", timestamp_ms);
                dict.set("source", GString::from(&source));
                dict.set("label", GString::from(&label));
                result.push(&dict);
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Internal implementation
// ---------------------------------------------------------------------------

impl SpectatorRecorder {
    fn capture_frame(&mut self) {
        let Some(ref collector) = self.collector else {
            return;
        };

        let params = GetSnapshotDataParams {
            perspective: PerspectiveParam::Camera,
            radius: f64::MAX,
            include_offscreen: true,
            groups: vec![],
            class_filter: vec![],
            detail: DetailLevel::Standard,
            expose_internals: false,
        };

        let snapshot = collector.bind().collect_snapshot(&params);

        let frame_entities: Vec<FrameEntityData> = snapshot
            .entities
            .iter()
            .map(|e| FrameEntityData {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position.clone(),
                rotation_deg: e.rotation_deg.clone(),
                velocity: e.velocity.clone(),
                groups: e.groups.clone(),
                visible: e.visible,
                state: e.state.clone(),
            })
            .collect();

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

    fn flush_to_db(&mut self) {
        let Some(ref db) = self.db else {
            return;
        };

        if self.frame_buffer.is_empty() && self.event_buffer.is_empty() && self.marker_buffer.is_empty() {
            return;
        }

        let tx = match db.unchecked_transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("SQLite transaction error: {e}");
                return;
            }
        };

        {
            let mut stmt = match tx.prepare_cached(
                "INSERT OR REPLACE INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("SQLite prepare error: {e}");
                    return;
                }
            };
            for f in &self.frame_buffer {
                let _ = stmt.execute(rusqlite::params![f.frame, f.timestamp_ms, &f.data]);
            }
        }

        {
            let mut stmt = match tx.prepare_cached(
                "INSERT INTO events (frame, event_type, node_path, data) VALUES (?1, ?2, ?3, ?4)",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("SQLite prepare error: {e}");
                    return;
                }
            };
            for ev in &self.event_buffer {
                let _ = stmt
                    .execute(rusqlite::params![ev.frame, &ev.event_type, &ev.node_path, &ev.data]);
            }
        }

        {
            let mut stmt = match tx.prepare_cached(
                "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("SQLite prepare error: {e}");
                    return;
                }
            };
            for m in &self.marker_buffer {
                let _ = stmt
                    .execute(rusqlite::params![m.frame, m.timestamp_ms, &m.source, &m.label]);
            }
        }

        if let Err(e) = tx.commit() {
            tracing::error!("SQLite commit error: {e}");
        }

        self.frame_buffer.clear();
        self.event_buffer.clear();
        self.marker_buffer.clear();
    }

    fn create_db(&mut self, path: &str) -> Result<(), String> {
        let db = Connection::open(path).map_err(|e| format!("SQLite open error: {e}"))?;
        db.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("WAL error: {e}"))?;
        db.execute_batch(SCHEMA_SQL)
            .map_err(|e| format!("Schema error: {e}"))?;
        self.db = Some(db);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SQLite schema
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn current_physics_frame() -> u64 {
    godot::classes::Engine::singleton().get_physics_frames()
}

pub(crate) fn globalize_path(godot_path: &str) -> String {
    godot::classes::ProjectSettings::singleton()
        .globalize_path(godot_path)
        .to_string()
}

fn rand_u32() -> u32 {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (t.as_nanos() & 0xFFFF_FFFF) as u32
}

fn chrono_like_timestamp() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        // Exclude sqlite_sequence (created automatically by AUTOINCREMENT)
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 4);
    }

    #[test]
    fn schema_indexes_created() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
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
        )
        .unwrap();

        let read_data: Vec<u8> = db
            .query_row("SELECT data FROM frames WHERE frame = 100", [], |r| r.get(0))
            .unwrap();

        let read_entities: Vec<FrameEntityData> = rmp_serde::from_slice(&read_data).unwrap();
        assert_eq!(read_entities.len(), 1);
        assert_eq!(read_entities[0].path, "test/node");
    }

    #[test]
    fn markers_insert_and_query() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();

        // Insert parent frames first (FK constraint requires frames to exist)
        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![100u64, 1667u64, &[] as &[u8]],
        )
        .unwrap();
        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![200u64, 3334u64, &[] as &[u8]],
        )
        .unwrap();

        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![100u64, 1667u64, "human", "bug here"],
        )
        .unwrap();
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![200u64, 3334u64, "agent", "root cause"],
        )
        .unwrap();

        let mut stmt = db
            .prepare("SELECT source, label FROM markers ORDER BY frame")
            .unwrap();
        let markers: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .flatten()
            .collect();

        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0], ("human".into(), "bug here".into()));
        assert_eq!(markers[1], ("agent".into(), "root cause".into()));
    }

    #[test]
    fn msgpack_size_is_compact() {
        let entities: Vec<FrameEntityData> = (0..50)
            .map(|i| FrameEntityData {
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
            })
            .collect();

        let msgpack = rmp_serde::to_vec(&entities).unwrap();
        let json = serde_json::to_vec(&entities).unwrap();

        assert!(msgpack.len() < json.len(), "MessagePack should be smaller than JSON");
        let ratio = msgpack.len() as f64 / json.len() as f64;
        assert!(
            ratio < 0.7,
            "Expected >30% reduction, got {:.0}% reduction",
            (1.0 - ratio) * 100.0
        );
    }
}
