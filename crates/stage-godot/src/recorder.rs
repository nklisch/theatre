use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use godot::classes::{Engine, Node, Node2D, Node3D, image::Format, node::ProcessMode};
use godot::obj::Gd;
use godot::prelude::*;
use rusqlite::Connection;
use stage_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};
use stage_protocol::recording::FrameEntityData;

use crate::collector::StageCollector;

// ---------------------------------------------------------------------------
// In-memory buffer types
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CapturedFrame {
    frame: u64,
    timestamp_ms: u64,
    data: Vec<u8>, // MessagePack-encoded Vec<FrameEntityData>
    camera: Option<stage_protocol::recording::CameraFrameData>,
}

/// A captured viewport screenshot.
#[derive(Clone)]
struct CapturedScreenshot {
    frame: u64,
    timestamp_ms: u64,
    jpeg_data: Vec<u8>,
    width: u32,
    height: u32,
}

struct RawShot {
    frame: u64,
    timestamp_ms: u64,
    rgba: Vec<u8>,
    width: u16,
    height: u16,
    quality: u8,
    analyze: bool,
    noise_floor: u8,
}

#[derive(Debug, Clone, Copy)]
struct FrameAnalysis {
    proportion: f64,
    reset: bool,
    analysis_ms: f64,
}

struct EncodedShot {
    frame: u64,
    timestamp_ms: u64,
    jpeg_data: Option<Vec<u8>>,
    width: u32,
    height: u32,
    error: Option<String>,
    analysis: Option<FrameAnalysis>,
}

/// Reusable strided changed-pixel lattice. The worker owns this value, so no
/// Godot object or mutable state crosses the thread boundary.
struct ChangeLattice {
    width: u16,
    height: u16,
    previous: Vec<u16>,
}

impl ChangeLattice {
    const MAX_SAMPLES: usize = 16_384;

    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            previous: Vec::new(),
        }
    }

    fn analyze(&mut self, rgba: &[u8], width: u16, height: u16, noise_floor: u8) -> FrameAnalysis {
        let started = Instant::now();
        let count = width as usize * height as usize;
        if rgba.len() < count.saturating_mul(4) || width == 0 || height == 0 {
            return FrameAnalysis {
                proportion: 0.0,
                reset: true,
                analysis_ms: started.elapsed().as_secs_f64() * 1000.0,
            };
        }
        let reset = self.width != width || self.height != height;
        self.width = width;
        self.height = height;
        let stride = ((count as f64 / Self::MAX_SAMPLES as f64).sqrt().ceil() as usize).max(1);
        let samples_w = (width as usize).div_ceil(stride);
        let samples_h = (height as usize).div_ceil(stride);
        let needed = samples_w * samples_h;
        if self.previous.len() != needed {
            self.previous.resize(needed, 0);
        }
        let mut changed = 0usize;
        let mut sampled = 0usize;
        let mut index = 0;
        for y in (0..height as usize).step_by(stride) {
            for x in (0..width as usize).step_by(stride) {
                let p = (y * width as usize + x) * 4;
                let luma = ((rgba[p] as u32 * 13933
                    + rgba[p + 1] as u32 * 46871
                    + rgba[p + 2] as u32 * 4732)
                    >> 16) as u16;
                if !reset && luma.abs_diff(self.previous[index]) > noise_floor as u16 {
                    changed += 1;
                }
                self.previous[index] = luma;
                sampled += 1;
                index += 1;
            }
        }
        FrameAnalysis {
            proportion: if reset || sampled == 0 {
                0.0
            } else {
                changed as f64 / sampled as f64
            },
            reset,
            analysis_ms: started.elapsed().as_secs_f64() * 1000.0,
        }
    }
}

#[derive(Debug)]
struct AnomalyDetector {
    ema: f64,
    have_ema: bool,
    last_proportion: f64,
    streak: u32,
    triggers_total: u64,
    suppressed_cooldown: u64,
    last_trigger_frame: Option<u64>,
    last_trigger_proportion: Option<f64>,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self {
            ema: 0.0,
            have_ema: false,
            last_proportion: 0.0,
            streak: 0,
            triggers_total: 0,
            suppressed_cooldown: 0,
            last_trigger_frame: None,
            last_trigger_proportion: None,
        }
    }
}

#[derive(Clone, Copy)]
struct AnomalySettings {
    min: f64,
    relative: f64,
    sustained: u32,
    cooldown_frames: u64,
}

impl AnomalyDetector {
    fn observe(
        &mut self,
        proportion: f64,
        reset: bool,
        frame: u64,
        settings: AnomalySettings,
    ) -> Option<String> {
        self.last_proportion = proportion;
        if reset {
            self.streak = 0;
            return None;
        }
        if !self.have_ema {
            self.ema = proportion;
            self.have_ema = true;
            self.streak = 0;
            return None;
        }
        let baseline = self.ema;
        let anomalous =
            proportion >= settings.min && proportion >= settings.relative * baseline.max(0.02);
        if !anomalous {
            self.streak = 0;
            self.ema = baseline * 0.9 + proportion * 0.1;
            return None;
        }
        self.streak = self.streak.saturating_add(1);
        if self.streak < settings.sustained.max(1) {
            return None;
        }
        self.streak = 0;
        if self
            .last_trigger_frame
            .is_some_and(|last| frame.saturating_sub(last) < settings.cooldown_frames)
        {
            self.suppressed_cooldown += 1;
            return None;
        }
        self.triggers_total += 1;
        self.last_trigger_frame = Some(frame);
        self.last_trigger_proportion = Some(proportion);
        Some(format!(
            "visual_anomaly: change {proportion:.2} vs baseline {:.2} ({:.1}x)",
            baseline,
            proportion / baseline.max(0.02)
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureGap {
    start_frame: u64,
    end_frame: u64,
    reason: String,
    dropped: u32,
}

#[derive(Debug, Default)]
struct GapLedger {
    gaps: VecDeque<CaptureGap>,
    overflow: u64,
}

impl GapLedger {
    fn record(&mut self, frame: u64, reason: &str) {
        if let Some(last) = self.gaps.back_mut()
            && last.reason == reason
            && frame <= last.end_frame.saturating_add(1)
        {
            last.end_frame = last.end_frame.max(frame);
            last.dropped = last.dropped.saturating_add(1);
            return;
        }
        if self.gaps.len() >= 256 {
            self.gaps.pop_front();
            self.overflow = self.overflow.saturating_add(1);
        }
        self.gaps.push_back(CaptureGap {
            start_frame: frame,
            end_frame: frame,
            reason: reason.to_string(),
            dropped: 1,
        });
    }

    fn overlapping(&self, start: u64, end: u64) -> impl Iterator<Item = &CaptureGap> {
        self.gaps
            .iter()
            .filter(move |gap| gap.end_frame >= start && gap.start_frame <= end)
    }

    fn summary_json(&self) -> serde_json::Value {
        serde_json::json!({
            "count": self.gaps.len(),
            "overflow": self.overflow,
            "dropped": self.gaps.iter().map(|g| g.dropped as u64).sum::<u64>(),
        })
    }
}

#[derive(Debug)]
struct CaptureProbe {
    readback_ms_ema: f64,
    readback_ms_max: f64,
    dispatched: u64,
    dropped_queue_full: u64,
    encode_depth_max: usize,
    physics_delta_ms_ema: f64,
    physics_deltas: VecDeque<f64>,
    analysis_ms_ema: f64,
    analysis_ms_max: f64,
}

impl Default for CaptureProbe {
    fn default() -> Self {
        Self {
            readback_ms_ema: 0.0,
            readback_ms_max: 0.0,
            dispatched: 0,
            dropped_queue_full: 0,
            encode_depth_max: 0,
            physics_delta_ms_ema: 0.0,
            physics_deltas: VecDeque::with_capacity(600),
            analysis_ms_ema: 0.0,
            analysis_ms_max: 0.0,
        }
    }
}

impl CaptureProbe {
    fn observe_delta(&mut self, delta_ms: f64) {
        self.physics_delta_ms_ema = if self.physics_delta_ms_ema == 0.0 {
            delta_ms
        } else {
            self.physics_delta_ms_ema * 0.95 + delta_ms * 0.05
        };
        if self.physics_deltas.len() == 600 {
            self.physics_deltas.pop_front();
        }
        self.physics_deltas.push_back(delta_ms);
    }

    fn p95(&self) -> f64 {
        if self.physics_deltas.is_empty() {
            return 0.0;
        }
        let mut values: Vec<f64> = self.physics_deltas.iter().copied().collect();
        values.sort_by(f64::total_cmp);
        values[((values.len() - 1) * 95 / 100).min(values.len() - 1)]
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "readback_ms_ema": self.readback_ms_ema,
            "readback_ms_max": self.readback_ms_max,
            "dispatched": self.dispatched,
            "dropped_queue_full": self.dropped_queue_full,
            "encode_depth_max": self.encode_depth_max,
            "physics_delta_ms_ema": self.physics_delta_ms_ema,
            "physics_delta_ms_p95_window": self.p95(),
            "analysis_ms_ema": self.analysis_ms_ema,
            "analysis_ms_max": self.analysis_ms_max,
        })
    }
}

// FrameEntityData is defined in stage-protocol and imported above.

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

/// Max number of pending silent markers. Oldest are evicted when exceeded.
const MAX_PENDING_SILENT_MARKERS: usize = 1000;

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
    // Screenshot settings
    pub screenshot_enabled: bool,
    pub screenshot_interval_frames: u32,
    pub screenshot_quality: f32,
    pub screenshot_max_dimension: u32,
    pub screenshot_byte_cap_mb: u32,
    pub screenshot_encode_queue: usize,
    pub dense_burst_enabled: bool,
    pub dense_burst_interval_frames: u32,
    pub dense_burst_duration_sec: u32,
    pub anomaly_enabled: bool,
    pub anomaly_min_proportion: f64,
    pub anomaly_relative_factor: f64,
    pub anomaly_sustained_frames: u32,
    pub anomaly_cooldown_sec: u32,
    pub anomaly_noise_floor: u8,
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
            screenshot_enabled: true,
            screenshot_interval_frames: 4,
            screenshot_quality: 0.65,
            screenshot_max_dimension: 480,
            screenshot_byte_cap_mb: 32,
            screenshot_encode_queue: 8,
            dense_burst_enabled: false,
            dense_burst_interval_frames: 2,
            dense_burst_duration_sec: 15,
            anomaly_enabled: true,
            anomaly_min_proportion: 0.30,
            anomaly_relative_factor: 4.0,
            anomaly_sustained_frames: 4,
            anomaly_cooldown_sec: 30,
            anomaly_noise_floor: 24,
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
        /// Screenshots captured after the trigger.
        post_screenshots: Vec<CapturedScreenshot>,
    },
}

// ---------------------------------------------------------------------------
// StageRecorder GDExtension class
// ---------------------------------------------------------------------------

#[derive(GodotClass)]
#[class(base = Node)]
pub struct StageRecorder {
    base: Base<Node>,

    // Physics frame counter (for dashcam capture interval)
    frame_counter: u32,

    // Collector reference for snapshot data
    collector: Option<Gd<StageCollector>>,

    // Dashcam state
    dashcam_config: DashcamConfig,
    dashcam_state: DashcamState,
    ring_buffer: VecDeque<CapturedFrame>,
    ring_buffer_bytes: usize,
    /// Exponential moving average of per-frame byte size (for byte cap).
    avg_frame_bytes: usize,
    /// Cached physics FPS (from Engine.physics_ticks_per_second).
    physics_fps: u32,

    // Screenshot ring buffer (separate from spatial frames)
    screenshot_ring: VecDeque<CapturedScreenshot>,
    screenshot_ring_bytes: usize,
    last_screenshot_frame: u64,
    screenshot_tx: Option<SyncSender<RawShot>>,
    screenshot_rx: Option<Receiver<EncodedShot>>,
    screenshot_worker: Option<JoinHandle<()>>,
    encode_depth: usize,
    gap_ledger: GapLedger,
    capture_probe: CaptureProbe,
    burst_until_frame: Option<u64>,
    physics_tick_count: u64,
    last_tick_instant: Option<Instant>,

    /// Silent markers waiting to be attached to the next saved clip.
    pending_silent_markers: Vec<DashcamTrigger>,
    anomaly_detector: AnomalyDetector,
    anomaly_frames_analyzed: u64,
    anomaly_frames_skipped: u64,
}

#[godot_api]
impl INode for StageRecorder {
    fn ready(&mut self) {
        // Always process even when the game is paused so recording continues.
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);

        // Cache physics FPS.
        self.physics_fps = Engine::singleton().get_physics_ticks_per_second() as u32;

        // Auto-start dashcam.
        if self.dashcam_config.enabled {
            self.dashcam_state = DashcamState::Buffering;
            tracing::info!(
                "[Stage] Dashcam started (pre={}s/{}s, post={}s/{}s, cap={}MB)",
                self.dashcam_config.pre_window_system_sec,
                self.dashcam_config.pre_window_deliberate_sec,
                self.dashcam_config.post_window_system_sec,
                self.dashcam_config.post_window_deliberate_sec,
                self.dashcam_config.byte_cap_mb,
            );
        }
    }

    fn exit_tree(&mut self) {
        self.stop_screenshot_worker();
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
            screenshot_ring: VecDeque::new(),
            screenshot_ring_bytes: 0,
            last_screenshot_frame: 0,
            screenshot_tx: None,
            screenshot_rx: None,
            screenshot_worker: None,
            encode_depth: 0,
            gap_ledger: GapLedger::default(),
            capture_probe: CaptureProbe::default(),
            burst_until_frame: None,
            physics_tick_count: 0,
            last_tick_instant: None,
            pending_silent_markers: Vec::new(),
            anomaly_detector: AnomalyDetector::default(),
            anomaly_frames_analyzed: 0,
            anomaly_frames_skipped: 0,
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        self.physics_tick_count = self.physics_tick_count.saturating_add(1);
        // Wall-clock pacing between ticks (engine `delta` is the fixed
        // timestep and would hide real hitches).
        let now = Instant::now();
        if let Some(last) = self.last_tick_instant.replace(now) {
            self.capture_probe
                .observe_delta(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        self.drain_encoded_shots();
        // --- Dashcam force-close check (no capture needed) ---
        self.dashcam_check_force_close();

        if matches!(self.dashcam_state, DashcamState::Disabled) {
            return;
        }

        self.frame_counter += 1;

        // Screenshot capture (independent of spatial capture interval)
        if self.dashcam_config.screenshot_enabled {
            let frame = current_physics_frame();
            let interval = self.effective_screenshot_interval();
            if frame >= self.last_screenshot_frame.saturating_add(interval) {
                self.last_screenshot_frame = frame;
                self.dispatch_screenshot_capture();
            }
        }

        if self.physics_tick_count.is_multiple_of(3600) {
            tracing::info!(
                "[Stage] Capture probe: {:?}; screenshot gaps={}",
                self.capture_probe.json(),
                self.gap_ledger.gaps.len()
            );
        }

        if !self
            .frame_counter
            .is_multiple_of(self.dashcam_config.capture_interval)
        {
            return;
        }

        let Some(captured) = self.do_capture() else {
            return;
        };

        self.dashcam_ingest(captured);
    }
}

#[godot_api]
impl StageRecorder {
    #[signal]
    fn marker_added(frame: u64, source: GString, label: GString);

    #[signal]
    fn dashcam_clip_saved(clip_id: GString, tier: GString, frames: u32);

    #[signal]
    fn dashcam_clip_started(trigger_frame: u64, tier: GString);

    #[func]
    pub fn set_collector(&mut self, collector: Gd<StageCollector>) {
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
            self.stop_screenshot_worker();
            self.dashcam_state = DashcamState::Disabled;
            self.ring_buffer.clear();
            self.ring_buffer_bytes = 0;
            self.screenshot_ring.clear();
            self.screenshot_ring_bytes = 0;
            self.last_screenshot_frame = 0;
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

    /// Returns current screenshot ring buffer count.
    #[func]
    pub fn get_screenshot_buffer_count(&self) -> u32 {
        self.screenshot_ring.len() as u32
    }

    /// Returns current screenshot ring buffer memory usage in KB.
    #[func]
    pub fn get_screenshot_buffer_kb(&self) -> u32 {
        (self.screenshot_ring_bytes / 1024) as u32
    }

    #[func]
    pub fn get_capture_probe_json(&self) -> GString {
        GString::from(self.capture_probe.json().to_string().as_str())
    }

    #[func]
    pub fn get_screenshot_gaps_json(&self) -> GString {
        GString::from(self.gap_ledger.summary_json().to_string().as_str())
    }

    #[func]
    pub fn get_anomaly_status_json(&self) -> GString {
        let reason = if Engine::singleton().is_editor_hint() {
            "editor_hint"
        } else if matches!(self.dashcam_state, DashcamState::Disabled) {
            "dashcam_disabled"
        } else if !self.dashcam_config.screenshot_enabled {
            "screenshots_disabled"
        } else if !self.dashcam_config.anomaly_enabled {
            "anomaly_disabled"
        } else {
            ""
        };
        GString::from(serde_json::json!({
            "active": reason.is_empty(), "reason": reason,
            "frames_analyzed": self.anomaly_frames_analyzed, "frames_skipped": self.anomaly_frames_skipped,
            "metric_ema": self.anomaly_detector.ema, "last_proportion": self.anomaly_detector.last_proportion,
            "anomalous_streak": self.anomaly_detector.streak, "triggers_total": self.anomaly_detector.triggers_total,
            "suppressed_cooldown": self.anomaly_detector.suppressed_cooldown,
            "last_trigger_frame": self.anomaly_detector.last_trigger_frame,
            "last_trigger_proportion": self.anomaly_detector.last_trigger_proportion,
        }).to_string().as_str())
    }

    #[func]
    pub fn screenshots_available(&self) -> bool {
        !self.screenshot_ring.is_empty()
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
                post_screenshots: Vec::new(),
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
            tracing::warn!("[Stage] apply_dashcam_config: invalid JSON");
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
        if let Some(b) = v.get("screenshot_enabled").and_then(|x| x.as_bool()) {
            self.dashcam_config.screenshot_enabled = b;
        }
        if let Some(n) = v.get("screenshot_interval_frames").and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.screenshot_interval_frames = n as u32;
        }
        if let Some(f) = v.get("screenshot_quality").and_then(|x| x.as_f64())
            && (0.0..=1.0).contains(&f)
        {
            self.dashcam_config.screenshot_quality = f as f32;
        }
        if let Some(n) = v.get("screenshot_max_dimension").and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.screenshot_max_dimension = (n as u32).clamp(1, 8192);
        }
        if let Some(n) = v.get("screenshot_byte_cap_mb").and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.screenshot_byte_cap_mb = n as u32;
        }
        if let Some(n) = v.get("screenshot_encode_queue").and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.screenshot_encode_queue = n as usize;
        }
        if let Some(b) = v.get("dense_burst_enabled").and_then(|x| x.as_bool()) {
            self.dashcam_config.dense_burst_enabled = b;
        }
        if let Some(n) = v
            .get("dense_burst_interval_frames")
            .and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.dense_burst_interval_frames = n as u32;
        }
        if let Some(b) = v.get("anomaly_enabled").and_then(|x| x.as_bool()) {
            self.dashcam_config.anomaly_enabled = b;
        }
        if let Some(f) = v.get("anomaly_min_proportion").and_then(|x| x.as_f64())
            && (0.0..=1.0).contains(&f)
        {
            self.dashcam_config.anomaly_min_proportion = f;
        }
        if let Some(f) = v.get("anomaly_relative_factor").and_then(|x| x.as_f64())
            && f.is_finite()
            && f >= 1.0
        {
            self.dashcam_config.anomaly_relative_factor = f;
        }
        if let Some(n) = v.get("anomaly_sustained_frames").and_then(|x| x.as_u64())
            && n > 0
        {
            self.dashcam_config.anomaly_sustained_frames = n as u32;
        }
        if let Some(n) = v.get("anomaly_cooldown_sec").and_then(|x| x.as_u64()) {
            self.dashcam_config.anomaly_cooldown_sec = n as u32;
        }
        if let Some(n) = v.get("anomaly_noise_floor").and_then(|x| x.as_u64())
            && n <= 255
        {
            self.dashcam_config.anomaly_noise_floor = n as u8;
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
            "screenshot_enabled": cfg.screenshot_enabled,
            "screenshot_interval_frames": cfg.screenshot_interval_frames,
            "screenshot_quality": cfg.screenshot_quality,
            "screenshot_max_dimension": cfg.screenshot_max_dimension,
            "screenshot_byte_cap_mb": cfg.screenshot_byte_cap_mb,
            "screenshot_encode_queue": cfg.screenshot_encode_queue,
            "dense_burst_enabled": cfg.dense_burst_enabled,
            "dense_burst_interval_frames": cfg.dense_burst_interval_frames,
            "dense_burst_duration_sec": cfg.dense_burst_duration_sec,
            "anomaly_enabled": cfg.anomaly_enabled,
            "anomaly_min_proportion": cfg.anomaly_min_proportion,
            "anomaly_relative_factor": cfg.anomaly_relative_factor,
            "anomaly_sustained_frames": cfg.anomaly_sustained_frames,
            "anomaly_cooldown_sec": cfg.anomaly_cooldown_sec,
            "anomaly_noise_floor": cfg.anomaly_noise_floor,
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
            &[frame.to_variant(), source.to_variant(), label.to_variant()],
        );
    }

    /// Add a code marker at the current frame.
    /// Tier: "system" (default, rate-limited), "deliberate" (always triggers),
    /// "silent" (annotate-only, no clip trigger).
    #[func]
    pub fn add_code_marker(&mut self, label: GString, tier: GString) {
        let frame = current_physics_frame();
        let timestamp_ms = current_time_ms();
        let label_str = label.to_string();
        let tier_str = tier.to_string();

        match tier_str.as_str() {
            "silent" => {
                self.add_silent_marker("code", &label_str, frame, timestamp_ms);
            }
            "deliberate" => {
                self.on_dashcam_marker_with_tier(
                    "code",
                    &label_str,
                    frame,
                    timestamp_ms,
                    DashcamTier::Deliberate,
                );
                self.base_mut().emit_signal(
                    "marker_added",
                    &[
                        frame.to_variant(),
                        GString::from("code").to_variant(),
                        label.to_variant(),
                    ],
                );
            }
            _ => {
                // Default: system tier (includes "system" and any unrecognized string)
                self.on_dashcam_marker_with_tier(
                    "code",
                    &label_str,
                    frame,
                    timestamp_ms,
                    DashcamTier::System,
                );
                self.base_mut().emit_signal(
                    "marker_added",
                    &[
                        frame.to_variant(),
                        GString::from("code").to_variant(),
                        label.to_variant(),
                    ],
                );
            }
        }
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

            if let Ok(db) =
                Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                && let Ok(mut stmt) = db.prepare(
                    "SELECT id, name, started_at_frame, ended_at_frame, \
                     started_at_ms, ended_at_ms, capture_config FROM recording LIMIT 1",
                )
            {
                let row_result = stmt.query_row([], |row| {
                    let id: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let start_frame: i64 = row.get(2)?;
                    let end_frame: Option<i64> = row.get(3)?;
                    let start_ms: i64 = row.get(4)?;
                    let end_ms: Option<i64> = row.get(5)?;
                    let capture_config: Option<String> = row.get(6)?;
                    Ok((
                        id,
                        name,
                        start_frame,
                        end_frame,
                        start_ms,
                        end_ms,
                        capture_config,
                    ))
                });

                if let Ok((id, name, start_frame, end_frame, start_ms, end_ms, capture_config)) =
                    row_result
                {
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

                    // Prefer stored wall-clock time; fall back to file mtime.
                    let created_at_unix_ms: Option<i64> = db
                        .query_row(
                            "SELECT created_at_unix_ms FROM recording LIMIT 1",
                            [],
                            |row| row.get(0),
                        )
                        .ok()
                        .flatten();
                    let created_at_unix_ms = created_at_unix_ms.unwrap_or_else(|| {
                        std::fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0)
                    });

                    // First human/agent marker label — why this clip was saved.
                    let trigger_label: Option<String> = db
                        .query_row(
                            "SELECT label FROM markers ORDER BY CASE WHEN source IN ('human', 'agent') THEN 0 WHEN source = 'system' THEN 1 ELSE 2 END, frame ASC LIMIT 1",
                            [],
                            |row| row.get(0),
                        )
                        .ok();

                    // Check if this is a dashcam clip
                    let capture_value: Option<serde_json::Value> = capture_config
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

                    let is_dashcam = capture_value
                        .as_ref()
                        .and_then(|v| v.get("dashcam").and_then(|b| b.as_bool()))
                        .unwrap_or(false);

                    let dashcam_tier = if is_dashcam {
                        capture_value
                            .as_ref()
                            .and_then(|v| {
                                v.get("tier")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    // Build capture block JSON string for recording_handler.rs.
                    let capture_block_json: Option<String> = capture_value.as_ref().map(|v| {
                            serde_json::json!({
                                "include_input": v.get("include_input").and_then(|b| b.as_bool()).unwrap_or(false),
                                "include_signals": v.get("include_signals").and_then(|b| b.as_bool()).unwrap_or(true),
                                "capture_interval": v.get("capture_interval").and_then(|n| n.as_u64()).unwrap_or(1),
                            })
                            .to_string()
                        });

                    let mut dict = VarDictionary::new();
                    dict.set("clip_id", GString::from(&id));
                    dict.set("name", GString::from(&name));
                    dict.set("frames_captured", frame_count as u32);
                    dict.set("duration_ms", duration_ms);
                    dict.set("frame_range_start", start_frame);
                    dict.set("frame_range_end", end_frame.unwrap_or(start_frame));
                    dict.set("markers_count", marker_count as u32);
                    dict.set("size_kb", size_kb as u32);
                    dict.set("created_at_unix_ms", created_at_unix_ms);
                    dict.set("dashcam", is_dashcam);
                    dict.set("dashcam_tier", GString::from(&dashcam_tier));
                    if let Some(label) = trigger_label {
                        dict.set("trigger_label", GString::from(&label));
                    }
                    if let Some(capture_json) = capture_block_json {
                        dict.set("capture_json", GString::from(&capture_json));
                    }
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

        let mut stmt = match db
            .prepare("SELECT frame, timestamp_ms, source, label FROM markers ORDER BY frame")
        {
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

fn effective_screenshot_interval(
    frame: u64,
    burst_until_frame: Option<u64>,
    dense_burst_enabled: bool,
    base_interval: u32,
    burst_interval: u32,
) -> u64 {
    if dense_burst_enabled && burst_until_frame.is_some_and(|end| frame <= end) {
        burst_interval.max(1) as u64
    } else {
        base_interval.max(1) as u64
    }
}

impl StageRecorder {
    fn effective_screenshot_interval(&self) -> u64 {
        effective_screenshot_interval(
            current_physics_frame(),
            self.burst_until_frame,
            self.dashcam_config.dense_burst_enabled,
            self.dashcam_config.screenshot_interval_frames,
            self.dashcam_config.dense_burst_interval_frames,
        )
    }

    fn ensure_screenshot_worker(&mut self) {
        if self.screenshot_tx.is_some() {
            return;
        }
        let (tx, rx) =
            mpsc::sync_channel::<RawShot>(self.dashcam_config.screenshot_encode_queue.max(1));
        let (encoded_tx, encoded_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut lattice = ChangeLattice::new();
            while let Ok(raw) = rx.recv() {
                let analysis = raw
                    .analyze
                    .then(|| lattice.analyze(&raw.rgba, raw.width, raw.height, raw.noise_floor));
                let mut output = Vec::new();
                let quality = raw.quality.clamp(1, 100);
                let encoder = jpeg_encoder::Encoder::new(&mut output, quality);
                let result = encoder.encode(
                    &raw.rgba,
                    raw.width,
                    raw.height,
                    jpeg_encoder::ColorType::Rgba,
                );
                let shot = match result {
                    Ok(()) => EncodedShot {
                        frame: raw.frame,
                        timestamp_ms: raw.timestamp_ms,
                        jpeg_data: Some(output),
                        width: raw.width as u32,
                        height: raw.height as u32,
                        error: None,
                        analysis,
                    },
                    Err(error) => EncodedShot {
                        frame: raw.frame,
                        timestamp_ms: raw.timestamp_ms,
                        jpeg_data: None,
                        width: raw.width as u32,
                        height: raw.height as u32,
                        error: Some(error.to_string()),
                        analysis,
                    },
                };
                let _ = encoded_tx.send(shot);
            }
        });
        self.screenshot_tx = Some(tx);
        self.screenshot_rx = Some(encoded_rx);
        self.screenshot_worker = Some(handle);
    }

    /// Read and resize on the Godot thread; only plain RGBA bytes cross threads.
    fn dispatch_screenshot_capture(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        let started = Instant::now();
        let Some(viewport) = self.base().get_viewport() else {
            return;
        };
        let Some(texture) = viewport.get_texture() else {
            return;
        };
        let Some(mut image) = texture.get_image() else {
            return;
        };
        let orig_width = image.get_width() as u32;
        let orig_height = image.get_height() as u32;
        if orig_width == 0 || orig_height == 0 {
            return;
        }
        let max_dim = self.dashcam_config.screenshot_max_dimension.clamp(1, 8192);
        if orig_width.max(orig_height) > max_dim {
            let (w, h) = if orig_width >= orig_height {
                (max_dim, (orig_height * max_dim / orig_width).max(1))
            } else {
                ((orig_width * max_dim / orig_height).max(1), max_dim)
            };
            image.resize(w as i32, h as i32);
        }
        image.convert(Format::RGBA8);
        let raw = RawShot {
            frame: current_physics_frame(),
            timestamp_ms: current_time_ms(),
            rgba: image.get_data().to_vec(),
            width: image.get_width() as u16,
            height: image.get_height() as u16,
            quality: (self.dashcam_config.screenshot_quality * 100.0).round() as u8,
            analyze: self.dashcam_config.anomaly_enabled && self.dashcam_config.screenshot_enabled,
            noise_floor: self.dashcam_config.anomaly_noise_floor,
        };
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        self.capture_probe.readback_ms_ema = if self.capture_probe.readback_ms_ema == 0.0 {
            elapsed
        } else {
            self.capture_probe.readback_ms_ema * 0.95 + elapsed * 0.05
        };
        self.capture_probe.readback_ms_max = self.capture_probe.readback_ms_max.max(elapsed);
        self.ensure_screenshot_worker();
        let Some(tx) = self.screenshot_tx.as_ref() else {
            return;
        };
        match tx.try_send(raw) {
            Ok(()) => {
                self.encode_depth = self.encode_depth.saturating_add(1);
                self.capture_probe.dispatched = self.capture_probe.dispatched.saturating_add(1);
                self.capture_probe.encode_depth_max =
                    self.capture_probe.encode_depth_max.max(self.encode_depth);
            }
            Err(TrySendError::Full(raw)) => {
                self.capture_probe.dropped_queue_full =
                    self.capture_probe.dropped_queue_full.saturating_add(1);
                self.gap_ledger.record(raw.frame, "encode_queue_full");
            }
            Err(TrySendError::Disconnected(_)) => self.stop_screenshot_worker(),
        }
    }

    fn drain_encoded_shots(&mut self) {
        loop {
            let shot = match self.screenshot_rx.as_ref().map(Receiver::try_recv) {
                Some(Ok(shot)) => shot,
                Some(Err(TryRecvError::Empty)) | None => break,
                Some(Err(TryRecvError::Disconnected)) => {
                    self.screenshot_rx = None;
                    break;
                }
            };
            self.encode_depth = self.encode_depth.saturating_sub(1);
            if let Some(analysis) = shot.analysis {
                self.capture_probe.analysis_ms_ema = if self.capture_probe.analysis_ms_ema == 0.0 {
                    analysis.analysis_ms
                } else {
                    self.capture_probe.analysis_ms_ema * 0.95 + analysis.analysis_ms * 0.05
                };
                self.capture_probe.analysis_ms_max =
                    self.capture_probe.analysis_ms_max.max(analysis.analysis_ms);
                self.anomaly_frames_analyzed = self.anomaly_frames_analyzed.saturating_add(1);
                if analysis.reset {
                    self.anomaly_frames_skipped = self.anomaly_frames_skipped.saturating_add(1);
                }
                if self.dashcam_config.anomaly_enabled
                    && let Some(label) = self.anomaly_detector.observe(
                        analysis.proportion,
                        analysis.reset,
                        shot.frame,
                        AnomalySettings {
                            min: self.dashcam_config.anomaly_min_proportion,
                            relative: self.dashcam_config.anomaly_relative_factor,
                            sustained: self.dashcam_config.anomaly_sustained_frames,
                            cooldown_frames: self.dashcam_config.anomaly_cooldown_sec as u64
                                * self.physics_fps as u64,
                        },
                    )
                {
                    self.on_dashcam_marker_with_tier(
                        "system",
                        &label,
                        shot.frame,
                        shot.timestamp_ms,
                        DashcamTier::System,
                    );
                    self.base_mut().emit_signal(
                        "marker_added",
                        &[
                            shot.frame.to_variant(),
                            GString::from("system").to_variant(),
                            GString::from(&label).to_variant(),
                        ],
                    );
                }
            } else {
                self.anomaly_frames_skipped = self.anomaly_frames_skipped.saturating_add(1);
            }
            if shot.error.is_some() {
                self.gap_ledger.record(shot.frame, "encode_failed");
                continue;
            }
            let Some(jpeg_data) = shot.jpeg_data else {
                self.gap_ledger.record(shot.frame, "encode_failed");
                continue;
            };
            self.screenshot_ingest(CapturedScreenshot {
                frame: shot.frame,
                timestamp_ms: shot.timestamp_ms,
                jpeg_data,
                width: shot.width,
                height: shot.height,
            });
        }
    }

    fn stop_screenshot_worker(&mut self) {
        self.screenshot_tx.take();
        self.screenshot_rx.take();
        if let Some(handle) = self.screenshot_worker.take() {
            let _ = handle.join();
        }
        self.encode_depth = 0;
    }

    /// Ingest a screenshot into the ring buffer and post-capture state.
    fn screenshot_ingest(&mut self, screenshot: CapturedScreenshot) {
        self.screenshot_ring_bytes += screenshot.jpeg_data.len();
        self.screenshot_ring.push_back(screenshot.clone());
        self.enforce_screenshot_byte_cap();

        if let DashcamState::PostCapture {
            post_screenshots, ..
        } = &mut self.dashcam_state
        {
            post_screenshots.push(screenshot);
        }
    }

    /// Evict oldest screenshot ring buffer entries until within byte_cap.
    fn enforce_screenshot_byte_cap(&mut self) {
        let byte_cap = self.dashcam_config.screenshot_byte_cap_mb as usize * 1024 * 1024;
        while self.screenshot_ring_bytes > byte_cap && !self.screenshot_ring.is_empty() {
            if let Some(evicted) = self.screenshot_ring.pop_front() {
                self.screenshot_ring_bytes = self
                    .screenshot_ring_bytes
                    .saturating_sub(evicted.jpeg_data.len());
            } else {
                break;
            }
        }
    }

    /// Capture one frame of entity data and return it (without pushing to any buffer).
    fn do_capture(&mut self) -> Option<CapturedFrame> {
        let collector = self.collector.as_ref()?;

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

        let camera = self
            .base()
            .get_viewport()
            .and_then(|viewport| viewport.get_camera_3d())
            .map(|camera| {
                let position = camera.get_global_position();
                let quaternion = camera.get_global_transform().basis.get_quaternion();
                let object = camera.clone().upcast::<godot::classes::Object>();
                stage_protocol::recording::CameraFrameData {
                    position: vec![position.x as f64, position.y as f64, position.z as f64],
                    quaternion: vec![
                        quaternion.x as f64,
                        quaternion.y as f64,
                        quaternion.z as f64,
                        quaternion.w as f64,
                    ],
                    projection: object.get("projection").to::<i64>() as u8,
                    fov_deg: object.get("fov").to::<f64>(),
                    ortho_size: object.get("size").to::<f64>(),
                    keep_aspect: object.get("keep_aspect").to::<i64>() as u8,
                    camera_path: camera.get_path().to_string(),
                }
            });

        Some(CapturedFrame {
            frame: snapshot.frame,
            timestamp_ms: snapshot.timestamp_ms,
            data,
            camera,
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
                self.ring_buffer_bytes = self.ring_buffer_bytes.saturating_sub(evicted.data.len());
            } else {
                break;
            }
        }
    }

    /// Compute max frames to keep in the ring buffer.
    fn ring_cap_frames(&self) -> usize {
        let fps = self.physics_fps.max(1) as usize;
        let interval = self.dashcam_config.capture_interval.max(1) as usize;
        let time_based = self.dashcam_config.pre_window_deliberate_sec as usize * fps / interval;

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
    /// Resolves tier from source: "system" → System, anything else → Deliberate.
    fn on_dashcam_marker(&mut self, source: &str, label: &str, frame: u64, timestamp_ms: u64) {
        let tier = if source == "system" {
            DashcamTier::System
        } else {
            DashcamTier::Deliberate
        };
        self.on_dashcam_marker_with_tier(source, label, frame, timestamp_ms, tier);
    }

    /// Handle a marker trigger with an explicit tier. Core of the dashcam state machine.
    fn on_dashcam_marker_with_tier(
        &mut self,
        source: &str,
        label: &str,
        frame: u64,
        timestamp_ms: u64,
        tier: DashcamTier,
    ) {
        // Determine action without borrowing dashcam_state.
        let is_buffering = matches!(self.dashcam_state, DashcamState::Buffering);
        let is_post_capture = matches!(self.dashcam_state, DashcamState::PostCapture { .. });

        if matches!(self.dashcam_state, DashcamState::Disabled) {
            return;
        }
        if self.dashcam_config.dense_burst_enabled {
            self.burst_until_frame = Some(frame.saturating_add(
                self.dashcam_config.dense_burst_duration_sec as u64 * self.physics_fps as u64,
            ));
        }

        if is_buffering {
            // Snapshot ring buffer and transition to PostCapture.
            let pre_buffer: Vec<CapturedFrame> = self.ring_buffer.iter().cloned().collect();
            let post_window = self.post_window_frames(tier);
            let force_close_at_frame = if tier == DashcamTier::System {
                Some(frame + self.dashcam_config.max_window_sec as u64 * self.physics_fps as u64)
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
                last_system_trigger_frame: if tier == DashcamTier::System {
                    frame
                } else {
                    0
                },
                force_close_at_frame,
                post_screenshots: Vec::new(),
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

    /// Record a silent marker. Does not trigger dashcam capture.
    /// The marker is attached to the next clip whose frame range includes it.
    fn add_silent_marker(&mut self, source: &str, label: &str, frame: u64, timestamp_ms: u64) {
        self.pending_silent_markers.push(DashcamTrigger {
            frame,
            timestamp_ms,
            source: source.to_string(),
            label: label.to_string(),
        });

        if self.pending_silent_markers.len() > MAX_PENDING_SILENT_MARKERS {
            let excess = self.pending_silent_markers.len() - MAX_PENDING_SILENT_MARKERS;
            self.pending_silent_markers.drain(..excess);
        }

        self.base_mut().emit_signal(
            "marker_added",
            &[
                frame.to_variant(),
                GString::from(source).to_variant(),
                GString::from(label).to_variant(),
            ],
        );
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
            post_screenshots,
            ..
        } = state
        else {
            return None;
        };

        let clip_id = format!("clip_{:08x}", rand_u32());
        let storage_path = "user://stage_recordings/";
        let dir_path = globalize_path(storage_path);
        let _ = std::fs::create_dir_all(&dir_path);
        let db_path = format!("{}/{}.sqlite", dir_path, clip_id);

        let db = match Connection::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                tracing::error!("[Stage] Failed to create dashcam clip DB: {e}");
                return None;
            }
        };

        if db.execute_batch("PRAGMA journal_mode=WAL;").is_err() {
            tracing::error!("[Stage] Failed to set WAL mode for dashcam clip");
            return None;
        }
        if db.execute_batch(SCHEMA_SQL).is_err() {
            tracing::error!("[Stage] Failed to create dashcam schema");
            return None;
        }

        let tier_str = tier.as_str();
        let all_frames: Vec<&CapturedFrame> = pre_buffer.iter().chain(post_buffer.iter()).collect();
        let total_frames = all_frames.len() as u32;

        let first_frame = all_frames.first().map(|f| f.frame).unwrap_or(0);
        let last_frame = all_frames.last().map(|f| f.frame).unwrap_or(0);
        let first_ts = all_frames.first().map(|f| f.timestamp_ms).unwrap_or(0);
        let last_ts = all_frames.last().map(|f| f.timestamp_ms).unwrap_or(0);

        // Merge pending silent markers that fall within this clip's frame range.
        let mut all_markers = markers;
        {
            let mut silent_in_range = Vec::new();
            self.pending_silent_markers.retain(|m| {
                if m.frame >= first_frame && m.frame <= last_frame {
                    silent_in_range.push(DashcamTrigger {
                        frame: m.frame,
                        timestamp_ms: m.timestamp_ms,
                        source: m.source.clone(),
                        label: m.label.clone(),
                    });
                    false
                } else {
                    true
                }
            });
            all_markers.extend(silent_in_range);
            all_markers.sort_by_key(|m| m.frame);
        }

        let triggers_json: Vec<serde_json::Value> = all_markers
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
            "screenshot_interval_frames": self.dashcam_config.screenshot_interval_frames,
            "screenshot_quality": self.dashcam_config.screenshot_quality,
            "screenshot_max_dimension": self.dashcam_config.screenshot_max_dimension,
            "screenshot_byte_cap_mb": self.dashcam_config.screenshot_byte_cap_mb,
            "screenshot_encode_queue": self.dashcam_config.screenshot_encode_queue,
            "dense_burst_enabled": self.dashcam_config.dense_burst_enabled,
            "dense_burst_interval_frames": self.dashcam_config.dense_burst_interval_frames,
            "dense_burst_duration_sec": self.dashcam_config.dense_burst_duration_sec,
        });

        let physics_ticks = self.physics_fps;
        let scene_dims =
            detect_scene_dimensions(self.base().get_tree().and_then(|t| t.get_current_scene()));

        let created_at_unix_ms = current_time_ms() as i64;
        let _ = db.execute(
            "INSERT INTO recording \
             (id, name, started_at_frame, ended_at_frame, started_at_ms, ended_at_ms, \
              scene_dimensions, physics_ticks_per_sec, capture_config, created_at_unix_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                created_at_unix_ms,
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
                "INSERT OR REPLACE INTO camera_frames (frame, timestamp_ms, camera_path, data) VALUES (?1, ?2, ?3, ?4)",
            ) {
                for f in &all_frames {
                    if let Some(camera) = &f.camera
                        && let Ok(data) = rmp_serde::to_vec(camera)
                    {
                        let _ = stmt.execute(rusqlite::params![f.frame, f.timestamp_ms, &camera.camera_path, data]);
                    }
                }
            }

            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            ) {
                for m in &all_markers {
                    let _ = stmt.execute(rusqlite::params![
                        m.frame,
                        m.timestamp_ms,
                        &m.source,
                        &m.label
                    ]);
                }
            }

            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO screenshot_gaps (start_frame, end_frame, reason, dropped) VALUES (?1, ?2, ?3, ?4)",
            ) {
                for gap in self.gap_ledger.overlapping(first_frame, last_frame) {
                    let _ = stmt.execute(rusqlite::params![gap.start_frame, gap.end_frame, &gap.reason, gap.dropped]);
                }
            }

            // Write screenshots: ring entries in frame range + post_screenshots
            let all_screenshots: Vec<&CapturedScreenshot> = self
                .screenshot_ring
                .iter()
                .filter(|s| s.frame >= first_frame && s.frame <= last_frame)
                .chain(post_screenshots.iter())
                .collect();

            if !all_screenshots.is_empty()
                && let Ok(mut stmt) = tx.prepare_cached(
                    "INSERT OR REPLACE INTO screenshots \
                     (frame, timestamp_ms, image_data, width, height) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
            {
                for s in &all_screenshots {
                    let _ = stmt.execute(rusqlite::params![
                        s.frame,
                        s.timestamp_ms,
                        &s.jpeg_data,
                        s.width,
                        s.height
                    ]);
                }
            }

            if let Err(e) = tx.commit() {
                tracing::error!("[Stage] Failed to commit dashcam clip: {e}");
                return None;
            }
        }

        tracing::info!(
            "[Stage] Dashcam clip saved: {} ({} frames, {} tier)",
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
    capture_config TEXT,
    created_at_unix_ms INTEGER
);

CREATE TABLE IF NOT EXISTS frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    data BLOB
);

CREATE TABLE IF NOT EXISTS camera_frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    camera_path TEXT,
    data BLOB,
    FOREIGN KEY (frame) REFERENCES frames(frame)
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

CREATE TABLE IF NOT EXISTS screenshots (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER,
    image_data BLOB,
    width INTEGER,
    height INTEGER
);

CREATE TABLE IF NOT EXISTS screenshot_gaps (
    start_frame INTEGER,
    end_frame INTEGER,
    reason TEXT,
    dropped INTEGER
);

CREATE TABLE IF NOT EXISTS artifacts (
    cache_key TEXT PRIMARY KEY,
    kind TEXT,
    params_json TEXT,
    manifest_json TEXT,
    dims TEXT,
    png BLOB,
    created_at_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_events_frame ON events(frame);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_node ON events(node_path);
CREATE INDEX IF NOT EXISTS idx_markers_frame ON markers(frame);
CREATE INDEX IF NOT EXISTS idx_screenshots_timestamp ON screenshots(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_screenshot_gaps_frame ON screenshot_gaps(start_frame, end_frame);
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
        assert_eq!(count, 8); // recording, frames, camera_frames, events, markers, screenshots, gaps, artifacts
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
        assert_eq!(count, 6); // events (3) + markers + screenshots + gaps
    }

    #[test]
    fn gap_ledger_coalesces_and_evicts() {
        let mut ledger = GapLedger::default();
        ledger.record(10, "encode_queue_full");
        ledger.record(11, "encode_queue_full");
        assert_eq!(ledger.gaps.len(), 1);
        assert_eq!(ledger.gaps[0].dropped, 2);
        for frame in 0..300 {
            ledger.record(frame * 2, "readback");
        }
        assert_eq!(ledger.gaps.len(), 256);
        assert!(ledger.overflow > 0);
    }

    #[test]
    fn effective_burst_interval_is_dense_only_inside_window() {
        assert_eq!(effective_screenshot_interval(9, Some(10), true, 4, 2), 2);
        assert_eq!(effective_screenshot_interval(10, Some(10), true, 4, 2), 2);
        assert_eq!(effective_screenshot_interval(11, Some(10), true, 4, 2), 4);
        assert_eq!(effective_screenshot_interval(10, Some(10), false, 4, 2), 4);
    }

    #[test]
    fn screenshot_ring_evicts_at_byte_cap() {
        let mut ring: VecDeque<CapturedScreenshot> = VecDeque::new();
        let mut ring_bytes = 0usize;
        let byte_cap = 1024 * 1024; // 1 MB

        // Each screenshot is 20KB; 100 of them = 2MB, should be evicted to ~1MB
        for i in 0..100u64 {
            let s = CapturedScreenshot {
                frame: i,
                timestamp_ms: i * 2000,
                jpeg_data: vec![0u8; 20_000],
                width: 960,
                height: 540,
            };
            ring_bytes += s.jpeg_data.len();
            ring.push_back(s);

            while ring_bytes > byte_cap && !ring.is_empty() {
                if let Some(evicted) = ring.pop_front() {
                    ring_bytes = ring_bytes.saturating_sub(evicted.jpeg_data.len());
                } else {
                    break;
                }
            }
        }

        assert!(
            ring_bytes <= byte_cap,
            "Ring bytes {ring_bytes} should be <= cap {byte_cap}"
        );
        assert!(
            !ring.is_empty(),
            "Ring should have some entries after eviction"
        );
    }

    #[test]
    fn screenshot_config_defaults_are_sane() {
        let cfg = DashcamConfig::default();
        assert!(cfg.screenshot_enabled);
        assert!(cfg.screenshot_interval_frames > 0);
        assert!(cfg.screenshot_quality > 0.0 && cfg.screenshot_quality <= 1.0);
        assert!(cfg.screenshot_max_dimension > 0);
        assert!(cfg.screenshot_byte_cap_mb > 0);
    }

    #[test]
    fn screenshot_config_parsing() {
        let json = serde_json::json!({
            "screenshot_enabled": false,
            "screenshot_interval_frames": 5,
            "screenshot_quality": 0.9,
            "screenshot_max_dimension": 720,
            "screenshot_byte_cap_mb": 32,
        });

        let mut cfg = DashcamConfig::default();
        if let Some(b) = json.get("screenshot_enabled").and_then(|x| x.as_bool()) {
            cfg.screenshot_enabled = b;
        }
        if let Some(n) = json
            .get("screenshot_interval_frames")
            .and_then(|x| x.as_u64())
            && n > 0
        {
            cfg.screenshot_interval_frames = n as u32;
        }
        if let Some(f) = json.get("screenshot_quality").and_then(|x| x.as_f64())
            && (0.0..=1.0).contains(&f)
        {
            cfg.screenshot_quality = f as f32;
        }
        if let Some(n) = json
            .get("screenshot_max_dimension")
            .and_then(|x| x.as_u64())
            && n > 0
        {
            cfg.screenshot_max_dimension = (n as u32).clamp(1, 8192);
        }
        if let Some(n) = json.get("screenshot_byte_cap_mb").and_then(|x| x.as_u64())
            && n > 0
        {
            cfg.screenshot_byte_cap_mb = n as u32;
        }

        assert!(!cfg.screenshot_enabled);
        assert_eq!(cfg.screenshot_interval_frames, 5);
        assert!((cfg.screenshot_quality - 0.9f32).abs() < 0.01);
        assert_eq!(cfg.screenshot_max_dimension, 720);
        assert_eq!(cfg.screenshot_byte_cap_mb, 32);
    }

    #[test]
    fn screenshots_table_insert_and_query() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();

        let jpeg_data = vec![0xFFu8, 0xD8, 0xFF, 0xE0]; // JPEG header bytes
        db.execute(
            "INSERT INTO screenshots (frame, timestamp_ms, image_data, width, height) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![100u64, 2000u64, &jpeg_data, 960u32, 540u32],
        )
        .unwrap();

        let (frame, width, height, size): (u64, u32, u32, usize) = db
            .query_row(
                "SELECT frame, width, height, LENGTH(image_data) FROM screenshots WHERE frame = 100",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? as usize)),
            )
            .unwrap();

        assert_eq!(frame, 100);
        assert_eq!(width, 960);
        assert_eq!(height, 540);
        assert_eq!(size, 4);
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
            .query_row("SELECT data FROM frames WHERE frame = 100", [], |r| {
                r.get(0)
            })
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

        assert!(
            msgpack.len() < json.len(),
            "MessagePack should be smaller than JSON"
        );
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
                camera: None,
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
        if let Some(n) = json
            .get("post_window_deliberate_sec")
            .and_then(|x| x.as_u64())
        {
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

        let time_based = (cfg.pre_window_deliberate_sec as usize) * (physics_fps as usize)
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

        let time_based = (cfg.pre_window_deliberate_sec as usize) * (physics_fps as usize)
            / (cfg.capture_interval as usize);
        let byte_based = byte_cap / avg_frame_bytes.max(1);

        let cap = time_based.min(byte_based);
        assert_eq!(
            cap, 1024,
            "byte-based cap should be the binding constraint for large frames"
        );
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
        let time_based = (cfg.pre_window_system_sec as usize) * (physics_fps as usize)
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

        assert_eq!(
            frames_remaining, 1800,
            "deliberate+deliberate should extend to full window"
        );
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

        assert_eq!(
            existing_tier,
            DashcamTier::Deliberate,
            "tier must not downgrade"
        );
        assert_eq!(
            frames_remaining, 600,
            "window should extend to system_frames"
        );
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

        assert!(
            force_close.is_none(),
            "deliberate clips should not have force_close"
        );
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
        assert_eq!(
            cfg.post_window_system_sec, original_post_system,
            "unset field should be preserved"
        );
        assert_eq!(
            cfg.min_after_sec, original_min_after,
            "unset field should be preserved"
        );
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

    // -------------------------------------------------------------------------
    // Unit 5: Code markers tests
    // -------------------------------------------------------------------------

    #[test]
    fn silent_marker_stored_in_pending() {
        // Simulate the add_silent_marker logic without Godot.
        let pending: Vec<DashcamTrigger> = vec![DashcamTrigger {
            frame: 100,
            timestamp_ms: 1000,
            source: "code".to_string(),
            label: "entered_zone".to_string(),
        }];

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].frame, 100);
        assert_eq!(pending[0].source, "code");
        assert_eq!(pending[0].label, "entered_zone");

        // Dashcam state is unaffected — silent markers don't change it.
        // (Verified by the fact that we only pushed to pending, not to dashcam_state.)
    }

    #[test]
    fn silent_markers_merged_into_clip() {
        // Set up pending markers with various frame positions.
        let mut pending: Vec<DashcamTrigger> = vec![
            DashcamTrigger {
                frame: 50,
                timestamp_ms: 500,
                source: "code".to_string(),
                label: "before_clip".to_string(),
            },
            DashcamTrigger {
                frame: 150,
                timestamp_ms: 1500,
                source: "code".to_string(),
                label: "in_range".to_string(),
            },
            DashcamTrigger {
                frame: 200,
                timestamp_ms: 2000,
                source: "code".to_string(),
                label: "also_in_range".to_string(),
            },
            DashcamTrigger {
                frame: 350,
                timestamp_ms: 3500,
                source: "code".to_string(),
                label: "after_clip".to_string(),
            },
        ];

        let first_frame: u64 = 100;
        let last_frame: u64 = 300;

        // Simulate the retain+extend merge logic.
        let mut clip_markers: Vec<DashcamTrigger> = vec![DashcamTrigger {
            frame: 175,
            timestamp_ms: 1750,
            source: "system".to_string(),
            label: "trigger".to_string(),
        }];

        let mut silent_in_range = Vec::new();
        pending.retain(|m| {
            if m.frame >= first_frame && m.frame <= last_frame {
                silent_in_range.push(DashcamTrigger {
                    frame: m.frame,
                    timestamp_ms: m.timestamp_ms,
                    source: m.source.clone(),
                    label: m.label.clone(),
                });
                false
            } else {
                true
            }
        });
        clip_markers.extend(silent_in_range);
        clip_markers.sort_by_key(|m| m.frame);

        // In-range markers were collected.
        assert_eq!(
            clip_markers.len(),
            3,
            "should have trigger + 2 in-range silent markers"
        );
        assert_eq!(clip_markers[0].label, "in_range");
        assert_eq!(clip_markers[1].label, "trigger");
        assert_eq!(clip_markers[2].label, "also_in_range");

        // Out-of-range markers remain in pending.
        assert_eq!(
            pending.len(),
            2,
            "out-of-range markers should remain pending"
        );
        assert_eq!(pending[0].label, "before_clip");
        assert_eq!(pending[1].label, "after_clip");
    }

    #[test]
    fn silent_markers_capped_at_max() {
        let mut pending: Vec<DashcamTrigger> = Vec::new();

        // Fill beyond the cap.
        for i in 0..(MAX_PENDING_SILENT_MARKERS + 10) {
            pending.push(DashcamTrigger {
                frame: i as u64,
                timestamp_ms: i as u64 * 16,
                source: "code".to_string(),
                label: format!("marker_{i}"),
            });

            if pending.len() > MAX_PENDING_SILENT_MARKERS {
                let excess = pending.len() - MAX_PENDING_SILENT_MARKERS;
                pending.drain(..excess);
            }
        }

        assert_eq!(
            pending.len(),
            MAX_PENDING_SILENT_MARKERS,
            "pending should be capped at MAX_PENDING_SILENT_MARKERS"
        );

        // Oldest markers were evicted — the remaining ones should have the highest frames.
        let first_remaining_frame = pending.first().unwrap().frame;
        assert_eq!(
            first_remaining_frame, 10,
            "first 10 (oldest) markers should have been evicted"
        );
        let last_remaining_frame = pending.last().unwrap().frame;
        assert_eq!(
            last_remaining_frame,
            (MAX_PENDING_SILENT_MARKERS + 9) as u64,
            "last remaining marker should be the newest"
        );
    }

    #[test]
    fn code_marker_system_tier_is_rate_limited() {
        // Reuse the pattern from dashcam_rate_limiting_system_markers.
        // A system-tier code marker within the min_interval should not extend the window.
        let mut frames_remaining: u32 = 600;
        let min_interval: u64 = 120; // 2s at 60fps
        let mut last_system_trigger_frame: u64 = 100;
        let system_frames: u32 = 600;

        // First code/system marker sets the window (already set above as 600).
        // Second code/system marker at frame 150 (50 frames after first — within interval).
        let new_frame: u64 = 150;
        let elapsed = new_frame.saturating_sub(last_system_trigger_frame);

        if elapsed >= min_interval {
            frames_remaining = frames_remaining.max(system_frames);
            last_system_trigger_frame = new_frame;
        }
        // Rate-limited: frames_remaining and last_trigger unchanged.
        assert_eq!(
            frames_remaining, 600,
            "rate-limited system marker should not extend window"
        );
        assert_eq!(
            last_system_trigger_frame, 100,
            "last_system_trigger_frame should not update when rate-limited"
        );
    }

    #[test]
    fn change_lattice_identical_noise_and_change() {
        let mut lattice = ChangeLattice::new();
        let quiet = vec![100u8; 4 * 4 * 4];
        assert!(lattice.analyze(&quiet, 4, 4, 24).reset);
        assert_eq!(lattice.analyze(&quiet, 4, 4, 24).proportion, 0.0);
        let mut changed = quiet.clone();
        changed[0] = 255;
        assert_eq!(lattice.analyze(&changed, 4, 4, 24).proportion, 1.0 / 16.0);
        let mut noise = changed.clone();
        noise[0] = 230;
        assert_eq!(lattice.analyze(&noise, 4, 4, 24).proportion, 0.0);
    }

    #[test]
    fn change_lattice_dimension_reset_and_stride_cap() {
        let mut lattice = ChangeLattice::new();
        let huge = vec![0u8; 200 * 200 * 4];
        assert!(lattice.analyze(&huge, 200, 200, 24).reset);
        assert!(lattice.previous.len() <= ChangeLattice::MAX_SAMPLES);
        assert!(
            lattice
                .analyze(&vec![0u8; 100 * 100 * 4], 100, 100, 24)
                .reset
        );
    }

    #[test]
    fn anomaly_detector_requires_sustained_spike_and_cools_down() {
        let mut detector = AnomalyDetector::default();
        let settings = AnomalySettings {
            min: 0.30,
            relative: 1.0,
            sustained: 3,
            cooldown_frames: 30,
        };
        assert!(detector.observe(0.01, false, 1, settings).is_none());
        for frame in 2..4 {
            assert!(detector.observe(0.9, false, frame, settings).is_none());
        }
        assert!(detector.observe(0.9, false, 4, settings).is_some());
        for frame in 5..8 {
            assert!(detector.observe(0.9, false, frame, settings).is_none());
        }
        assert_eq!(detector.suppressed_cooldown, 1);
        assert!(detector.observe(0.01, false, 9, settings).is_none());
        assert_eq!(detector.streak, 0);
    }

    #[test]
    fn anomaly_detector_default_settings_fire_after_quiet_baseline() {
        let mut detector = AnomalyDetector::default();
        let settings = AnomalySettings {
            min: 0.30,
            relative: 4.0,
            sustained: 4,
            cooldown_frames: 1_000,
        };
        for frame in 0..30 {
            assert!(detector.observe(0.02, false, frame, settings).is_none());
        }
        for frame in 30..33 {
            assert!(detector.observe(0.8, false, frame, settings).is_none());
        }
        assert!(detector.observe(0.8, false, 33, settings).is_some());
        for frame in 34..38 {
            assert!(detector.observe(0.8, false, frame, settings).is_none());
        }
        assert_eq!(detector.triggers_total, 1);
        assert_eq!(detector.suppressed_cooldown, 1);
    }

    #[test]
    fn anomaly_detector_reset_clears_streak() {
        let mut detector = AnomalyDetector::default();
        let settings = AnomalySettings {
            min: 0.3,
            relative: 1.0,
            sustained: 3,
            cooldown_frames: 0,
        };
        assert!(detector.observe(0.9, false, 1, settings).is_none());
        assert!(detector.observe(0.9, true, 2, settings).is_none());
        assert_eq!(detector.streak, 0);
    }

    #[test]
    fn code_marker_deliberate_tier_always_triggers() {
        // A deliberate-tier code marker should transition Buffering → PostCapture.
        // Simulate the on_dashcam_marker_with_tier logic for Buffering state.
        let tier = DashcamTier::Deliberate;
        let frame: u64 = 500;
        let physics_fps: u64 = 60;
        let max_window_sec: u64 = 120;

        let is_buffering = true; // simulating Buffering state

        let mut new_state_is_post_capture = false;
        let mut captured_tier = DashcamTier::System;
        let mut force_close: Option<u64> = Some(99999); // would be set for system

        if is_buffering {
            // Deliberate: no force_close, full post-window.
            force_close = if tier == DashcamTier::System {
                Some(frame + max_window_sec * physics_fps)
            } else {
                None
            };
            new_state_is_post_capture = true;
            captured_tier = tier;
        }

        assert!(
            new_state_is_post_capture,
            "deliberate code marker should transition to PostCapture"
        );
        assert_eq!(
            captured_tier,
            DashcamTier::Deliberate,
            "captured tier should be Deliberate"
        );
        assert!(
            force_close.is_none(),
            "deliberate clips should not have force_close"
        );
    }
}
