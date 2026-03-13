# Pattern: Serde Default Functions

Optional parameters with non-`Default` defaults use `#[serde(default = "fn_name")]` pointing to a small free function that returns the intended default value.

## Rationale

Rust's `Default` trait returns `0`, `false`, `""`, or `None` — wrong for most MCP parameters. The serde `default = "path"` attribute allows arbitrary default values without wrapping fields in `Option`. Spectator groups shared defaults in `defaults.rs`; Director and spectator-core define defaults inline near the struct.

## Examples

### Example 1: Shared defaults module (Spectator)
**File**: `crates/spectator-server/src/mcp/defaults.rs`
```rust
pub fn default_perspective() -> String { "camera".to_string() }
pub fn default_radius() -> f64 { 50.0 }
pub fn default_detail() -> String { "standard".to_string() }
pub fn default_k() -> usize { 5 }
pub fn default_query_radius() -> f64 { 20.0 }
```

Used by parameter structs in the same crate:
```rust
// crates/spectator-server/src/mcp/snapshot.rs:21,34,38
#[serde(default = "default_perspective")]
pub perspective: String,
#[serde(default = "default_radius")]
pub radius: f64,
#[serde(default = "default_detail")]
pub detail: String,
```

### Example 2: Inline defaults (Director)
**File**: `crates/director/src/mcp/node.rs:15-16`
```rust
#[serde(default = "default_root")]
pub parent_path: String,

// defined below the struct:
fn default_root() -> String { ".".to_string() }
```

### Example 3: Config defaults (spectator-core)
**File**: `crates/spectator-core/src/config.rs:45-80`
```rust
#[serde(default = "default_poll_interval")]
pub poll_interval: u64,
#[serde(default = "default_token_hard_cap")]
pub token_hard_cap: u32,
#[serde(default = "default_dashcam_enabled")]
pub dashcam_enabled: bool,

fn default_poll_interval() -> u64 { 1 }
fn default_token_hard_cap() -> u32 { 5000 }
fn default_dashcam_enabled() -> bool { true }
```

## When to Use
- Any MCP parameter that has a sensible non-zero/non-false/non-empty default
- Prefer over `Option<T>` when the field is always logically present (just optional to pass)
- Shared across multiple structs in the same crate → collect in `defaults.rs`
- Used in only one struct → define inline below the struct

## When NOT to Use
- Field is genuinely optional with no default (use `#[serde(default)] pub foo: Option<T>`)
- Default is `0`, `false`, `""`, or `None` (use plain `#[serde(default)]` which calls `Default::default()`)

## Common Violations
- Using `Option<T>` with `.unwrap_or(50.0)` in handler logic instead of a typed default — hide the default in the struct, not the handler
- Defining the default function far from the struct — always define it immediately below the struct or in `defaults.rs`
