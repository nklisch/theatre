use godot::prelude::*;

mod action_handler;
mod collector;
mod query_handler;
mod recorder;
mod recording_handler;
mod tcp_server;

struct StageExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StageExtension {}
