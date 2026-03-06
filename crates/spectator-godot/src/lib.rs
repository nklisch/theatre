use godot::prelude::*;

mod collector;
mod query_handler;
mod tcp_server;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
