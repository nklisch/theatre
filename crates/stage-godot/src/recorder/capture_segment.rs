//! Contiguous capture segments owned by StageRecorder. No separate service or
//! consumer-specific adapter owns buffers or persistence.
use super::*;
use stage_protocol::capture::{PhaseConfig, Retention, SavePolicy};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SegmentContext {
    target_scene: String,
    readiness: stage_protocol::capture::Readiness,
    operator: stage_protocol::capture::Operator,
    sources: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    project_metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

pub(super) struct Metric {
    render_frame: u64,
    physics_frame: u64,
    timestamp_ms: u64,
    elapsed_usec: u64,
    interval_usec: u64,
    data: String,
}

pub(super) struct Segment {
    context: SegmentContext,
    config: PhaseConfig,
    phase: String,
    recording_id: String,
    id: String,
    started: Instant,
    started_frame: u64,
    started_ms: u64,
    ended_frame: Option<u64>,
    ended_ms: Option<u64>,
    elapsed_usec: u64,
    stop_reason: Option<String>,
    metrics: VecDeque<Metric>,
    metric_bytes: usize,
    marker_bytes: usize,
    previous_sample: Option<Instant>,
    observer_total_usec: u64,
    observer_max_usec: u64,
    dimensions: u32,
    pending_limit: Option<&'static str>,
    trigger_deadline: Option<Instant>,
    last_system_marker: Option<Instant>,
}

impl Segment {
    fn active(&self) -> bool {
        self.stop_reason.is_none()
    }

    fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "state": if self.active() { "recording" } else { "review" },
            "recording_id": self.recording_id, "segment_id": self.id,
            "phase": self.phase, "config": self.config,
            "context": self.context,
            "frame_range": [self.started_frame, self.ended_frame.unwrap_or_else(current_physics_frame)],
            "started_at_ms": self.started_ms, "ended_at_ms": self.ended_ms,
            "active_duration_usec": if self.active() { self.started.elapsed().as_micros() as u64 } else { self.elapsed_usec },
            "metric_samples": self.metrics.len(), "metric_payload_bytes": self.metric_bytes,
            "observer_total_usec": self.observer_total_usec, "observer_max_usec": self.observer_max_usec,
            "stop_reason": self.stop_reason,
            "pending_post_window": self.trigger_deadline.is_some() && self.active(),
            "post_window_shortened": self.trigger_deadline.is_some() && self.stop_reason.as_deref().is_some_and(|reason| reason != "marker_post_window"),
        })
    }
}

impl StageRecorder {
    pub(super) fn begin_segment(
        &mut self,
        config: PhaseConfig,
        phase: &str,
        recording_id: &str,
        context: SegmentContext,
    ) -> Result<serde_json::Value, String> {
        config.validate(phase)?;
        if self.segment.is_some() {
            return Err("Finish and keep or discard the current segment first".into());
        }
        if !config.enabled {
            return Err("The selected capture phase is off".into());
        }
        if !matches!(phase, "startup" | "play") {
            return Err("phase must be startup or play".into());
        }
        let metadata_bytes = serde_json::to_vec(&context.project_metadata)
            .map_err(|e| format!("Invalid project metadata: {e}"))?
            .len();
        if context.project_metadata.len() > 32 || metadata_bytes > 64 * 1024 {
            return Err("Project metadata exceeds 32 providers or 64 KiB encoded payload".into());
        }
        let mut recorder_config = self.dashcam_config.clone();
        recorder_config.movement_nodes = config.movement_nodes.clone();
        recorder_config.input_actions = config.input_actions.clone();
        // Startup targets do not exist yet. Validate their portable names above;
        // resolve them while sampling, without inventing missing-node evidence.
        if phase == "play" {
            crate::movement_capture::validate_targets(&self.base().clone(), &mut recorder_config)?;
        }
        if recording_id.len() > 128
            || !recording_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("Invalid recording identifier".into());
        }
        self.set_capture_enabled_internal(false);
        self.pending_silent_markers.clear();
        self.gap_ledger = GapLedger::default();
        self.capture_probe = CaptureProbe::default();
        self.dashcam_config.capture_interval = config.spatial_interval;
        self.dashcam_config.screenshot_enabled = config.images;
        self.dashcam_config.screenshot_interval_frames = config.image_interval;
        self.dashcam_config.screenshot_max_dimension = config.image_size;
        self.dashcam_config.byte_cap_mb = config.payload_mib;
        self.dashcam_config.screenshot_byte_cap_mb = config.payload_mib;
        self.dashcam_config.anomaly_enabled = config.anomaly_enabled;
        self.dashcam_config.anomaly_min_proportion = config.anomaly_min_proportion;
        self.dashcam_config.anomaly_relative_factor = config.anomaly_relative_factor;
        self.dashcam_config.anomaly_sustained_frames = config.anomaly_sustained_frames;
        self.dashcam_config.anomaly_cooldown_sec = config.anomaly_cooldown_sec;
        self.dashcam_config.anomaly_noise_floor = config.anomaly_noise_floor;
        self.dashcam_config.dense_burst_enabled = false;
        self.dashcam_config.screenshot_readback = config.image_readback;
        self.dashcam_config.movement_nodes = recorder_config.movement_nodes;
        self.dashcam_config.input_actions = recorder_config.input_actions;
        let id = format!("clip_{}_{:08x}", current_time_ms(), rand_u32());
        self.segment = Some(Segment {
            context,
            config,
            phase: phase.into(),
            recording_id: if recording_id.is_empty() {
                id.clone()
            } else {
                recording_id.into()
            },
            id,
            started: Instant::now(),
            started_frame: current_physics_frame(),
            started_ms: current_time_ms(),
            ended_frame: None,
            ended_ms: None,
            elapsed_usec: 0,
            stop_reason: None,
            metrics: VecDeque::new(),
            metric_bytes: 0,
            marker_bytes: 0,
            previous_sample: None,
            observer_total_usec: 0,
            observer_max_usec: 0,
            dimensions: 0,
            pending_limit: None,
            trigger_deadline: None,
            last_system_marker: None,
        });
        self.set_capture_enabled_internal(true);
        Ok(self.segment_status())
    }

    pub(super) fn segment_status(&self) -> serde_json::Value {
        let mut status = self
            .segment
            .as_ref()
            .map(Segment::status)
            .unwrap_or_else(|| serde_json::json!({"state":"idle"}));
        status["spatial_samples"] = serde_json::json!(self.ring_buffer.len());
        status["image_samples"] = serde_json::json!(self.screenshot_ring.len());
        status["last_saved_clip"] = serde_json::json!(self.last_saved_clip);
        status["last_save_error"] = serde_json::json!(self.last_save_error);
        status["screenshot_capture"] = self.screenshot_capture_status();
        status["capture_probe"] = self.capture_probe.json();
        status["screenshot_gaps"] =
            serde_json::from_str(&self.get_screenshot_gaps_json().to_string())
                .unwrap_or(serde_json::Value::Null);
        status["anomaly"] = serde_json::from_str(&self.get_anomaly_status_json().to_string())
            .unwrap_or(serde_json::Value::Null);
        status["storage_budget_mib"] = serde_json::json!(1024);
        status
    }

    pub(super) fn segment_active(&self) -> bool {
        self.segment.as_ref().is_some_and(Segment::active)
    }

    pub(super) fn segment_scope(&self) -> Option<&[String]> {
        self.segment.as_ref().map(|s| s.config.scope.as_slice())
    }

    pub(super) fn segment_spatial_enabled(&self) -> bool {
        self.segment.as_ref().is_none_or(|s| s.config.spatial)
    }

    pub(super) fn segment_set_dimensions(&mut self, dimensions: u32) {
        if let Some(segment) = self.segment.as_mut() {
            segment.dimensions = dimensions;
        }
    }

    pub(super) fn check_segment_bounds(&mut self) {
        if let Some(reason) = self
            .segment
            .as_ref()
            .filter(|s| s.active())
            .and_then(|s| s.pending_limit)
        {
            let _ = self.finish_segment(reason);
            return;
        }
        if let Some(segment) = self
            .segment
            .as_ref()
            .filter(|s| s.active() && s.trigger_deadline.is_some_and(|d| Instant::now() >= d))
        {
            let config = segment.config.clone();
            let recording_id = segment.recording_id.clone();
            let phase = segment.phase.clone();
            let context = segment.context.clone();
            if self.finish_segment("marker_post_window").is_ok() && self.segment.is_none() {
                let _ = self.begin_segment(config, &phase, &recording_id, context);
            }
            return;
        }
        let Some(segment) = self
            .segment
            .as_ref()
            .filter(|s| s.active() && s.config.retention == Retention::Session)
        else {
            return;
        };
        let reason = if let Some(reason) = segment.pending_limit {
            Some(reason)
        } else if segment.started.elapsed().as_secs() >= segment.config.duration_secs as u64 {
            Some("duration_limit")
        } else if segment.metrics.len()
            + self.ring_buffer.len()
            + self.screenshot_ring.len()
            + self.pending_silent_markers.len()
            >= segment.config.max_records as usize
        {
            Some("record_limit")
        } else if segment.metric_bytes
            + segment.marker_bytes
            + self.ring_buffer_bytes
            + self.screenshot_ring_bytes
            >= segment.config.payload_mib as usize * 1024 * 1024
        {
            Some("payload_limit")
        } else {
            None
        };
        if let Some(reason) = reason {
            let _ = self.finish_segment(reason);
        }
    }

    pub(super) fn finish_segment(&mut self, reason: &str) -> Result<serde_json::Value, String> {
        if !self.segment_active() {
            return Ok(self.segment_status());
        }
        self.poll_screenshot_readback();
        self.drain_encoded_shots();
        // Retain only completed images. The existing generation ledger records
        // outstanding work as missing rather than assigning it to the next segment.
        self.invalidate_capture_generation();
        self.dashcam_config.enabled = false;
        self.dashcam_state = DashcamState::Disabled;
        let segment = self.segment.as_mut().ok_or("No active segment")?;
        segment.elapsed_usec = segment.started.elapsed().as_micros() as u64;
        segment.ended_frame = Some(current_physics_frame());
        segment.ended_ms = Some(current_time_ms());
        segment.stop_reason = Some(reason.chars().take(256).collect());
        if segment.config.save == SavePolicy::OnStop
            || (segment.config.save == SavePolicy::OnTrigger && segment.trigger_deadline.is_some())
        {
            self.keep_segment("")
        } else {
            Ok(self.segment_status())
        }
    }

    pub(super) fn discard_segment(&mut self) -> Result<serde_json::Value, String> {
        if self.segment_active() {
            return Err("Stop the active segment before discarding it".into());
        }
        self.segment = None;
        self.last_save_error = None;
        self.set_capture_enabled_internal(false);
        self.pending_silent_markers.clear();
        Ok(self.segment_status())
    }

    pub(super) fn keep_segment(&mut self, note: &str) -> Result<serde_json::Value, String> {
        let segment = self.segment.as_ref().ok_or("No segment to keep")?;
        if segment.active() {
            return Err("Stop the segment before keeping it".into());
        }
        let directory = std::path::PathBuf::from(globalize_path("user://stage_recordings/"));
        let path = directory.join(format!("{}.sqlite", segment.id));
        let mut reserved = false;
        let saved = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            std::fs::create_dir_all(&directory)?;
            let retained_bytes: u64 = std::fs::read_dir(&directory)?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sqlite"))
                .map(|entry| entry.metadata().map(|meta| meta.len()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum();
            let payload = segment.metric_bytes
                + segment.marker_bytes
                + self.ring_buffer_bytes
                + self.screenshot_ring_bytes;
            // Admission reserves an estimate for SQLite pages, not an exact global
            // allocation guarantee across simultaneous processes.
            if retained_bytes
                .saturating_add(payload as u64)
                .saturating_add(1024 * 1024)
                > 1024 * 1024 * 1024
            {
                return Err("Capture storage budget reached (1024 MiB). Delete selected saved clips before retrying.".into());
            }
            drop(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?,
            );
            reserved = true;
            let mut db = Connection::open(&path)?;
            // Bound the committed SQLite file too, not only its encoded inputs.
            // Concurrent processes still use independent admission snapshots.
            let page_size: u64 = db.query_row("PRAGMA page_size", [], |row| row.get(0))?;
            db.pragma_update(
                None,
                "max_page_count",
                (1024 * 1024 * 1024u64).saturating_sub(retained_bytes) / page_size,
            )?;
            db.execute_batch(SCHEMA_SQL)?;
            let tx = db.transaction()?;
            let mut capture = segment.status();
            capture["format_version"] = serde_json::json!(2);
            capture["runtime"] = serde_json::json!(crate::runtime_identity::identity());
            capture["note"] = serde_json::json!(note.chars().take(4096).collect::<String>());
            capture["spatial_frame_count"] = serde_json::json!(self.ring_buffer.len());
            capture["screenshot_frame_count"] = serde_json::json!(self.screenshot_ring.len());
            capture["screenshot_capture"] = self.screenshot_capture_status();
            tx.execute("INSERT INTO recording (id,name,started_at_frame,ended_at_frame,started_at_ms,ended_at_ms,scene_dimensions,physics_ticks_per_sec,capture_config,created_at_unix_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![segment.id, format!("{} segment", segment.phase), segment.started_frame, segment.ended_frame,
                    segment.started_ms, segment.ended_ms, segment.dimensions, self.physics_fps, capture.to_string(), current_time_ms()])?;
            {
                let mut insert = tx.prepare_cached("INSERT INTO metrics (render_frame,physics_frame,timestamp_ms,elapsed_usec,interval_usec,data) VALUES (?1,?2,?3,?4,?5,?6)")?;
                for sample in &segment.metrics {
                    insert.execute(rusqlite::params![
                        sample.render_frame,
                        sample.physics_frame,
                        sample.timestamp_ms,
                        sample.elapsed_usec,
                        sample.interval_usec,
                        sample.data
                    ])?;
                }
            }
            {
                let mut insert = tx.prepare_cached(
                    "INSERT INTO frames (frame,timestamp_ms,data) VALUES (?1,?2,?3)",
                )?;
                let mut camera = tx.prepare_cached("INSERT INTO camera_frames (frame,timestamp_ms,camera_path,data) VALUES (?1,?2,?3,?4)")?;
                for frame in &self.ring_buffer {
                    insert.execute(rusqlite::params![
                        frame.frame,
                        frame.timestamp_ms,
                        frame.data
                    ])?;
                    if let Some(value) = &frame.camera {
                        camera.execute(rusqlite::params![
                            frame.frame,
                            frame.timestamp_ms,
                            value.camera_path,
                            rmp_serde::to_vec(value)?
                        ])?;
                    }
                }
            }
            {
                let mut insert = tx.prepare_cached("INSERT INTO screenshots (frame,timestamp_ms,image_data,width,height) VALUES (?1,?2,?3,?4,?5)")?;
                for shot in &self.screenshot_ring {
                    insert.execute(rusqlite::params![
                        shot.frame,
                        shot.timestamp_ms,
                        shot.jpeg_data,
                        shot.width,
                        shot.height
                    ])?;
                }
                let mut gaps = tx.prepare_cached("INSERT INTO screenshot_gaps (start_frame,end_frame,reason,dropped) VALUES (?1,?2,?3,?4)")?;
                for gap in &self.gap_ledger.gaps {
                    gaps.execute(rusqlite::params![
                        gap.start_frame,
                        gap.end_frame,
                        gap.reason,
                        gap.dropped
                    ])?;
                }
            }
            {
                let mut insert = tx.prepare_cached(
                    "INSERT INTO markers (frame,timestamp_ms,source,label) VALUES (?1,?2,?3,?4)",
                )?;
                for marker in &self.pending_silent_markers {
                    insert.execute(rusqlite::params![
                        marker.frame,
                        marker.timestamp_ms,
                        marker.source,
                        marker.label
                    ])?;
                }
            }
            tx.commit()?;
            Ok(
                serde_json::json!({"clip_id":segment.id,"recording_id":segment.recording_id,"phase":segment.phase,
                    "runtime":capture["runtime"], "frame_range":capture["frame_range"],
                    "scene_at_save":crate::runtime_identity::status(Some(self.base().get_tree())).current_scene,
                    "capture":capture}),
            )
        })();
        match saved {
            Ok(clip) => {
                self.last_saved_clip = Some(clip);
                self.last_save_error = None;
                self.segment = None;
                self.set_capture_enabled_internal(false);
                self.pending_silent_markers.clear();
                let hints = std::path::PathBuf::from(globalize_path("res://.stage"));
                if let Err(error) = std::fs::create_dir_all(&hints).and_then(|()| {
                    std::fs::write(
                        hints.join("clip_storage_path"),
                        directory.to_string_lossy().as_bytes(),
                    )
                }) {
                    tracing::warn!("Clip saved but offline storage hint failed: {error}");
                }
                Ok(self.segment_status())
            }
            Err(error) => {
                if reserved && let Err(cleanup) = std::fs::remove_file(&path) {
                    tracing::warn!("Cannot remove incomplete segment: {cleanup}");
                }
                let error = format!("Segment was not saved; draft retained: {error}");
                self.last_save_error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn sample_segment_metrics(
        &mut self,
        data: &str,
        callback_usec: u64,
    ) -> Result<(), String> {
        if !self
            .segment
            .as_ref()
            .is_some_and(|s| s.active() && s.config.metrics)
        {
            return Ok(());
        }
        if !self.segment_admit(data.len() + 5 * std::mem::size_of::<u64>()) {
            return Ok(());
        }
        let Some(segment) = self
            .segment
            .as_mut()
            .filter(|s| s.active() && s.config.metrics)
        else {
            return Ok(());
        };
        let value: serde_json::Value =
            serde_json::from_str(data).map_err(|e| format!("Invalid metrics: {e}"))?;
        if !value.is_object() {
            return Err("Metrics must be a dictionary of explicit provider values".into());
        }
        let now = Instant::now();
        let interval_usec = segment
            .previous_sample
            .replace(now)
            .map(|last| now.duration_since(last).as_micros() as u64)
            .unwrap_or(0);
        let metric = Metric {
            render_frame: Engine::singleton().get_process_frames(),
            physics_frame: current_physics_frame(),
            timestamp_ms: current_time_ms(),
            elapsed_usec: segment.started.elapsed().as_micros() as u64,
            interval_usec,
            data: value.to_string(),
        };
        // Count the encoded value and fixed scalar fields, not process memory.
        let bytes = metric.data.len() + 5 * std::mem::size_of::<u64>();
        let cap = segment.config.payload_mib as usize * 1024 * 1024;
        if bytes > cap {
            return Err("One metric sample exceeds the capture payload budget".into());
        }
        segment.metric_bytes += bytes;
        segment.metrics.push_back(metric);
        segment.observer_total_usec = segment.observer_total_usec.saturating_add(callback_usec);
        segment.observer_max_usec = segment.observer_max_usec.max(callback_usec);
        if segment.config.retention == Retention::Rolling {
            while segment.metrics.len() > segment.config.max_records as usize
                || segment.metric_bytes > cap
                || segment.metrics.front().is_some_and(|m| {
                    segment.started.elapsed().as_micros() as u64 - m.elapsed_usec
                        > segment.config.duration_secs as u64 * 1_000_000
                })
            {
                if let Some(old) = segment.metrics.pop_front() {
                    segment.metric_bytes -= old.data.len() + 5 * std::mem::size_of::<u64>();
                } else {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Admit before appending. Session evidence is never silently evicted;
    /// rolling evidence evicts the oldest channel sample under one shared cap.
    pub(super) fn segment_admit(&mut self, bytes: usize) -> bool {
        let Some(segment) = self.segment.as_mut() else {
            return true;
        };
        if !segment.active() || segment.pending_limit.is_some() {
            return false;
        }
        let cap = segment.config.payload_mib as usize * 1024 * 1024;
        if bytes > cap {
            segment.pending_limit = Some("oversized_sample");
            return false;
        }
        loop {
            let count = segment.metrics.len()
                + self.ring_buffer.len()
                + self.screenshot_ring.len()
                + self.pending_silent_markers.len();
            let payload = segment.metric_bytes
                + segment.marker_bytes
                + self.ring_buffer_bytes
                + self.screenshot_ring_bytes;
            let oldest = [
                segment.metrics.front().map(|s| s.timestamp_ms),
                self.ring_buffer.front().map(|s| s.timestamp_ms),
                self.screenshot_ring.front().map(|s| s.timestamp_ms),
                self.pending_silent_markers.first().map(|s| s.timestamp_ms),
            ]
            .into_iter()
            .enumerate()
            .filter_map(|(channel, time)| time.map(|time| (time, channel)))
            .min();
            let expired = oldest.is_some_and(|(time, _)| {
                current_time_ms().saturating_sub(time) > segment.config.duration_secs as u64 * 1000
            });
            if count < segment.config.max_records as usize
                && payload.saturating_add(bytes) <= cap
                && (segment.config.retention != Retention::Rolling || !expired)
            {
                return true;
            }
            if segment.config.retention == Retention::Session {
                segment.pending_limit = Some(if count >= segment.config.max_records as usize {
                    "record_limit"
                } else {
                    "payload_limit"
                });
                return false;
            }
            match oldest.map(|(_, channel)| channel) {
                Some(0) => {
                    if let Some(old) = segment.metrics.pop_front() {
                        segment.metric_bytes -= old.data.len() + 5 * std::mem::size_of::<u64>();
                    }
                }
                Some(1) => {
                    if let Some(old) = self.ring_buffer.pop_front() {
                        self.ring_buffer_bytes -= old.encoded_bytes();
                    }
                }
                Some(2) => {
                    if let Some(old) = self.screenshot_ring.pop_front() {
                        self.screenshot_ring_bytes -= old.jpeg_data.len();
                    }
                }
                Some(3) => {
                    let old = self.pending_silent_markers.remove(0);
                    segment.marker_bytes -= old.label.len() + old.source.len() + 16;
                }
                _ => return false,
            }
        }
    }

    pub(super) fn segment_marker(&mut self, source: &str, label: &str, tier: &str) {
        let now = Instant::now();
        let Some(segment) = self.segment.as_ref().filter(|s| s.active()) else {
            return;
        };
        // Suppressed markers must not evict evidence or trip a session bound.
        if tier == "system"
            && segment
                .last_system_marker
                .is_some_and(|last| now.duration_since(last).as_secs() < 5)
        {
            return;
        }
        let source: String = source.chars().take(64).collect();
        let label: String = label.chars().take(1024).collect();
        if !self.segment_admit(source.len() + label.len() + 16) {
            return;
        }
        let Some(segment) = self.segment.as_mut().filter(|s| s.active()) else {
            return;
        };
        if tier == "system" {
            segment.last_system_marker = Some(now);
        }
        if tier != "silent"
            && segment.config.retention == Retention::Rolling
            && segment.config.save == SavePolicy::OnTrigger
        {
            // A bounded post-window; repeated markers cannot postpone it forever.
            segment
                .trigger_deadline
                .get_or_insert(now + std::time::Duration::from_secs(5));
        }
        let frame = current_physics_frame();
        if self.pending_silent_markers.len() < 1024 {
            segment.marker_bytes += source.len() + label.len() + 16;
            self.pending_silent_markers.push(DashcamTrigger {
                frame,
                timestamp_ms: current_time_ms(),
                source: source.clone(),
                label: label.clone(),
            });
        }
        if tier != "silent" {
            self.base_mut().emit_signal(
                "marker_added",
                &[
                    (frame as i64).to_variant(),
                    GString::from(&source).to_variant(),
                    GString::from(&label).to_variant(),
                ],
            );
        }
    }
}

pub(super) fn reply(result: Result<serde_json::Value, String>) -> GString {
    let value = result.unwrap_or_else(|error| serde_json::json!({"error":error}));
    GString::from(&value.to_string())
}
