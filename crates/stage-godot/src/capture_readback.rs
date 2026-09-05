//! Main-thread drawable ownership and bounded render-callback dispatch.
use crate::capture_native::NativeReadback;
use godot::classes::{RenderingServer, image::Format, rendering_server::TextureDrawableFormat};
use godot::prelude::*;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

pub(super) struct Pixels {
    pub rgba: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub completion_copy_ms: f64,
    pub latency_ms: f64,
}

use crate::capture_render_call::render_call;

enum Reply {
    Submitted(Result<(), String>, f64),
    Polled(Result<Option<Vec<u8>>, String>, f64),
}

pub(super) struct Readback {
    texture: Rid,
    native_handle: u32,
    shader: Rid,
    material: Rid,
    size: (u32, u32),
    native: Arc<Mutex<Option<NativeReadback>>>,
    reply: Option<mpsc::Receiver<Reply>>,
    pending: bool,
    started: Instant,
    submitted_tick: u64,
    pub submission_ms: f64,
}

impl Default for Readback {
    fn default() -> Self {
        Self {
            texture: Rid::Invalid,
            native_handle: 0,
            shader: Rid::Invalid,
            material: Rid::Invalid,
            size: (0, 0),
            native: Arc::new(Mutex::new(None)),
            reply: None,
            pending: false,
            started: Instant::now(),
            submitted_tick: 0,
            submission_ms: 0.0,
        }
    }
}

impl Readback {
    fn blit(&mut self, source: Rid, size: (u32, u32)) {
        let mut server = RenderingServer::singleton();
        if self.shader == Rid::Invalid {
            self.shader = server.shader_create();
            server.shader_set_code(self.shader, "shader_type texture_blit;\nrender_mode blend_disabled;\nuniform sampler2D source_texture0 : hint_blit_source0, filter_linear, repeat_disable;\nvoid blit() { COLOR0 = texture(source_texture0, UV); }");
            self.material = server.material_create();
            server.material_set_shader(self.material, self.shader);
        }
        if self.size != size {
            if self.texture != Rid::Invalid {
                server.free_rid(self.texture);
            }
            self.texture = server.texture_drawable_create(
                size.0 as i32,
                size.1 as i32,
                TextureDrawableFormat::RGBA8,
            );
            self.size = size;
            self.native_handle = 0;
        }
        server.texture_drawable_blit_rect(
            &Array::from(&[self.texture]),
            Rect2i::new(Vector2i::ZERO, Vector2i::new(size.0 as i32, size.1 as i32)),
            self.material,
            Color::WHITE,
            &Array::from(&[source]),
        );
    }

    /// Must be called only after recorder admission. The destination is never
    /// replaced while a submitted callback or GPU transfer may reference it.
    pub fn submit(&mut self, source: Rid, size: (u32, u32), tick: u64) -> Result<(), String> {
        if self.pending {
            return Err("readback_pending".into());
        }
        self.started = Instant::now();
        self.blit(source, size);
        // This getter can synchronize the rendering command queue. The owned
        // drawable name is stable until resize, so do not pay that cost per shot.
        if self.native_handle == 0 {
            self.native_handle =
                RenderingServer::singleton().texture_get_native_handle(self.texture) as u32;
        }
        let handle = self.native_handle;
        if handle == 0 {
            return Err("Drawable has no native OpenGL texture".into());
        }
        self.submission_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let native = Arc::clone(&self.native);
        let (tx, rx) = mpsc::channel();
        let callback = render_call("stage_readback_submit", move || {
            let start = Instant::now();
            let result = native
                .lock()
                .map_err(|_| "Readback state poisoned".to_string())
                .and_then(|mut state| {
                    if state.is_none() {
                        *state = Some(NativeReadback::new()?);
                    }
                    state
                        .as_mut()
                        .ok_or("Readback initialization failed".to_string())?
                        .submit(handle, size.0, size.1)
                });
            let _ = tx.send(Reply::Submitted(
                result,
                start.elapsed().as_secs_f64() * 1000.0,
            ));
        });
        self.pending = true;
        self.submitted_tick = tick;
        self.reply = Some(rx);
        RenderingServer::singleton().call_on_render_thread(&callback);
        Ok(())
    }

    /// No callback result wait, positive fence timeout, or immediate mapping.
    pub fn poll(&mut self, tick: u64) -> Option<Result<Pixels, String>> {
        if !self.pending || tick <= self.submitted_tick {
            return None;
        }
        if let Some(rx) = &self.reply {
            match rx.try_recv() {
                Ok(Reply::Submitted(result, ms)) => {
                    self.submission_ms += ms;
                    self.reply = None;
                    if let Err(error) = result {
                        self.pending = false;
                        return Some(Err(error));
                    }
                }
                Ok(Reply::Polled(result, ms)) => {
                    self.reply = None;
                    match result {
                        Ok(Some(rgba)) => {
                            self.pending = false;
                            return Some(Ok(Pixels {
                                rgba,
                                width: self.size.0 as u16,
                                height: self.size.1 as u16,
                                completion_copy_ms: ms,
                                latency_ms: self.started.elapsed().as_secs_f64() * 1000.0,
                            }));
                        }
                        Err(error) => {
                            self.pending = false;
                            return Some(Err(error));
                        }
                        Ok(None) => {}
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = false;
                    return Some(Err("Readback callback disconnected".into()));
                }
            }
        }
        let native = Arc::clone(&self.native);
        let (tx, rx) = mpsc::channel();
        let callback = render_call("stage_readback_poll", move || {
            let start = Instant::now();
            let result = native
                .lock()
                .map_err(|_| "Readback state poisoned".to_string())
                .and_then(|mut state| {
                    state
                        .as_mut()
                        .ok_or("Readback not initialized".to_string())?
                        .poll()
                });
            let _ = tx.send(Reply::Polled(
                result,
                start.elapsed().as_secs_f64() * 1000.0,
            ));
        });
        self.reply = Some(rx);
        RenderingServer::singleton().call_on_render_thread(&callback);
        None
    }

    /// Explicit recovery only: GPU reduction saves transfer bytes but this
    /// texture_2d_get intentionally stalls for the GPU and is not async.
    pub fn synchronous(&mut self, source: Rid, size: (u32, u32)) -> Result<Pixels, String> {
        if self.pending {
            return Err("readback_pending".into());
        }
        let start = Instant::now();
        self.blit(source, size);
        let mut image = RenderingServer::singleton()
            .texture_2d_get(self.texture)
            .ok_or("Drawable pixels unavailable")?;
        image.convert(Format::RGBA8);
        Ok(Pixels {
            rgba: image.get_data().to_vec(),
            width: size.0 as u16,
            height: size.1 as u16,
            completion_copy_ms: 0.0,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

impl Drop for Readback {
    fn drop(&mut self) {
        let native = Arc::clone(&self.native);
        // Dispatch ordering retains native ownership until older callbacks have
        // finished. free_rid is ordered AFTER retirement on the same server.
        let callback = render_call("stage_readback_retire", move || {
            if let Ok(mut state) = native.lock() {
                if let Some(state) = state.as_mut() {
                    state.retire();
                }
                *state = None;
            }
        });
        let mut server = RenderingServer::singleton();
        server.call_on_render_thread(&callback);
        for rid in [self.texture, self.material, self.shader] {
            if rid != Rid::Invalid {
                server.free_rid(rid);
            }
        }
    }
}

pub(super) fn reduced_size(width: u32, height: u32, maximum: u32) -> (u32, u32) {
    let largest = width.max(height);
    if largest <= maximum {
        (width.max(1), height.max(1))
    } else {
        (
            (width as u64 * maximum as u64 / largest as u64).max(1) as u32,
            (height as u64 * maximum as u64 / largest as u64).max(1) as u32,
        )
    }
}
