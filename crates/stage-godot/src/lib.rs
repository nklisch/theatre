use godot::prelude::*;

mod action_handler;
mod collector;
mod query_handler;
mod recorder;
mod recording_handler;
pub mod runtime_identity;
mod tcp_server;
mod viewport;

struct StageExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StageExtension {}
