//! Shared spatial logic for Stage.
//!
//! Pure computation: bearing math, spatial indexing, delta engine, token budget.
//! No Godot API, no MCP API — testable standalone.

pub mod bearing;
pub mod budget;
pub mod cluster;
pub mod config;
pub mod delta;
pub mod index;
pub mod types;
pub mod watch;
