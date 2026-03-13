//! Default serde functions shared by Director MCP parameter structs.

/// Default parent/root path: `"."` (the root node of the scene).
pub fn default_root() -> String {
    ".".to_string()
}

/// Default boolean true — used for optional flags that default to enabled.
pub fn default_true() -> bool {
    true
}
