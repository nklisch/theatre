use godot::prelude::*;

mod action_handler;
mod capture_native;
mod capture_readback;
mod capture_render_call;
mod collector;
mod movement_capture;
mod query_handler;
mod recorder;
mod recording_handler;
pub mod runtime_identity;
mod tcp_server;
mod viewport;

struct StageExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StageExtension {}
