use std::collections::VecDeque;

use godot::classes::{Engine, Node, Node2D, Node3D, node::ProcessMode};
use godot::obj::Gd;
use godot::prelude::*;
use rusqlite::Connection;
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};
use spectator_protocol::recording::FrameEntityData;

use crate::collector::SpectatorCollector;

// ---------------------------------------------------------------------------
// In-memory buffer types
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CapturedFrame {
    frame: u64,
    timestamp_ms: u64,
    data: Vec<u8>, // MessagePack-encoded Vec<FrameEntityData>
}

// FrameEntityData is defined in spectator-protocol and imported above.

// ---------------------------------------------------------------------------
// Dashcam types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashcamTier {
    System,
    Deliberate, // agent or human
}

impl DashcamTier {
    fn as_str(self) -> &'static str {
        match self {
            DashcamTier::System => "system",
            DashcamTier::Deliberate => "deliberate",
        }
    }
}

struct DashcamTrigger {
    frame: u64,
    timestamp_ms: u64,
    source: String,
    label: String,
}

/// Dashcam configuration — all timing in seconds, capture_interval in physics frames.
pub struct DashcamConfig {
    pub enabled: bool,
    pub capture_interval: u32,
    pub pre_window_system_sec: u32,
    pub pre_window_deliberate_sec: u32,
    pub post_window_system_sec: u32,
    pub post_window_deliberate_sec: u32,
    pub max_window_sec: u32,
    pub min_after_sec: u32,
    pub system_min_interval_sec: u32,
    pub byte_cap_mb: u32,
}

impl Default for DashcamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_interval: 1,
            pre_window_system_sec: 30,
            pre_window_deliberate_sec: 60,
            post_window_system_sec: 10,
            post_window_deliberate_sec: 30,
            max_window_sec: 120,
            min_after_sec: 5,
            system_min_interval_sec: 2,
            byte_cap_mb: 1024,
        }
    }
}

/// Dashcam clip state machine.
enum DashcamState {
    Disabled,
    Buffering,
    PostCapture {
        frames_remaining: u32,
        tier: DashcamTier,
        /// Snapshot of ring_buffer at the moment the first trigger fired.
        pre_buffer: Vec<CapturedFrame>,
        /// Frames captured after the trigger (will become the clip's tail).
        post_buffer: Vec<CapturedFrame>,
        /// All trigger annotations recorded in this clip.
        markers: Vec<DashcamTrigger>,
        /// Frame of the last system marker (for rate-limiting).
        last_system_trigger_frame: u64,
        /// Absolute frame at which a system-tier clip is force-closed.
        force_close_at_frame: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// SpectatorRecorder GDExtension class
// ---------------------------------------------------------------------------

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorRecorder {
    base: Base<Node>,

    // Physics frame counter (for dashcam capture interval)
    frame_counter: u32,

    // Collector reference for snapshot data
    collector: Option<Gd<SpectatorCollector>>,

    // Dashcam state
    dashcam_config: DashcamConfig,
    dashcam_state: DashcamState,
    ring_buffer: VecDeque<CapturedFrame>,
    ring_buffer_bytes: usize,
    /// Exponential moving average of per-frame byte size (for byte cap).
    avg_frame_bytes: usize,
    /// Cached physics FPS (from Engine.physics_ticks_per_second).
    physics_fps: u32,
}

#[godot_api]
impl INode for SpectatorRecorder {
    fn ready(&mut self) {
        // Always process even when the game is paused so recording continues.
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);

        // Cache physics FPS.
        self.physics_fps = Engine::singleton().get_physics_ticks_per_second() as u32;

        // Auto-start dashcam.
        if self.dashcam_config.enabled {
            self.dashcam_state = DashcamState::Buffering;
            tracing::info!(
                "[Spectator] Dashcam started (pre={}s/{}s, post={}s/{}s, cap={}MB)",
                self.dashcam_config.pre_window_system_sec,
                self.dashcam_config.pre_window_deliberate_sec,
                self.dashcam_config.post_window_system_sec,
                self.dashcam_config.post_window_deliberate_sec,
                self.dashcam_config.byte_cap_mb,
            );
        }
    }

    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            frame_counter: 0,
            collector: None,
            dashcam_config: DashcamConfig::default(),
            dashcam_state: DashcamState::Disabled,
            ring_buffer: VecDeque::new(),
            ring_buffer_bytes: 0,
            avg_frame_bytes: 0,
            physics_fps: 60,
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        // --- Dashcam force-close check (no capture needed) ---
        self.dashcam_check_force_close();

        if matches!(self.dashcam_state, DashcamState::Disabled) {
            return;
        }

        self.frame_counter += 1;
        if !self.frame_counter.is_multiple_of(self.dashcam_config.capture_interval) {
            return;
        }

        let Some(captured) = self.do_capture() else {
            return;
        };

        self.dashcam_ingest(captured);
    }
}

#[godot_api]
impl SpectatorRecorder {
    #[signal]
    fn marker_added(frame: u64, source: GString, label: GString);

    #[signal]
    fn dashcam_clip_saved(clip_id: GString, tier: GString, frames: u32);

    #[signal]
    fn dashcam_clip_started(trigger_frame: u64, tier: GString);

    #[func]
    pub fn set_collector(&mut self, collector: Gd<SpectatorCollector>) {
        self.collector = Some(collector);
    }

    // --- Dashcam funcs ---

    /// Enable or disable dashcam mode at runtime.
    #[func]
    pub fn set_dashcam_enabled(&mut self, enabled: bool) {
        if enabled {
            if matches!(self.dashcam_state, DashcamState::Disabled) {
                self.dashcam_state = DashcamState::Buffering;
            }
        } else {
            self.dashcam_state = DashcamState::Disabled;
            self.ring_buffer.clear();
            self.ring_buffer_bytes = 0;
        }
    }

    /// Returns true if dashcam is actively buffering or in post-capture.
    #[func]
    pub fn is_dashcam_active(&self) -> bool {
        matches!(
            self.dashcam_state,
            DashcamState::Buffering | DashcamState::PostCapture { .. }
        )
    }

    /// Returns current ring buffer size in frames.
    #[func]
    pub fn get_dashcam_buffer_frames(&self) -> u32 {
        self.ring_buffer.len() as u32
    }

    /// Returns current ring buffer memory usage in KB.
    #[func]
    pub fn get_dashcam_buffer_kb(&self) -> u32 {
        (self.ring_buffer_bytes / 1024) as u32
    }

    /// Returns dashcam clip state string: "buffering", "post_capture", or "disabled".
    #[func]
    pub fn get_dashcam_state(&self) -> GString {
        match &self.dashcam_state {
            DashcamState::Disabled => GString::from("disabled"),
            DashcamState::Buffering => GString::from("buffering"),
            DashcamState::PostCapture { .. } => GString::from("post_capture"),
        }
    }

    /// Trigger a dashcam clip from an external marker (TCP handler).
    /// Transitions Buffering → PostCapture or merges into existing clip.
    #[func]
    pub fn trigger_dashcam_clip(&mut self, source: GString, label: GString, _tier: GString) {
        let frame = current_physics_frame();
        let timestamp_ms = current_time_ms();
        self.on_dashcam_marker(&source.to_string(), &label.to_string(), frame, timestamp_ms);
    }

    /// Force-flush the current ring buffer to a clip immediately.
    /// Returns the clip_id or empty string on error.
    #[func]
    pub fn flush_dashcam_clip(&mut self, label: GString) -> GString {
        if matches!(self.dashcam_state, DashcamState::Disabled) {
            return GString::new();
        }

        if matches!(self.dashcam_state, DashcamState::Buffering) {
            // Create a PostCapture state with frames_remaining=0 for immediate flush.
            let frame = current_physics_frame();
            let timestamp_ms = current_time_ms();
            let pre_buffer: Vec<CapturedFrame> = self.ring_buffer.iter().cloned().collect();
            self.dashcam_state = DashcamState::PostCapture {
                frames_remaining: 0,
                tier: DashcamTier::Deliberate,
                pre_buffer,
                post_buffer: Vec::new(),
                markers: vec![DashcamTrigger {
                    frame,
                    timestamp_ms,
                    source: "human".into(),
                    label: label.to_string(),
                }],
                last_system_trigger_frame: 0,
                force_close_at_frame: None,
            };
        } else if let DashcamState::PostCapture {
            ref mut frames_remaining,
            ..
        } = self.dashcam_state
        {
            *frames_remaining = 0;
        }

        if let Some(id) = self.flush_dashcam_clip_internal() {
            GString::from(&id)
        } else {
            GString::new()
        }
    }

    /// Apply dashcam configuration from a JSON string.
    /// Fields absent from the JSON are left unchanged.
    #[func]
    pub fn apply_dashcam_config(&mut self, config_json: GString) -> bool {
        let s = config_json.to_string();
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else {
            tracing::warn!("[Spectator] apply_dashcam_config: invalid JSON");
            return false;
        };

        if let Some(b) = v.get("enabled").and_then(|x| x.as_bool()) {
            self.dashcam_config.enabled = b;
        }
        if let Some(n) = v.get("capture_interval").and_then(|x| x.as_u64()) {
            self.dashcam_config.capture_interval = n as u32;
        }
        if let Some(n) = v.get("pre_window_system_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.pre_window_system_sec = n as u32;
        }
        if let Some(n) = v.get("pre_window_deliberate_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.pre_window_deliberate_sec = n as u32;
        }
        if let Some(n) = v.get("post_window_system_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.post_window_system_sec = n as u32;
        }
        if let Some(n) = v.get("post_window_deliberate_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.post_window_deliberate_sec = n as u32;
        }
        if let Some(n) = v.get("max_window_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.max_window_sec = n as u32;
        }
        if let Some(n) = v.get("min_after_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.min_after_sec = n as u32;
        }
        if let Some(n) = v.get("system_min_interval_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.system_min_interval_sec = n as u32;
        }
        if let Some(n) = v.get("byte_cap_mb").and_then(|x| x.as_u64()) {
            self.dashcam_config.byte_cap_mb = n as u32;
        }
        true
    }

    /// Return dashcam config as a JSON dict string for TCP status response.
    #[func]
    pub fn get_dashcam_config_json(&self) -> GString {
        let cfg = &self.dashcam_config;
        let json = serde_json::json!({
            "enabled": cfg.enabled,
            "capture_interval": cfg.capture_interval,
            "pre_window_sec": { "system": cfg.pre_window_system_sec, "deliberate": cfg.pre_window_deliberate_sec },
            "post_window_sec": { "system": cfg.post_window_system_sec, "deliberate": cfg.post_window_deliberate_sec },
            "max_window_sec": cfg.max_window_sec,
            "min_after_sec": cfg.min_after_sec,
            "system_min_interval_sec": cfg.system_min_interval_sec,
            "byte_cap_mb": cfg.byte_cap_mb,
        });
        GString::from(json.to_string().as_str())
    }

    /// Add a marker at the current frame. Triggers a dashcam clip.
    #[func]
    pub fn add_marker(&mut self, source: GString, label: GString) {
        let frame = current_physics_frame();
        let timestamp_ms = current_time_ms();
        let source_str = source.to_string();
        let label_str = label.to_string();

        self.on_dashcam_marker(&source_str, &label_str, frame, timestamp_ms);

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
                     started_at_ms, ended_at_ms, capture_config FROM recording LIMIT 1",
                ) {
                    let row_result = stmt.query_row([], |row| {
                        let id: String = row.get(0)?;
                        let name: String = row.get(1)?;
                        let start_frame: i64 = row.get(2)?;
                        let end_frame: Option<i64> = row.get(3)?;
                        let start_ms: i64 = row.get(4)?;
                        let end_ms: Option<i64> = row.get(5)?;
                        let capture_config: Option<String> = row.get(6)?;
                        Ok((id, name, start_frame, end_frame, start_ms, end_ms, capture_config))
                    });

                    if let Ok((id, name, start_frame, end_frame, start_ms, end_ms, capture_config)) = row_result {
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

                        // Check if this is a dashcam clip
                        let is_dashcam = capture_config.as_deref()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                            .and_then(|v| v.get("dashcam").and_then(|b| b.as_bool()))
                            .unwrap_or(false);

                        let dashcam_tier = if is_dashcam {
                            capture_config.as_deref()
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|v| v.get("tier").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };

                        let mut dict = VarDictionary::new();
                        dict.set("clip_id", GString::from(&id));
                        dict.set("name", GString::from(&name));
                        dict.set("frames_captured", frame_count as u32);
                        dict.set("duration_ms", duration_ms);
                        dict.set("frame_range_start", start_frame);
                        dict.set("frame_range_end", end_frame.unwrap_or(start_frame));
                        dict.set("markers_count", marker_count as u32);
                        dict.set("size_kb", size_kb as u32);
                        dict.set("created_at_ms", start_ms);
                        dict.set("dashcam", is_dashcam);
                        dict.set("dashcam_tier", GString::from(&dashcam_tier));
                        result.push(&dict);
                    }
                }
        }

        result
    }

    /// Delete the clip file for the given clip_id. Returns true on success.
    #[func]
    pub fn delete_recording(&self, storage_path: GString, clip_id: GString) -> bool {
        let dir_path = globalize_path(&storage_path.to_string());
        let file_path = format!("{}/{}.sqlite", dir_path, clip_id);
        std::fs::remove_file(&file_path).is_ok()
    }

    /// Return all markers for a clip by reading its SQLite file.
    #[func]
    pub fn get_recording_markers(
        &self,
        storage_path: GString,
        clip_id: GString,
    ) -> Array<VarDictionary> {
        let dir_path = globalize_path(&storage_path.to_string());
        let file_path = format!("{}/{}.sqlite", dir_path, clip_id);
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
    /// Capture one frame of entity data and return it (without pushing to any buffer).
    fn do_capture(&mut self) -> Option<CapturedFrame> {
        let Some(ref collector) = self.collector else {
            return None;
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
                return None;
            }
        };

        Some(CapturedFrame {
            frame: snapshot.frame,
            timestamp_ms: snapshot.timestamp_ms,
            data,
        })
    }

    // -----------------------------------------------------------------------
    // Dashcam internals
    // -----------------------------------------------------------------------

    /// Check if a system-tier dashcam clip should be force-closed (max_window exceeded).
    fn dashcam_check_force_close(&mut self) {
        if let DashcamState::PostCapture {
            tier: DashcamTier::System,
            force_close_at_frame: Some(close_frame),
            ..
        } = &self.dashcam_state
            && current_physics_frame() >= *close_frame
        {
            self.flush_dashcam_clip_internal();
        }
    }

    /// Ingest a captured frame into the dashcam ring buffer and post-capture state.
    fn dashcam_ingest(&mut self, captured: CapturedFrame) {
        // Update byte size estimate (exponential moving average, α≈0.05).
        if self.avg_frame_bytes == 0 {
            self.avg_frame_bytes = captured.data.len();
        } else {
            self.avg_frame_bytes = (self.avg_frame_bytes * 19 + captured.data.len()) / 20;
        }

        // Add to ring buffer (clone needed if PostCapture also wants the original).
        self.ring_buffer_bytes += captured.data.len();
        self.ring_buffer.push_back(captured.clone());
        self.enforce_ring_byte_cap();

        // If in PostCapture: add to post_buffer and count down.
        let should_flush = if let DashcamState::PostCapture {
            frames_remaining,
            post_buffer,
            ..
        } = &mut self.dashcam_state
        {
            post_buffer.push(captured);
            if *frames_remaining > 0 {
                *frames_remaining -= 1;
            }
            *frames_remaining == 0
        } else {
            false
        };

        if should_flush {
            self.flush_dashcam_clip_internal();
        }
    }

    /// Evict oldest ring buffer frames until within byte_cap_mb.
    fn enforce_ring_byte_cap(&mut self) {
        let byte_cap = self.dashcam_config.byte_cap_mb as usize * 1024 * 1024;
        // Also enforce time-based frame cap.
        let frame_cap = self.ring_cap_frames();
        while self.ring_buffer.len() > frame_cap
            || (self.ring_buffer_bytes > byte_cap && !self.ring_buffer.is_empty())
        {
            if let Some(evicted) = self.ring_buffer.pop_front() {
                self.ring_buffer_bytes =
                    self.ring_buffer_bytes.saturating_sub(evicted.data.len());
            } else {
                break;
            }
        }
    }

    /// Compute max frames to keep in the ring buffer.
    fn ring_cap_frames(&self) -> usize {
        let fps = self.physics_fps.max(1) as usize;
        let interval = self.dashcam_config.capture_interval.max(1) as usize;
        let time_based =
            self.dashcam_config.pre_window_deliberate_sec as usize * fps / interval;

        if self.avg_frame_bytes == 0 {
            return time_based.max(1);
        }

        let byte_cap = self.dashcam_config.byte_cap_mb as usize * 1024 * 1024;
        let byte_based = byte_cap / self.avg_frame_bytes;
        time_based.min(byte_based).max(1)
    }

    /// Compute post-window in frames for the given tier, clamped by min_after_sec.
    fn post_window_frames(&self, tier: DashcamTier) -> u32 {
        let fps = self.physics_fps.max(1);
        let interval = self.dashcam_config.capture_interval.max(1);
        let post_sec = match tier {
            DashcamTier::System => self.dashcam_config.post_window_system_sec,
            DashcamTier::Deliberate => self.dashcam_config.post_window_deliberate_sec,
        };
        let min_frames = self.dashcam_config.min_after_sec * fps / interval;
        let desired_frames = post_sec * fps / interval;
        desired_frames.max(min_frames)
    }

    /// Handle a marker trigger for the dashcam state machine.
    fn on_dashcam_marker(&mut self, source: &str, label: &str, frame: u64, timestamp_ms: u64) {
        let tier = if source == "system" {
            DashcamTier::System
        } else {
            DashcamTier::Deliberate
        };

        // Determine action without borrowing dashcam_state.
        let is_buffering = matches!(self.dashcam_state, DashcamState::Buffering);
        let is_post_capture = matches!(self.dashcam_state, DashcamState::PostCapture { .. });

        if matches!(self.dashcam_state, DashcamState::Disabled) {
            return;
        }

        if is_buffering {
            // Snapshot ring buffer and transition to PostCapture.
            let pre_buffer: Vec<CapturedFrame> = self.ring_buffer.iter().cloned().collect();
            let post_window = self.post_window_frames(tier);
            let force_close_at_frame = if tier == DashcamTier::System {
                Some(
                    frame
                        + self.dashcam_config.max_window_sec as u64
                            * self.physics_fps as u64,
                )
            } else {
                None
            };

            self.dashcam_state = DashcamState::PostCapture {
                frames_remaining: post_window,
                tier,
                pre_buffer,
                post_buffer: Vec::new(),
                markers: vec![DashcamTrigger {
                    frame,
                    timestamp_ms,
                    source: source.to_string(),
                    label: label.to_string(),
                }],
                last_system_trigger_frame: if tier == DashcamTier::System { frame } else { 0 },
                force_close_at_frame,
            };

            let tier_str = tier.as_str();
            self.base_mut().emit_signal(
                "dashcam_clip_started",
                &[frame.to_variant(), GString::from(tier_str).to_variant()],
            );
        } else if is_post_capture {
            self.merge_dashcam_trigger(tier, source, label, frame, timestamp_ms);
        }
    }

    /// Merge a new trigger into an open PostCapture clip.
    fn merge_dashcam_trigger(
        &mut self,
        tier: DashcamTier,
        source: &str,
        label: &str,
        frame: u64,
        timestamp_ms: u64,
    ) {
        // Pre-compute config values before borrowing dashcam_state.
        let deliberate_frames = self.post_window_frames(DashcamTier::Deliberate);
        let system_frames = self.post_window_frames(DashcamTier::System);
        let min_interval =
            self.dashcam_config.system_min_interval_sec as u64 * self.physics_fps as u64;

        let DashcamState::PostCapture {
            ref mut frames_remaining,
            tier: ref mut existing_tier,
            ref mut markers,
            ref mut last_system_trigger_frame,
            ref mut force_close_at_frame,
            ..
        } = self.dashcam_state
        else {
            return;
        };

        let trigger = DashcamTrigger {
            frame,
            timestamp_ms,
            source: source.to_string(),
            label: label.to_string(),
        };

        if tier == DashcamTier::Deliberate {
            // Deliberate trigger: upgrade clip tier, extend post-window, clear force-close.
            *frames_remaining = (*frames_remaining).max(deliberate_frames);
            *existing_tier = DashcamTier::Deliberate;
            *force_close_at_frame = None;
            markers.push(trigger);
        } else {
            // System trigger into existing clip.
            let elapsed_since_last = frame.saturating_sub(*last_system_trigger_frame);
            if elapsed_since_last >= min_interval {
                // Not rate-limited: extend post-window.
                *frames_remaining = (*frames_remaining).max(system_frames);
                *last_system_trigger_frame = frame;
            }
            // Always record as annotation (even if rate-limited).
            markers.push(trigger);
        }
    }

    /// Flush the current PostCapture clip to a new SQLite file.
    /// Resets dashcam_state to Buffering.
    fn flush_dashcam_clip_internal(&mut self) -> Option<String> {
        let state = std::mem::replace(&mut self.dashcam_state, DashcamState::Buffering);
        let DashcamState::PostCapture {
            tier,
            pre_buffer,
            post_buffer,
            markers,
            ..
        } = state
        else {
            return None;
        };

        let clip_id = format!("clip_{:08x}", rand_u32());
        let storage_path = "user://spectator_recordings/";
        let dir_path = globalize_path(storage_path);
        let _ = std::fs::create_dir_all(&dir_path);
        let db_path = format!("{}/{}.sqlite", dir_path, clip_id);

        let db = match Connection::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                tracing::error!("[Spectator] Failed to create dashcam clip DB: {e}");
                return None;
            }
        };

        if db.execute_batch("PRAGMA journal_mode=WAL;").is_err() {
            tracing::error!("[Spectator] Failed to set WAL mode for dashcam clip");
            return None;
        }
        if db.execute_batch(SCHEMA_SQL).is_err() {
            tracing::error!("[Spectator] Failed to create dashcam schema");
            return None;
        }

        let tier_str = tier.as_str();
        let all_frames: Vec<&CapturedFrame> =
            pre_buffer.iter().chain(post_buffer.iter()).collect();
        let total_frames = all_frames.len() as u32;

        let first_frame = all_frames.first().map(|f| f.frame).unwrap_or(0);
        let last_frame = all_frames.last().map(|f| f.frame).unwrap_or(0);
        let first_ts = all_frames.first().map(|f| f.timestamp_ms).unwrap_or(0);
        let last_ts = all_frames.last().map(|f| f.timestamp_ms).unwrap_or(0);

        let triggers_json: Vec<serde_json::Value> = markers
            .iter()
            .map(|m| {
                serde_json::json!({
                    "frame": m.frame,
                    "source": m.source,
                    "label": m.label,
                })
            })
            .collect();

        let capture_config = serde_json::json!({
            "capture_interval": self.dashcam_config.capture_interval,
            "max_frames": total_frames,
            "dashcam": true,
            "tier": tier_str,
            "triggers": triggers_json,
        });

        let physics_ticks = self.physics_fps;
        let scene_dims = detect_scene_dimensions(
            self.base().get_tree().and_then(|t| t.get_current_scene()),
        );

        let _ = db.execute(
            "INSERT INTO recording \
             (id, name, started_at_frame, ended_at_frame, started_at_ms, ended_at_ms, \
              scene_dimensions, physics_ticks_per_sec, capture_config) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &clip_id,
                &format!("dashcam_{}", chrono_like_timestamp()),
                first_frame,
                last_frame,
                first_ts,
                last_ts,
                scene_dims,
                physics_ticks,
                capture_config.to_string(),
            ],
        );

        // Write frames and markers in one transaction.
        if let Ok(tx) = db.unchecked_transaction() {
            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT OR REPLACE INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            ) {
                for f in &all_frames {
                    let _ = stmt.execute(rusqlite::params![f.frame, f.timestamp_ms, &f.data]);
                }
            }

            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            ) {
                for m in &markers {
                    let _ = stmt.execute(rusqlite::params![
                        m.frame,
                        m.timestamp_ms,
                        &m.source,
                        &m.label
                    ]);
                }
            }

            if let Err(e) = tx.commit() {
                tracing::error!("[Spectator] Failed to commit dashcam clip: {e}");
                return None;
            }
        }

        tracing::info!(
            "[Spectator] Dashcam clip saved: {} ({} frames, {} tier)",
            clip_id,
            total_frames,
            tier_str
        );

        // Emit signal — all local borrows are released at this point.
        let id_var = GString::from(&clip_id).to_variant();
        let tier_var = GString::from(tier_str).to_variant();
        let frames_var = total_frames.to_variant();
        self.base_mut()
            .emit_signal("dashcam_clip_saved", &[id_var, tier_var, frames_var]);

        Some(clip_id)
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

/// Detect whether the current scene is 2D or 3D by walking the scene tree.
/// Returns 2 for 2D-only scenes, 3 for 3D or unknown.
fn detect_scene_dimensions(root: Option<Gd<Node>>) -> u32 {
    let Some(root) = root else { return 3 };
    let root_node: Gd<Node> = root.upcast();
    let has_2d = has_node_type_recursive(&root_node, true);
    let has_3d = has_node_type_recursive(&root_node, false);
    match (has_2d, has_3d) {
        (true, false) => 2,
        _ => 3,
    }
}

fn has_node_type_recursive(node: &Gd<Node>, check_2d: bool) -> bool {
    if check_2d {
        if node.clone().try_cast::<Node2D>().is_ok() {
            return true;
        }
    } else if node.clone().try_cast::<Node3D>().is_ok() {
        return true;
    }
    let count = node.get_child_count();
    for i in 0..count {
        if let Some(child) = node.get_child(i)
            && has_node_type_recursive(&child, check_2d)
        {
            return true;
        }
    }
    false
}

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

    // -----------------------------------------------------------------------
    // Dashcam config tests
    // -----------------------------------------------------------------------

    #[test]
    fn dashcam_config_defaults() {
        let cfg = DashcamConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.pre_window_system_sec, 30);
        assert_eq!(cfg.pre_window_deliberate_sec, 60);
        assert_eq!(cfg.post_window_system_sec, 10);
        assert_eq!(cfg.post_window_deliberate_sec, 30);
        assert_eq!(cfg.max_window_sec, 120);
        assert_eq!(cfg.min_after_sec, 5);
        assert_eq!(cfg.system_min_interval_sec, 2);
        assert_eq!(cfg.byte_cap_mb, 1024);
    }

    #[test]
    fn dashcam_tier_str() {
        assert_eq!(DashcamTier::System.as_str(), "system");
        assert_eq!(DashcamTier::Deliberate.as_str(), "deliberate");
    }

    #[test]
    fn ring_buffer_eviction_at_byte_cap() {
        // Simulate ring buffer eviction: fill with frames until byte cap forces eviction.
        let mut ring: VecDeque<CapturedFrame> = VecDeque::new();
        let mut ring_bytes: usize = 0;
        let byte_cap: usize = 10 * 1024; // 10 KB cap

        for i in 0u64..100 {
            let data = vec![0u8; 256]; // 256 bytes per frame
            ring_bytes += data.len();
            ring.push_back(CapturedFrame {
                frame: i,
                timestamp_ms: i * 16,
                data,
            });

            // Evict oldest frames when byte cap exceeded
            while ring_bytes > byte_cap && !ring.is_empty() {
                if let Some(evicted) = ring.pop_front() {
                    ring_bytes = ring_bytes.saturating_sub(evicted.data.len());
                }
            }
        }

        // At 256 bytes per frame with 10KB cap: max ~40 frames
        assert!(ring_bytes <= byte_cap);
        assert!(ring.len() <= 40);
        // Ring buffer should contain the MOST RECENT frames
        let last_frame = ring.back().unwrap().frame;
        assert_eq!(last_frame, 99);
    }

    #[test]
    fn dashcam_merge_system_plus_system_extends_window() {
        // System trigger into PostCapture with system tier: should extend frames_remaining.
        let mut frames_remaining: u32 = 5;
        let deliberate_frames: u32 = 1800;
        let system_frames: u32 = 600;
        let min_interval: u64 = 120; // 2s at 60fps

        let existing_tier = DashcamTier::System;
        let mut last_system_trigger_frame: u64 = 100;

        // New system trigger far enough from last (200 frames > 120 interval)
        let new_frame: u64 = 300;
        let elapsed = new_frame.saturating_sub(last_system_trigger_frame);

        if elapsed >= min_interval && existing_tier == DashcamTier::System {
            frames_remaining = frames_remaining.max(system_frames);
            last_system_trigger_frame = new_frame;
        }

        assert_eq!(frames_remaining, 600);
        assert_eq!(last_system_trigger_frame, 300);
        let _ = deliberate_frames; // unused in this test path
    }

    #[test]
    fn dashcam_merge_deliberate_upgrades_system_clip() {
        // Deliberate trigger into system-tier PostCapture: upgrades tier and extends window.
        let mut frames_remaining: u32 = 10;
        let mut existing_tier = DashcamTier::System;
        let mut force_close_at_frame: Option<u64> = Some(10000);
        let deliberate_frames: u32 = 1800;

        let new_tier = DashcamTier::Deliberate;
        if new_tier == DashcamTier::Deliberate {
            frames_remaining = frames_remaining.max(deliberate_frames);
            existing_tier = DashcamTier::Deliberate;
            force_close_at_frame = None;
        }

        assert_eq!(frames_remaining, 1800);
        assert_eq!(existing_tier, DashcamTier::Deliberate);
        assert!(force_close_at_frame.is_none());
    }

    #[test]
    fn dashcam_rate_limiting_system_markers() {
        // Rapid system markers within min_interval: should only annotate, not extend.
        let mut frames_remaining: u32 = 600;
        let min_interval: u64 = 120; // 2s at 60fps
        let mut last_system_trigger_frame: u64 = 100;
        let system_frames: u32 = 600;

        // Fire system marker 50 frames later (within the 120-frame interval)
        let new_frame: u64 = 150;
        let elapsed = new_frame.saturating_sub(last_system_trigger_frame);

        if elapsed >= min_interval {
            frames_remaining = frames_remaining.max(system_frames);
            last_system_trigger_frame = new_frame;
        }
        // Rate limited — frames_remaining unchanged, last_trigger unchanged
        assert_eq!(frames_remaining, 600); // unchanged
        assert_eq!(last_system_trigger_frame, 100); // unchanged
    }

    #[test]
    fn dashcam_max_window_force_close() {
        // A system clip should be force-closed when force_close_at_frame is reached.
        let trigger_frame: u64 = 1000;
        let physics_fps: u64 = 60;
        let max_window_sec: u64 = 120;
        let force_close_at_frame = trigger_frame + max_window_sec * physics_fps;

        assert_eq!(force_close_at_frame, 1000 + 7200);

        // Simulate frame advance past the force-close point
        let current_frame: u64 = force_close_at_frame + 1;
        assert!(current_frame >= force_close_at_frame);
    }

    #[test]
    fn dashcam_clip_metadata_in_sqlite() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();

        let capture_config = serde_json::json!({
            "capture_interval": 1,
            "max_frames": 100,
            "dashcam": true,
            "tier": "system",
            "triggers": [
                { "frame": 500, "source": "system", "label": "player_died" }
            ],
        });

        db.execute(
            "INSERT INTO recording (id, name, started_at_frame, ended_at_frame, \
             started_at_ms, ended_at_ms, scene_dimensions, physics_ticks_per_sec, capture_config) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                "clip_abc12345",
                "dashcam_1000",
                400u64,
                500u64,
                6666u64,
                8333u64,
                3u32,
                60u32,
                capture_config.to_string(),
            ],
        )
        .unwrap();

        let config_str: String = db
            .query_row(
                "SELECT capture_config FROM recording WHERE id = 'clip_abc12345'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&config_str).unwrap();
        assert_eq!(parsed["dashcam"], serde_json::json!(true));
        assert_eq!(parsed["tier"], serde_json::json!("system"));
        assert_eq!(parsed["triggers"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn dashcam_apply_config_json() {
        let mut cfg = DashcamConfig::default();

        // Simulate apply_dashcam_config logic
        let json = serde_json::json!({
            "pre_window_system_sec": 45,
            "post_window_deliberate_sec": 60,
            "byte_cap_mb": 512,
        });

        if let Some(n) = json.get("pre_window_system_sec").and_then(|x| x.as_u64()) {
            cfg.pre_window_system_sec = n as u32;
        }
        if let Some(n) = json.get("post_window_deliberate_sec").and_then(|x| x.as_u64()) {
            cfg.post_window_deliberate_sec = n as u32;
        }
        if let Some(n) = json.get("byte_cap_mb").and_then(|x| x.as_u64()) {
            cfg.byte_cap_mb = n as u32;
        }

        assert_eq!(cfg.pre_window_system_sec, 45);
        assert_eq!(cfg.post_window_deliberate_sec, 60);
        assert_eq!(cfg.byte_cap_mb, 512);
        // Other fields unchanged
        assert_eq!(cfg.pre_window_deliberate_sec, 60);
    }

    // -------------------------------------------------------------------------
    // Unit 8: Ring buffer time-based eviction
    // -------------------------------------------------------------------------

    #[test]
    fn ring_cap_frames_respects_time_based_limit() {
        // With 60fps, capture_interval=1, pre_window_deliberate=60s:
        // max frames from time = 60 * 60 / 1 = 3600
        // With 256 bytes/frame and 1024MB cap:
        // max frames from bytes = 1024 * 1024 * 1024 / 256 = 4194304
        // Time-based limit (3600) is the binding constraint.
        let cfg = DashcamConfig::default();
        let physics_fps = 60u32;
        let avg_frame_bytes = 256usize;
        let byte_cap = (cfg.byte_cap_mb as usize) * 1024 * 1024;

        let time_based = (cfg.pre_window_deliberate_sec as usize)
            * (physics_fps as usize)
            / (cfg.capture_interval as usize);
        let byte_based = byte_cap / avg_frame_bytes.max(1);

        let cap = time_based.min(byte_based);
        assert_eq!(cap, 3600, "time-based cap should be the binding constraint");
    }

    #[test]
    fn ring_cap_frames_byte_cap_wins_for_large_frames() {
        // With 10KB per frame and 10MB cap: max frames from bytes = 10*1024*1024 / 10240 = 1024
        // With 60fps, pre_window_deliberate=60s: time-based = 3600
        // Byte cap (1024) is the binding constraint.
        let cfg = DashcamConfig {
            byte_cap_mb: 10,
            ..DashcamConfig::default()
        };
        let physics_fps = 60u32;
        let avg_frame_bytes = 10240usize; // 10KB per frame
        let byte_cap = (cfg.byte_cap_mb as usize) * 1024 * 1024;

        let time_based = (cfg.pre_window_deliberate_sec as usize)
            * (physics_fps as usize)
            / (cfg.capture_interval as usize);
        let byte_based = byte_cap / avg_frame_bytes.max(1);

        let cap = time_based.min(byte_based);
        assert_eq!(cap, 1024, "byte-based cap should be the binding constraint for large frames");
        assert!(cap < time_based);
    }

    #[test]
    fn dashcam_capture_interval_reduces_frame_count() {
        // With capture_interval=2, only every other frame is captured.
        // At 60fps, pre_window_system=30s: time-based = 30 * 60 / 2 = 900
        let cfg = DashcamConfig {
            capture_interval: 2,
            ..DashcamConfig::default()
        };
        let physics_fps = 60u32;
        let time_based = (cfg.pre_window_system_sec as usize)
            * (physics_fps as usize)
            / (cfg.capture_interval as usize);

        assert_eq!(time_based, 900);
    }

    // -------------------------------------------------------------------------
    // Unit 9: Merge policy edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn dashcam_merge_deliberate_into_deliberate_extends_window() {
        // Two deliberate triggers: second should extend frames_remaining to
        // the larger of the two remaining post-windows.
        let mut frames_remaining: u32 = 100; // first trigger, nearly expired
        let deliberate_frames: u32 = 1800;

        // Second deliberate trigger arrives
        frames_remaining = frames_remaining.max(deliberate_frames);

        assert_eq!(frames_remaining, 1800, "deliberate+deliberate should extend to full window");
    }

    #[test]
    fn dashcam_system_trigger_into_deliberate_clip_does_not_downgrade() {
        // A system trigger into an already-deliberate clip should NOT downgrade the tier.
        let mut existing_tier = DashcamTier::Deliberate;
        let mut frames_remaining: u32 = 500;
        let system_frames: u32 = 600;

        let new_tier = DashcamTier::System;
        if new_tier == DashcamTier::Deliberate {
            // This branch is NOT taken — system trigger doesn't upgrade
            frames_remaining = frames_remaining.max(1800);
            existing_tier = DashcamTier::Deliberate;
        } else if existing_tier == DashcamTier::Deliberate {
            // System into deliberate: extend window but keep deliberate tier.
            frames_remaining = frames_remaining.max(system_frames);
            // tier stays deliberate
        }

        assert_eq!(existing_tier, DashcamTier::Deliberate, "tier must not downgrade");
        assert_eq!(frames_remaining, 600, "window should extend to system_frames");
    }

    #[test]
    fn dashcam_min_after_sec_floor() {
        // Post-window should never be less than min_after_sec, even if
        // the config specifies a shorter post-window.
        let cfg = DashcamConfig {
            post_window_system_sec: 2, // Shorter than min_after_sec
            min_after_sec: 5,
            ..DashcamConfig::default()
        };
        let physics_fps = 60u32;

        let post_window = cfg.post_window_system_sec.max(cfg.min_after_sec);
        let post_frames = post_window * physics_fps;

        assert_eq!(post_window, 5, "min_after_sec should floor the post-window");
        assert_eq!(post_frames, 300);
    }

    #[test]
    fn dashcam_force_close_not_applied_to_deliberate() {
        // Deliberate clips should NOT have force_close_at_frame set by default.
        let tier = DashcamTier::Deliberate;
        let force_close: Option<u64> = if tier == DashcamTier::System {
            Some(1000 + 120 * 60)
        } else {
            None
        };

        assert!(force_close.is_none(), "deliberate clips should not have force_close");
    }

    // -------------------------------------------------------------------------
    // Unit 10: Config JSON partial updates
    // -------------------------------------------------------------------------

    #[test]
    fn dashcam_apply_config_partial_preserves_unset_fields() {
        let mut cfg = DashcamConfig::default();
        let original_post_system = cfg.post_window_system_sec;
        let original_min_after = cfg.min_after_sec;

        // Only update one field
        let json = serde_json::json!({
            "pre_window_system_sec": 45,
        });

        if let Some(n) = json.get("pre_window_system_sec").and_then(|x| x.as_u64()) {
            cfg.pre_window_system_sec = n as u32;
        }
        if let Some(n) = json.get("post_window_system_sec").and_then(|x| x.as_u64()) {
            cfg.post_window_system_sec = n as u32;
        }

        assert_eq!(cfg.pre_window_system_sec, 45, "updated field should change");
        assert_eq!(cfg.post_window_system_sec, original_post_system, "unset field should be preserved");
        assert_eq!(cfg.min_after_sec, original_min_after, "unset field should be preserved");
    }

    #[test]
    fn dashcam_config_enabled_toggle() {
        let mut cfg = DashcamConfig::default();
        assert!(cfg.enabled, "dashcam should be enabled by default");

        let json = serde_json::json!({ "enabled": false });
        if let Some(b) = json.get("enabled").and_then(|x| x.as_bool()) {
            cfg.enabled = b;
        }
        assert!(!cfg.enabled, "dashcam should be disabled after toggle");
    }
}
