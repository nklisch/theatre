//! TCP wire protocol types shared between stage-server and stage-godot.

pub mod codec;
pub mod connection_state;
pub mod handshake;
#[cfg(feature = "mcp")]
pub mod mcp_helpers;
pub mod messages;
pub mod query;
pub mod query_dispatch;
pub mod recording;
pub mod static_classes;
pub mod variant_mapping;
