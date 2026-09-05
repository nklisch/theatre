//! Narrow native callable adapter for RenderingServer's rendering thread.
//!
//! gdext 0.5.5's high-level callable return conversion accesses main-thread-only
//! binding tables. Its experimental-threads feature conflicts with lazy tables.
//! Use Godot's public callable_custom_create2 interface instead: creation stays
//! on the main thread and the callback uses only owned Rust data and a cached
//! variant_new_nil pointer (which initializes local return storage, not objects).
use godot::{
    builtin::Callable,
    sys::{self, GodotFfi},
};
use std::sync::Mutex;

type Operation = Box<dyn FnOnce() + Send>;
struct RenderCall {
    name: &'static str,
    operation: Mutex<Option<Operation>>,
    return_nil: sys::GDExtensionInterfaceVariantNewNil,
}

pub(super) fn render_call(
    name: &'static str,
    operation: impl FnOnce() + Send + 'static,
) -> Callable {
    static TOKEN: u8 = 0;
    let data = Box::new(RenderCall {
        name,
        operation: Mutex::new(Some(Box::new(operation))),
        return_nil: sys::interface_fn!(variant_new_nil),
    });
    let mut info = sys::GDExtensionCallableCustomInfo2 {
        callable_userdata: Box::into_raw(data).cast(),
        token: (&TOKEN as *const u8).cast_mut().cast(),
        object_id: 0,
        call_func: Some(invoke),
        free_func: Some(free),
        is_valid_func: None,
        hash_func: None,
        equal_func: None,
        less_than_func: None,
        to_string_func: None,
        get_argument_count_func: None,
    };
    // SAFETY: Godot copies info and owns its boxed userdata until free_func.
    // Both creation and the resulting Callable's main-thread ownership follow
    // gdext's ordinary rules; only Godot dispatches its native callback.
    unsafe {
        Callable::new_with_uninit(|ptr| sys::interface_fn!(callable_custom_create2)(ptr, &mut info))
    }
}

unsafe extern "C" fn invoke(
    userdata: *mut std::ffi::c_void,
    _arguments: *const sys::GDExtensionConstVariantPtr,
    _argument_count: sys::GDExtensionInt,
    result: sys::GDExtensionVariantPtr,
    error: *mut sys::GDExtensionCallError,
) {
    // SAFETY: userdata is held alive by the invoking Godot callable. It is never
    // borrowed as a Godot object; callbacks only consume their owned operation.
    let call = unsafe { &*userdata.cast::<RenderCall>() };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Ok(mut operation) = call.operation.lock()
            && let Some(operation) = operation.take()
        {
            operation();
        }
    }));
    // Never unwind through Godot's C boundary. A panic drops the result sender,
    // so the main-thread owner sees a disconnected callback and reports a gap.
    if outcome.is_err() {
        eprintln!("[Stage] Native render callback {} failed", call.name);
    }
    unsafe {
        (call.return_nil)(result.cast());
        (*error).error = if outcome.is_ok() {
            sys::GDEXTENSION_CALL_OK
        } else {
            sys::GDEXTENSION_CALL_ERROR_INVALID_METHOD
        };
        (*error).argument = 0;
        (*error).expected = 0;
    }
}

unsafe extern "C" fn free(userdata: *mut std::ffi::c_void) {
    // SAFETY: Godot invokes this once, after the last callable reference and all
    // invocations are gone. No Godot value or native GL destructor is in data.
    unsafe {
        drop(Box::from_raw(userdata.cast::<RenderCall>()));
    }
}
