use crate::clip_analysis::{self, ClipGap, ClipScreenshot};
use crate::tcp::SessionState;
use jpeg_decoder::Decoder;
use rmcp::model::ErrorData as McpError;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use temporal_vision::{
    ArtifactLabels, DeclaredGap, DifferenceMapLimits, DifferenceMapParameters, Frame,
    FrameSequence, FrequencyMode, IntegerScale, Marker, MeasurementParameters, MotionDecay,
    MotionHistoryParameters, NormalizationParameters, PixelDimensions, PixelFormat,
    ProcessingLimits, RenderLimits, Rgb8, StoryboardParameters, StoryboardTileLimit, TimePalette,
    Timestamp, generate_motion_history, generate_storyboard, normalize_sequence,
    render_difference_map,
};
use tokio::sync::Mutex;

pub mod cache;
pub mod frames;
pub mod manifest;
pub mod params;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Storyboard,
    MotionHistory,
    DifferenceMap,
}

impl ArtifactKind {
    pub fn parse(s: &str) -> Result<Self, McpError> {
        match s {
            "storyboard" => Ok(Self::Storyboard),
            "motion_history" => Ok(Self::MotionHistory),
            "difference_map" => Ok(Self::DifferenceMap),
            _ => Err(McpError::invalid_params(
                format!(
                    "Unknown artifact '{s}'; expected storyboard, motion_history, or difference_map"
                ),
                None,
            )),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::Storyboard => "storyboard",
            Self::MotionHistory => "motion_history",
            Self::DifferenceMap => "difference_map",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactOutput {
    pub manifest: serde_json::Value,
    pub png: Vec<u8>,
    pub cache: String,
}

fn err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(format!("visual artifact generation failed: {e}"), None)
}
fn vision<T>(r: temporal_vision::Result<T>) -> Result<T, McpError> {
    r.map_err(err)
}

fn decode(s: &ClipScreenshot) -> Result<(PixelDimensions, Vec<u8>), McpError> {
    let mut d = Decoder::new(s.jpeg_data.as_slice());
    let rgb = d
        .decode()
        .map_err(|e| err(format!("JPEG decode failed at frame {}: {e}", s.frame)))?;
    let info = d.info().ok_or_else(|| err("JPEG has no dimensions"))?;
    let dims = PixelDimensions::new(info.width as u32, info.height as u32).map_err(err)?;
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for p in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    Ok((dims, rgba))
}

fn gaps_to_tv(gaps: &[ClipGap], cadence_ms: u64) -> Result<Vec<DeclaredGap<String>>, McpError> {
    gaps.iter()
        .enumerate()
        .map(|(i, g)| {
            let a = Timestamp::from_nanos(
                g.start_frame
                    .saturating_mul(cadence_ms)
                    .saturating_mul(1_000_000),
            );
            let b = Timestamp::from_nanos(
                g.end_frame
                    .saturating_mul(cadence_ms)
                    .saturating_mul(1_000_000),
            );
            vision(temporal_vision::TimeRange::new(a, b)).and_then(|range| {
                vision(DeclaredGap::new(
                    format!("gap-{i}"),
                    range,
                    &g.reason,
                    std::num::NonZeroU64::new(g.dropped),
                ))
            })
        })
        .collect()
}

/// Generate a bounded artifact from a saved clip. All pixel work is synchronous and occurs
/// after the clip has been opened, so the live capture path is never involved.
pub async fn generate_artifact(
    state: &Arc<Mutex<SessionState>>,
    clip_id: Option<&str>,
    kind: &str,
    at_frame: Option<u64>,
    at_time_ms: Option<u64>,
    reference_frame: Option<u64>,
    tile_limit: Option<u8>,
) -> Result<ArtifactOutput, McpError> {
    let session = clip_analysis::ClipSession::open(state, clip_id).await?;
    let kind = ArtifactKind::parse(kind)?;
    let interval = 4u64;
    let shots = clip_analysis::list_screenshots(&session.db)?;
    let fingerprint = format!(
        "{}:{}:{}",
        shots.len(),
        shots.first().map(|s| s.frame).unwrap_or(0),
        shots.last().map(|s| s.frame).unwrap_or(0)
    );
    let cache_key = cache_key(
        kind.as_str(),
        &json!({"at_frame":at_frame,"at_time_ms":at_time_ms,"reference_frame":reference_frame,"tile_limit":tile_limit}),
        &fingerprint,
    );
    if clip_analysis::artifacts_table_exists(&session.db)
        && let Ok((png, manifest_json)) = session.db.query_row(
            "SELECT png, manifest_json FROM artifacts WHERE cache_key = ?1",
            rusqlite::params![cache_key],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
        )
        && let Ok(manifest) = serde_json::from_str(&manifest_json)
    {
        return Ok(ArtifactOutput {
            manifest,
            png,
            cache: "hit".into(),
        });
    }
    if shots.is_empty() {
        return Err(McpError::internal_error(
            "no_screenshots: this clip contains no visual frames",
            None,
        ));
    }
    let mut decoded = Vec::new();
    let mut expected: Option<(u32, u32)> = None;
    let mut mismatch = 0u64;
    for meta in shots.iter().take(4096) {
        let shot = clip_analysis::read_screenshot_near_frame(&session.db, meta.frame)?
            .expect("metadata came from screenshots");
        let (dims, rgba) = decode(&shot)?;
        if expected.is_none() {
            expected = Some((dims.width(), dims.height()));
        }
        if expected != Some((dims.width(), dims.height())) {
            mismatch += 1;
            continue;
        }
        decoded.push((shot.frame, shot.timestamp_ms, dims, rgba));
    }
    if decoded.len() < 3 {
        return Err(McpError::internal_error(
            "insufficient_frames: at least three compatible screenshots are required",
            None,
        ));
    }
    let (w, h) = expected.unwrap();
    let mut frames = Vec::new();
    for (frame, ts, dims, rgba) in decoded {
        frames.push(vision(Frame::new(
            frame,
            Timestamp::from_nanos(ts.saturating_mul(1_000_000)),
            dims,
            PixelFormat::Rgba8SrgbStraight,
            rgba.into_boxed_slice(),
        ))?);
    }
    let anchor_frame = at_frame
        .or_else(|| {
            at_time_ms.and_then(|t| {
                frames
                    .iter()
                    .min_by_key(|f| f.timestamp().as_nanos().abs_diff(t * 1_000_000))
                    .map(|f| *f.id())
            })
        })
        .unwrap_or(
            session.meta.started_at_frame.max(0) as u64
                + ((session
                    .meta
                    .ended_at_frame
                    .unwrap_or(session.meta.started_at_frame)
                    - session.meta.started_at_frame)
                    .max(0) as u64
                    / 2),
        );
    let anchor_idx = frames
        .iter()
        .position(|f| *f.id() >= anchor_frame)
        .unwrap_or(frames.len() / 2);
    let ref_idx = reference_frame
        .and_then(|r| frames.iter().position(|f| *f.id() == r))
        .unwrap_or(anchor_idx.min(frames.len() - 1));
    let explicit = clip_analysis::read_screenshot_gaps(&session.db)?;
    let gaps = if explicit.is_empty() {
        clip_analysis::infer_screenshot_gaps(&session.db, interval)?
    } else {
        explicit
    };
    let markers = read_markers(&session.db)?;
    let tv_markers = markers
        .into_iter()
        .enumerate()
        .map(|(i, (f, ts, src, label))| {
            vision(Marker::new(
                format!("marker-{i}"),
                Timestamp::from_nanos(ts * 1_000_000),
                src,
                if label.is_empty() {
                    "(unlabeled)".into()
                } else {
                    label
                },
            ))
            .map(|m| (f, m))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let marker_values = tv_markers.iter().map(|(_, m)| m.clone()).collect();
    let cadence_ms = shots
        .windows(2)
        .find_map(|pair| {
            let delta = pair[1].timestamp_ms.saturating_sub(pair[0].timestamp_ms);
            (delta > 0).then_some(delta / pair[1].frame.saturating_sub(pair[0].frame).max(1))
        })
        .unwrap_or(1);
    let tv_gaps = gaps_to_tv(&gaps, cadence_ms)?;
    let seq = vision(FrameSequence::new(
        frames,
        marker_values,
        tv_gaps,
        None,
        None,
    ))?;
    let normalized = vision(normalize_sequence(
        &seq,
        NormalizationParameters::new(
            Rgb8::new(0, 0, 0),
            None,
            IntegerScale::IDENTITY,
            ProcessingLimits::default(),
        ),
    ))?;
    let limits = RenderLimits::new(
        NonZeroU32::new(2048).unwrap(),
        NonZeroU32::new(2048).unwrap(),
        NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
        NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
    );
    let labels = vision(ArtifactLabels::new(
        format!("{} — {}", kind.as_str(), session.clip_id),
        format!("clip {}", session.clip_id),
    ))?;
    let tv = match kind {
        ArtifactKind::Storyboard => {
            let tiles = vision(StoryboardTileLimit::new(tile_limit.unwrap_or(8)))?;
            let a = generate_storyboard(
                "artifact",
                None,
                &seq,
                &normalized,
                StoryboardParameters::new(
                    seq.frames()[anchor_idx].timestamp(),
                    tiles,
                    MeasurementParameters::default(),
                    labels,
                    limits,
                ),
            )
            .map_err(err)?;
            (
                a.storyboard().image().bytes().to_vec(),
                serde_json::to_value(a.storyboard().manifest()).map_err(err)?,
            )
        }
        ArtifactKind::MotionHistory => {
            let p = MotionHistoryParameters::new(
                ref_idx,
                MeasurementParameters::default(),
                MotionDecay::default(),
                160,
                Rgb8::new(255, 80, 40),
                Rgb8::new(255, 255, 255),
                labels,
                limits,
            );
            let a = generate_motion_history("artifact", &seq, &normalized, p).map_err(err)?;
            (
                a.image().bytes().to_vec(),
                serde_json::to_value(a.manifest()).map_err(err)?,
            )
        }
        ArtifactKind::DifferenceMap => {
            let lim = DifferenceMapLimits::new(
                NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
                NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            );
            let p = DifferenceMapParameters::new(
                ref_idx,
                FrequencyMode::Count,
                TimePalette::Spectral,
                None,
                MeasurementParameters::default(),
                Rgb8::new(0, 0, 0),
                lim,
            );
            let a = render_difference_map("artifact", &seq, &normalized, p).map_err(err)?;
            (
                a.image().bytes().to_vec(),
                serde_json::to_value(a.manifest()).map_err(err)?,
            )
        }
    };
    let mut manifest = tv.1;
    if let Some(o) = manifest.as_object_mut() {
        o.insert("stage".into(),json!({"clip_id":session.clip_id,"dimension_mismatch_dropped":mismatch,"subsampled":shots.len()>4096,"gaps":gaps,"cadence_interval_frames":interval,"image":{"width":w,"height":h},"budget":{"manifest_only":true}}));
    }
    let png = tv.0;
    let cache = if clip_analysis::artifacts_table_exists(&session.db) {
        let path =
            std::path::Path::new(&session.storage_path).join(format!("{}.sqlite", session.clip_id));
        if let Ok(rw) = rusqlite::Connection::open(path) {
            let manifest_json = serde_json::to_string(&manifest).unwrap_or_default();
            let dims = json!({"width": w, "height": h}).to_string();
            if rw.execute("INSERT OR REPLACE INTO artifacts (cache_key, kind, params_json, manifest_json, dims, png, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", rusqlite::params![cache_key, kind.as_str(), "{}", manifest_json, dims, png.as_slice(), 0i64]).is_ok() { "stored" } else { "unavailable" }
        } else {
            "unavailable"
        }
    } else {
        "unavailable"
    };
    Ok(ArtifactOutput {
        manifest,
        png,
        cache: cache.into(),
    })
}

fn read_markers(db: &rusqlite::Connection) -> Result<Vec<(u64, u64, String, String)>, McpError> {
    if !clip_analysis::screenshots_table_exists(db) { /* no-op */ }
    let mut s = db
        .prepare("SELECT frame,timestamp_ms,source,label FROM markers ORDER BY frame")
        .map_err(err)?;
    let rows = s
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, i64>(1)? as u64,
                r.get::<_, String>(2).unwrap_or_default(),
                r.get::<_, String>(3).unwrap_or_default(),
            ))
        })
        .map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}

pub fn cache_key(kind: &str, params: &serde_json::Value, fingerprint: &str) -> String {
    let canonical = serde_json::to_vec(params).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    h.update(b"|");
    h.update(canonical);
    h.update(b"|");
    h.update(fingerprint.as_bytes());
    h.update(b"|1");
    format!("{:x}", h.finalize())
}
