# Pattern: Serde Default Functions

Optional parameters with non-`Default` defaults use `#[serde(default = "fn_name")]` pointing to a small free function that returns the intended default value.

## Rationale

Rust's `Default` trait returns `0`, `false`, `""`, or `None` — wrong for most MCP parameters. The serde `default = "path"` attribute allows arbitrary default values without wrapping fields in `Option`. Stage groups shared defaults in `mcp/defaults.rs`; Director has its own `mcp/defaults.rs`; stage-core defines defaults inline in `config.rs`.

## Examples

### Example 1: Shared defaults module (Stage)
**File**: `crates/stage-server/src/mcp/defaults.rs`
```rust
pub fn default_radius() -> f64 { 50.0 }
pub fn default_k() -> usize { 5 }
pub fn default_query_radius() -> f64 { 20.0 }
```

Used by parameter structs in the same crate:
```rust
#[serde(default = "default_radius")]
pub radius: f64,
#[serde(default = "default_k")]
pub k: usize,
```

Note: `perspective` and `detail` use `#[serde(default)]` with `#[derive(Default)]` on their enum types (`PerspectiveMode`, `DetailLevel`) rather than named default functions.

### Example 2: Shared defaults module (Director)
**File**: `crates/director/src/mcp/defaults.rs`
```rust
pub fn default_root() -> String { ".".to_string() }
```

Used by parameter structs (`crates/director/src/mcp/node.rs`):
```rust
use super::defaults::default_root;

#[serde(default = "default_root")]
pub parent_path: String,
```

### Example 3: Config defaults (stage-core)
**File**: `crates/stage-core/src/config.rs:49-63`
```rust
#[serde(default = "default_poll_interval")]
pub poll_interval: u32,
#[serde(default = "default_token_hard_cap")]
pub token_hard_cap: u32,

fn default_poll_interval() -> u32 { 1 }
fn default_token_hard_cap() -> u32 { 5000 }
```

Dashcam settings are not session-config defaults — they are recorder-owned runtime settings applied through `DashcamConfigPatch`. Do not add them back to `SessionConfig`.

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
- Defining the default function far from the struct — keep it in the struct's module or the crate's `defaults.rs`
