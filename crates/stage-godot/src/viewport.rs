use godot::classes::{DisplayServer, Engine, Marshalls, SceneTree, Time};
use godot::prelude::*;
use stage_protocol::viewport::{
    ViewportAvailability, ViewportCapture, ViewportMetadata, ViewportParams,
    ViewportUnavailableReason,
};

/// Explicit-demand capture only. Godot readback and native encoding stay on the
/// calling engine thread; this path neither reads nor enables the recorder.
pub fn capture(
    tree: Option<Gd<SceneTree>>,
    params: &ViewportParams,
) -> Result<ViewportCapture, String> {
    let mut metadata = ViewportMetadata {
        identity: crate::runtime_identity::identity().clone(),
        readback_physics_frame: Engine::singleton().get_physics_frames(),
        frames_drawn: Engine::singleton().get_frames_drawn() as u64,
        timestamp_ms: Time::singleton().get_ticks_msec(),
        availability: ViewportAvailability::Unavailable {
            reason: ViewportUnavailableReason::NoViewport,
        },
    };
    let unavailable = |mut metadata: ViewportMetadata, reason| {
        metadata.availability = ViewportAvailability::Unavailable { reason };
        ViewportCapture {
            metadata,
            image_base64: None,
        }
    };
    // The headless renderer has no texture pixels; do not ask its dummy texture
    // backend to read back an invalid render target.
    if DisplayServer::singleton().get_name() == "headless" {
        return Ok(unavailable(metadata, ViewportUnavailableReason::Headless));
    }
    let Some(tree) = tree else {
        return Ok(unavailable(metadata, ViewportUnavailableReason::NoViewport));
    };
    let viewport = tree.get_root();
    let Some(texture) = viewport.get_texture() else {
        return Ok(unavailable(metadata, ViewportUnavailableReason::NoViewport));
    };
    let Some(mut image) = texture.get_image() else {
        return Ok(unavailable(
            metadata,
            ViewportUnavailableReason::EmptyPixels,
        ));
    };
    if image.is_empty() || image.get_width() <= 0 || image.get_height() <= 0 {
        return Ok(unavailable(
            metadata,
            ViewportUnavailableReason::EmptyPixels,
        ));
    }
    let source_width = image.get_width() as u32;
    let source_height = image.get_height() as u32;
    let scale = (params.max_dimension as f64 / source_width.max(source_height) as f64).min(1.0);
    let width = ((source_width as f64 * scale).round() as u32).max(1);
    let height = ((source_height as f64 * scale).round() as u32).max(1);
    if width != source_width || height != source_height {
        image.resize(width as i32, height as i32);
    }
    let jpeg = image.save_jpg_to_buffer_ex().quality(0.8).done();
    if jpeg.is_empty() {
        return Err("Godot could not encode the viewport as JPEG".into());
    }
    metadata.availability = ViewportAvailability::Available {
        width,
        height,
        source_width,
        source_height,
    };
    Ok(ViewportCapture {
        metadata,
        image_base64: Some(Marshalls::singleton().raw_to_base64(&jpeg).to_string()),
    })
}
