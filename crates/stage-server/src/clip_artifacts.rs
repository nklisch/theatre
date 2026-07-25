use crate::clip_analysis::{self, ClipGap, ClipScreenshot};
use crate::tcp::SessionState;
use jpeg_decoder::Decoder;
use rmcp::model::ErrorData as McpError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use temporal_vision::{
    ArtifactLabels, DeclaredGap, DifferenceMapLimits, DifferenceMapParameters, Frame,
    FrameSequence, FrequencyMode, IntegerScale, Marker, MeasurementParameters, MotionDecay,
    MotionHistoryParameters, NormalizationParameters, PixelDimensions, PixelFormat,
    ProcessingLimits, RenderLimits, Rgb8, StoryboardParameters, StoryboardTileLimit, TimePalette,
    Timestamp, generate_motion_history, generate_storyboard, normalize_sequence,
};
use tokio::sync::Mutex;

pub mod cache;
pub mod frames;
pub mod manifest;
pub mod params;

const TV_VERSION: &str = "1";

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

#[derive(Debug, Clone)]
pub struct ArtifactOutput {
    pub manifest: Value,
    pub png: Vec<u8>,
    pub cache: String,
}
fn err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(format!("visual artifact generation failed: {e}"), None)
}

/// A degradation condition the agent can act on, returned as a content-level
/// JSON error object (mirroring the `no_screenshots` pattern in clips.rs)
/// rather than a protocol error. The handler unwraps `data` into the response.
fn degraded(code: &str, detail: serde_json::Value) -> McpError {
    let mut obj = serde_json::Map::new();
    obj.insert("error".into(), code.into());
    if let serde_json::Value::Object(extra) = detail {
        obj.extend(extra);
    }
    McpError::internal_error(
        format!("{code}: visual artifact unavailable"),
        Some(serde_json::Value::Object(obj)),
    )
}

fn vision<T>(r: temporal_vision::Result<T>) -> Result<T, McpError> {
    r.map_err(|e| {
        degraded(
            "generation_failed",
            serde_json::json!({"code": e.code.as_str(), "message": e.to_string()}),
        )
    })
}
fn decode(s: &ClipScreenshot) -> Result<(PixelDimensions, Vec<u8>), McpError> {
    let mut d = Decoder::new(s.jpeg_data.as_slice());
    let rgb = d.decode().map_err(|e| {
        degraded(
            "decode_failed",
            serde_json::json!({"frame": s.frame, "message": e.to_string()}),
        )
    })?;
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

fn compact_params(
    at_frame: Option<u64>,
    at_time_ms: Option<u64>,
    reference_frame: Option<u64>,
    tile_limit: Option<u8>,
    inline_image: bool,
) -> Value {
    canonical(
        &json!({"at_frame": at_frame, "at_time_ms": at_time_ms, "reference_frame": reference_frame, "tile_limit": tile_limit, "inline_image": inline_image}),
    )
}
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(a, _)| *a);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k.clone(), canonical(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        _ => value.clone(),
    }
}
pub fn cache_key(kind: &str, params: &Value, fingerprint: &str) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    h.update(b"|");
    h.update(serde_json::to_vec(&canonical(params)).unwrap_or_default());
    h.update(b"|");
    h.update(fingerprint.as_bytes());
    h.update(b"|");
    h.update(TV_VERSION.as_bytes());
    format!("{:x}", h.finalize())
}
fn nearest_frame(frames: &[Frame<u64, Box<[u8]>>], frame: u64) -> usize {
    frames
        .iter()
        .enumerate()
        .min_by_key(|(_, f)| f.id().abs_diff(frame))
        .map(|(i, _)| i)
        .unwrap_or(0)
}
fn source_anchor(
    frames: &[Frame<u64, Box<[u8]>>],
    markers: &[(u64, u64, String, String)],
    fallback: u64,
) -> u64 {
    markers
        .iter()
        .find(|(_, _, source, _)| matches!(source.as_str(), "human" | "agent" | "code"))
        .map(|(frame, _, _, _)| *frame)
        .map(|f| *frames[nearest_frame(frames, f)].id())
        .unwrap_or(fallback)
}
fn budget_block(manifest: &Value, limit: u32, hard_cap: u32) -> Value {
    let used =
        stage_core::budget::estimate_tokens(serde_json::to_vec(manifest).unwrap_or_default().len());
    json!({"used": used, "limit": limit.min(hard_cap), "hard_cap": hard_cap})
}
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_artifact(
    state: &Arc<Mutex<SessionState>>,
    clip_id: Option<&str>,
    kind: &str,
    at_frame: Option<u64>,
    at_time_ms: Option<u64>,
    reference_frame: Option<u64>,
    tile_limit: Option<u8>,
    inline_image: bool,
    token_limit: u32,
    hard_cap: u32,
) -> Result<ArtifactOutput, McpError> {
    let session = clip_analysis::ClipSession::open(state, clip_id).await?;
    let kind = ArtifactKind::parse(kind)?;
    let shots = clip_analysis::list_screenshots(&session.db)?;
    let mut fp = Sha256::new();
    for s in &shots {
        fp.update(s.frame.to_le_bytes());
        fp.update(s.timestamp_ms.to_le_bytes());
        fp.update(s.size_bytes.to_le_bytes());
        if let Some(image) = clip_analysis::read_screenshot_near_frame(&session.db, s.frame)? {
            fp.update(&image.jpeg_data);
        }
    }
    let fingerprint = format!("{:x}", fp.finalize());
    let params = compact_params(
        at_frame,
        at_time_ms,
        reference_frame,
        tile_limit,
        inline_image,
    );
    let key = cache_key(kind.as_str(), &params, &fingerprint);
    if clip_analysis::artifacts_table_exists(&session.db)
        && let Ok((png, text)) = session.db.query_row(
            "SELECT png, manifest_json FROM artifacts WHERE cache_key = ?1",
            rusqlite::params![key],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?)),
        )
        && let Ok(mut manifest) = serde_json::from_str::<Value>(&text)
    {
        if let Some(image) = manifest.get_mut("image").and_then(Value::as_object_mut) {
            image.insert("cache".into(), json!("hit"));
        }
        return Ok(ArtifactOutput {
            manifest,
            png,
            cache: "hit".into(),
        });
    }
    if shots.is_empty() {
        return Err(degraded(
            "no_screenshots",
            serde_json::json!({"message": "this clip contains no visual frames"}),
        ));
    }
    let mut decoded = Vec::new();
    let mut expected = None;
    let mut mismatch = 0u64;
    for meta in shots.iter().take(4096) {
        let Some(shot) = clip_analysis::read_screenshot_near_frame(&session.db, meta.frame)? else {
            return Err(err(format!(
                "screenshot metadata frame {} has no image",
                meta.frame
            )));
        };
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
        return Err(degraded(
            "insufficient_frames",
            serde_json::json!({"usable": decoded.len()}),
        ));
    }
    let (w, h) = expected.ok_or_else(|| err("decoded screenshots have no dimensions"))?;
    let frames: Vec<Frame<u64, Box<[u8]>>> = decoded
        .into_iter()
        .map(|(f, ts, d, r)| {
            vision(Frame::new(
                f,
                Timestamp::from_nanos(ts.saturating_mul(1_000_000)),
                d,
                PixelFormat::Rgba8SrgbStraight,
                r.into_boxed_slice(),
            ))
        })
        .collect::<Result<_, _>>()?;
    let markers = read_markers(&session.db)?;
    let fallback = session.meta.started_at_frame.max(0) as u64
        + (session
            .meta
            .ended_at_frame
            .unwrap_or(session.meta.started_at_frame)
            .saturating_sub(session.meta.started_at_frame)
            .max(0) as u64
            / 2);
    let anchor_frame = at_frame
        .or_else(|| {
            at_time_ms.and_then(|t| {
                frames
                    .iter()
                    .min_by_key(|f| {
                        f.timestamp()
                            .as_nanos()
                            .abs_diff(t.saturating_mul(1_000_000))
                    })
                    .map(|f| *f.id())
            })
        })
        .unwrap_or_else(|| source_anchor(&frames, &markers, fallback));
    let anchor_idx = nearest_frame(&frames, anchor_frame);
    let ref_idx = reference_frame
        .map(|f| nearest_frame(&frames, f))
        .unwrap_or(anchor_idx);
    let interval = clip_analysis::screenshot_cadence_interval(&session.db)?;
    let explicit = clip_analysis::read_screenshot_gaps(&session.db)?;
    let gaps = if explicit.is_empty() {
        clip_analysis::infer_screenshot_gaps(&session.db, interval)?
    } else {
        explicit
    };
    let cadence_ms = shots
        .windows(2)
        .find_map(|p| {
            let df = p[1].frame.saturating_sub(p[0].frame);
            let dt = p[1].timestamp_ms.saturating_sub(p[0].timestamp_ms);
            (df > 0 && dt > 0).then_some(dt / df)
        })
        .unwrap_or(1);
    let tv_markers = markers
        .iter()
        .enumerate()
        .map(|(i, (_, ts, src, label))| {
            vision(Marker::new(
                format!("marker-{i}"),
                Timestamp::from_nanos(ts.saturating_mul(1_000_000)),
                src.clone(),
                if label.is_empty() {
                    "(unlabeled)".into()
                } else {
                    label.clone()
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let seq = vision(FrameSequence::new(
        frames,
        tv_markers,
        gaps_to_tv(&gaps, cadence_ms)?,
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
    let (png, mut manifest, extra) = match kind {
        ArtifactKind::Storyboard => {
            let a = generate_storyboard(
                "artifact",
                None,
                &seq,
                &normalized,
                StoryboardParameters::new(
                    seq.frames()[anchor_idx].timestamp(),
                    vision(StoryboardTileLimit::new(tile_limit.unwrap_or(8)))?,
                    MeasurementParameters::default(),
                    labels,
                    limits,
                ),
            )
            .map_err(err)?;
            let selected=a.selection().selected_frames().iter().map(|s| json!({"frame":s.frame_id(),"timestamp_ms":s.timestamp().as_nanos()/1_000_000,"reasons":s.reasons().iter().map(|r|r.as_str()).collect::<Vec<_>>()})).collect::<Vec<_>>();
            (
                a.storyboard().image().bytes().to_vec(),
                json!({"selected_frames":selected,"continuity_segments":a.selection().continuity_segment_count()}),
                json!({}),
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
            let plan = vision(temporal_vision::build_motion_history_plan(
                &seq,
                &normalized,
                &p,
            ))?;
            let a = generate_motion_history("artifact", &seq, &normalized, p).map_err(err)?;
            (
                a.image().bytes().to_vec(),
                json!({"reference_frame":seq.frames()[ref_idx].id(),"plan":{"continuity_segments":plan.continuity_segment_count(),"measured_pairs":plan.measured_pair_count(),"gap_pairs":plan.gap_pair_count()}}),
                json!({}),
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
            let a = temporal_vision::render_difference_map("artifact", &seq, &normalized, p)
                .map_err(err)?;
            (
                a.image().bytes().to_vec(),
                json!({"reference_frame":seq.frames()[ref_idx].id(),"mode":"count"}),
                json!({}),
            )
        }
    };
    let mut root = serde_json::Map::new();
    root.insert("clip_id".into(), json!(session.clip_id));
    root.insert("kind".into(), json!(kind.as_str()));
    root.insert("anchor_frame".into(), json!(seq.frames()[anchor_idx].id()));
    root.insert("frames_analyzed".into(), json!(seq.frames().len()));
    root.insert("subsampled".into(), json!(shots.len() > 4096));
    root.insert("cadence".into(),json!({"interval_frames":interval,"captured":shots.len(),"dropped":gaps.iter().map(|g|g.dropped).sum::<u64>(),"coverage":{"first_frame":shots.first().map(|s|s.frame),"last_frame":shots.last().map(|s|s.frame)}}));
    root.insert("gaps".into(), serde_json::to_value(&gaps).map_err(err)?);
    if !extra.is_null() {
        root.insert("extra".into(), extra);
    }
    root.extend(manifest.as_object_mut().cloned().unwrap_or_default());
    root.insert("dimension_mismatch_dropped".into(), json!(mismatch));
    if inline_image {
        let mut ih = Sha256::new();
        ih.update(&png);
        root.insert("image".into(),json!({"width":w,"height":h,"bytes":png.len(),"sha256":format!("{:x}",ih.finalize()),"cache":if clip_analysis::artifacts_table_exists(&session.db) { "stored" } else { "unavailable" }}));
    }
    let mut manifest = Value::Object(root);
    let budget = budget_block(&manifest, token_limit, hard_cap);
    manifest["budget"] = budget;
    let cache = if clip_analysis::artifacts_table_exists(&session.db) {
        let path =
            std::path::Path::new(&session.storage_path).join(format!("{}.sqlite", session.clip_id));
        match rusqlite::Connection::open(path) {
            Ok(rw) => {
                let text = serde_json::to_string(&manifest).map_err(err)?;
                let dims = json!({"width":w,"height":h}).to_string();
                if rw.execute("INSERT OR REPLACE INTO artifacts (cache_key,kind,params_json,manifest_json,dims,png,created_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7)",rusqlite::params![key,kind.as_str(),serde_json::to_string(&params).map_err(err)?,text,dims,png.as_slice(),now_ms()]).is_ok() {"stored"} else {"unavailable"}
            }
            Err(_) => "unavailable",
        }
    } else {
        "unavailable"
    };
    if let Some(image) = manifest.get_mut("image").and_then(Value::as_object_mut) {
        image.insert("cache".into(), json!(cache));
    }
    Ok(ArtifactOutput {
        manifest,
        png,
        cache: cache.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_canonical_and_fingerprint_sensitive() {
        let a = json!({"at_frame": 4, "options": {"b": 2, "a": 1}});
        let b = json!({"options": {"a": 1, "b": 2}, "at_frame": 4});
        assert_eq!(
            cache_key("storyboard", &a, "clip-a"),
            cache_key("storyboard", &b, "clip-a")
        );
        assert_ne!(
            cache_key("storyboard", &a, "clip-a"),
            cache_key("storyboard", &json!({"at_frame": 5}), "clip-a")
        );
        assert_ne!(
            cache_key("storyboard", &a, "clip-a"),
            cache_key("storyboard", &a, "clip-b")
        );
    }
}

fn read_markers(db: &rusqlite::Connection) -> Result<Vec<(u64, u64, String, String)>, McpError> {
    let mut s = db
        .prepare("SELECT frame,timestamp_ms,source,label FROM markers ORDER BY frame")
        .map_err(err)?;
    s.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)? as u64,
            r.get::<_, i64>(1)? as u64,
            r.get::<_, String>(2).unwrap_or_default(),
            r.get::<_, String>(3).unwrap_or_default(),
        ))
    })
    .map_err(err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(err)
}
