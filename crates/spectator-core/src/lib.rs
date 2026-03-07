//! Shared spatial logic for Spectator.
//!
//! Pure computation: bearing math, spatial indexing, delta engine, token budget.
//! No Godot API, no MCP API — testable standalone.

pub mod bearing;
pub mod budget;
pub mod cluster;
pub mod index;
pub mod types;
