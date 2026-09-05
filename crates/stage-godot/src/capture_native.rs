//! Context-local OpenGL pixel-pack transfer. No Godot objects enter this module.
//!
//! Every method is called only by RenderingServer render-thread callbacks. The
//! loader selects the implementation which actually has a current context, and
//! retains its libraries until the final context-valid retirement callback.
use libloading::Library;
use std::ffi::{CStr, c_char, c_void};

type CurrentContext = unsafe extern "system" fn() -> *mut c_void;
type ProcAddress = unsafe extern "system" fn(*const c_char) -> *const c_void;
type LoaderCandidate = (
    &'static [&'static str],
    &'static [u8],
    Option<&'static [u8]>,
);

struct Loader {
    libraries: Vec<Library>,
    current: CurrentContext,
    context: usize,
    get_proc: Option<ProcAddress>,
}

impl Loader {
    unsafe fn load() -> Result<Self, String> {
        // EGL may provide desktop GL or GLES, on either Wayland or X11. Only
        // fall through to GLX/WGL/CGL if that implementation owns this context.
        #[cfg(target_os = "linux")]
        let candidates: &[LoaderCandidate] = &[
            (
                &["libEGL.so.1", "libGLESv2.so.2", "libOpenGL.so.0"],
                b"eglGetCurrentContext\0",
                Some(b"eglGetProcAddress\0"),
            ),
            (
                &["libGL.so.1"],
                b"glXGetCurrentContext\0",
                Some(b"glXGetProcAddressARB\0"),
            ),
        ];
        #[cfg(target_os = "windows")]
        let candidates: &[LoaderCandidate] = &[
            (
                &["libEGL.dll", "libGLESv2.dll"],
                b"eglGetCurrentContext\0",
                Some(b"eglGetProcAddress\0"),
            ),
            (
                &["opengl32.dll"],
                b"wglGetCurrentContext\0",
                Some(b"wglGetProcAddress\0"),
            ),
        ];
        #[cfg(target_os = "macos")]
        let candidates: &[LoaderCandidate] = &[(
            &["/System/Library/Frameworks/OpenGL.framework/OpenGL"],
            b"CGLGetCurrentContext\0",
            None,
        )];
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let candidates: &[LoaderCandidate] = &[];
        for (names, current_name, proc_name) in candidates {
            let libraries: Vec<_> = names
                .iter()
                .filter_map(|name| unsafe { Library::new(name).ok() })
                .collect();
            let Some(current) = libraries.iter().find_map(|lib| unsafe {
                lib.get::<CurrentContext>(current_name).ok().map(|v| *v)
            }) else {
                continue;
            };
            let context = unsafe { current() } as usize;
            if context == 0 {
                continue;
            }
            let get_proc = proc_name.and_then(|name| {
                libraries
                    .iter()
                    .find_map(|lib| unsafe { lib.get::<ProcAddress>(name).ok().map(|v| *v) })
            });
            return Ok(Self {
                libraries,
                current,
                context,
                get_proc,
            });
        }
        Err("No supported current OpenGL context in the rendering callback".into())
    }

    fn is_current(&self) -> bool {
        unsafe { (self.current)() as usize == self.context }
    }

    unsafe fn address(&self, name: &CStr) -> Result<*const c_void, String> {
        if let Some(get_proc) = self.get_proc {
            let address = unsafe { get_proc(name.as_ptr()) };
            // WGL documents these failure sentinels in addition to null.
            if !address.is_null() && !matches!(address as usize, 1 | 2 | 3 | usize::MAX) {
                return Ok(address);
            }
        }
        for lib in &self.libraries {
            if let Ok(address) = unsafe { lib.get::<*const c_void>(name.to_bytes_with_nul()) } {
                return Ok(*address);
            }
        }
        Err(format!(
            "OpenGL entry point {} unavailable",
            name.to_string_lossy()
        ))
    }
}

macro_rules! functions {
    ($($name:ident: $ty:ty),* $(,)?) => {
        #[allow(non_snake_case)]
        struct Gl { loader: Loader, $( $name: $ty, )* }
        impl Gl {
            #[allow(non_snake_case)]
            unsafe fn load() -> Result<Self, String> {
                let loader = unsafe { Loader::load()? };
                $( let $name = unsafe { std::mem::transmute::<*const c_void, $ty>(loader.address(CStr::from_bytes_with_nul_unchecked(concat!(stringify!($name), "\0").as_bytes()))?) }; )*
                Ok(Self { loader, $( $name, )* })
            }
        }
    };
}
functions! {
    glGetIntegerv: unsafe extern "system" fn(u32, *mut i32),
    glGetString: unsafe extern "system" fn(u32) -> *const u8,
    glGetError: unsafe extern "system" fn() -> u32,
    glGenBuffers: unsafe extern "system" fn(i32, *mut u32),
    glBindBuffer: unsafe extern "system" fn(u32, u32),
    glBufferData: unsafe extern "system" fn(u32, isize, *const c_void, u32),
    glDeleteBuffers: unsafe extern "system" fn(i32, *const u32),
    glGenFramebuffers: unsafe extern "system" fn(i32, *mut u32),
    glBindFramebuffer: unsafe extern "system" fn(u32, u32),
    glFramebufferTexture2D: unsafe extern "system" fn(u32, u32, u32, u32, i32),
    glCheckFramebufferStatus: unsafe extern "system" fn(u32) -> u32,
    glDeleteFramebuffers: unsafe extern "system" fn(i32, *const u32),
    glPixelStorei: unsafe extern "system" fn(u32, i32),
    glReadPixels: unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut c_void),
    glFenceSync: unsafe extern "system" fn(u32, u32) -> *mut c_void,
    glClientWaitSync: unsafe extern "system" fn(*mut c_void, u32, u64) -> u32,
    glDeleteSync: unsafe extern "system" fn(*mut c_void),
    glMapBufferRange: unsafe extern "system" fn(u32, isize, isize, u32) -> *mut c_void,
    glUnmapBuffer: unsafe extern "system" fn(u32) -> u8,
    glFlush: unsafe extern "system" fn(),
}

const PACK: u32 = 0x88eb;
const READ_FRAMEBUFFER: u32 = 0x8ca8;
const PACK_STATES: [u32; 4] = [0x0d05, 0x0d02, 0x0d03, 0x0d04]; // alignment, row length, skip rows/pixels

/// Restores the exact engine bindings and pack layout, including error exits.
struct SavedState<'a> {
    gl: &'a Gl,
    framebuffer: i32,
    buffer: i32,
    pack: [i32; 4],
}
impl<'a> SavedState<'a> {
    unsafe fn save(gl: &'a Gl) -> Self {
        let mut state = Self {
            gl,
            framebuffer: 0,
            buffer: 0,
            pack: [0; 4],
        };
        unsafe {
            (gl.glGetIntegerv)(0x8caa, &mut state.framebuffer);
            (gl.glGetIntegerv)(0x88ed, &mut state.buffer);
            for (key, value) in PACK_STATES.iter().zip(state.pack.iter_mut()) {
                (gl.glGetIntegerv)(*key, value);
            }
        }
        state
    }
}
impl Drop for SavedState<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.gl.glBindFramebuffer)(READ_FRAMEBUFFER, self.framebuffer as u32);
            (self.gl.glBindBuffer)(PACK, self.buffer as u32);
            for (key, value) in PACK_STATES.iter().zip(self.pack) {
                (self.gl.glPixelStorei)(*key, value);
            }
        }
    }
}

/// Native resources are explicitly retired by the rendering callback, never by
/// Rust Drop on the main/encoder thread. GL deletion retires in-use storage; it
/// does not wait for completion or reuse an outstanding transfer's texture.
pub(super) struct NativeReadback {
    gl: Gl,
    buffer: u32,
    framebuffer: u32,
    fence: usize,
    length: usize,
}
impl NativeReadback {
    pub(super) fn new() -> Result<Self, String> {
        // SAFETY: invoked only from RenderingServer.call_on_render_thread.
        let gl = unsafe { Gl::load()? };
        if unsafe { (gl.glGetString)(0x1f02) }.is_null() {
            return Err("OpenGL context has no version".into());
        }
        let mut result = Self {
            gl,
            buffer: 0,
            framebuffer: 0,
            fence: 0,
            length: 0,
        };
        unsafe {
            (result.gl.glGenBuffers)(1, &mut result.buffer);
            (result.gl.glGenFramebuffers)(1, &mut result.framebuffer);
        }
        Ok(result)
    }

    pub(super) fn submit(&mut self, texture: u32, width: u32, height: u32) -> Result<(), String> {
        if !self.gl.loader.is_current() {
            return Err("OpenGL capture context changed".into());
        }
        if self.fence != 0 {
            return Err("OpenGL readback already pending".into());
        }
        let length = width as usize * height as usize * 4;
        let gl = &self.gl;
        unsafe {
            let _restore = SavedState::save(gl);
            (gl.glBindFramebuffer)(READ_FRAMEBUFFER, self.framebuffer);
            (gl.glFramebufferTexture2D)(READ_FRAMEBUFFER, 0x8ce0, 0x0de1, texture, 0);
            if (gl.glCheckFramebufferStatus)(READ_FRAMEBUFFER) != 0x8cd5 {
                return Err("Readback framebuffer incomplete".into());
            }
            (gl.glBindBuffer)(PACK, self.buffer);
            if self.length != length {
                (gl.glBufferData)(PACK, length as isize, std::ptr::null(), 0x88e1);
                self.length = length;
            }
            for (key, value) in PACK_STATES.iter().zip([1, 0, 0, 0]) {
                (gl.glPixelStorei)(*key, value);
            }
            (gl.glReadPixels)(
                0,
                0,
                width as i32,
                height as i32,
                0x1908,
                0x1401,
                std::ptr::null_mut(),
            );
            let error = (gl.glGetError)();
            if error != 0 {
                return Err(format!("OpenGL transfer error 0x{error:x}"));
            }
            self.fence = (gl.glFenceSync)(0x9117, 0) as usize;
            // Progress even if the window isn't swapping; no finish or wait.
            (gl.glFlush)();
            if self.fence == 0 {
                return Err("OpenGL fence creation failed".into());
            }
        }
        Ok(())
    }

    /// A single zero-timeout check, on a later recorder tick. Mapping is legal
    /// only after GL reports that the transfer has completed.
    pub(super) fn poll(&mut self) -> Result<Option<Vec<u8>>, String> {
        if !self.gl.loader.is_current() {
            return Err("OpenGL capture context changed".into());
        }
        let gl = &self.gl;
        unsafe {
            match (gl.glClientWaitSync)(self.fence as *mut c_void, 0, 0) {
                0x911b => return Ok(None), // TIMEOUT_EXPIRED
                0x911a | 0x911c => {}      // ALREADY_SIGNALED / CONDITION_SATISFIED
                _ => return Err("OpenGL fence polling failed".into()),
            }
            let _restore = SavedState::save(gl);
            (gl.glBindBuffer)(PACK, self.buffer);
            let mapped = (gl.glMapBufferRange)(PACK, 0, self.length as isize, 1);
            if mapped.is_null() {
                return Err("Completed OpenGL pixel buffer could not be mapped".into());
            }
            let pixels = std::slice::from_raw_parts(mapped.cast::<u8>(), self.length).to_vec();
            let valid = (gl.glUnmapBuffer)(PACK) != 0;
            (gl.glDeleteSync)(self.fence as *mut c_void);
            self.fence = 0;
            if !valid {
                return Err("OpenGL pixel buffer contents invalidated".into());
            }
            Ok(Some(pixels))
        }
    }

    pub(super) fn retire(&mut self) {
        // A lost context owns its own eventual cleanup. Never delete names in a
        // different context, where the same integers may identify engine data.
        if !self.gl.loader.is_current() {
            return;
        }
        unsafe {
            if self.fence != 0 {
                (self.gl.glDeleteSync)(self.fence as *mut c_void);
                self.fence = 0;
            }
            (self.gl.glDeleteBuffers)(1, &self.buffer);
            (self.gl.glDeleteFramebuffers)(1, &self.framebuffer);
        }
        self.buffer = 0;
        self.framebuffer = 0;
    }
}
