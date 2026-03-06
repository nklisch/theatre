use godot::prelude::*;

mod tcp_server;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
