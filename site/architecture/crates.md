# Crate Structure

Theatre's Rust workspace contains 5 crates. Each has a specific scope and dependency set designed to keep concerns separated.

## `spectator-protocol`

**Type**: Library (`lib`)
**Purpose**: Shared wire format types between server and GDExtension

This crate owns the TCP message types — the structs that are serialized to JSON and sent across the wire. It has no Godot dependencies and no MCP dependencies — just `serde` and the codec.

**Key contents**:
- `codec.rs` — length-prefix framing (sync + async via feature flag)
- `messages.rs` — all request/response type definitions
- `CodecError` — framing error type

**Dependency rules**:
- Depends on: `serde`, `serde_json`, `tokio` (optional, async feature)
- Depended on by: `spectator-server`, `spectator-godot`

The codec is shared rather than duplicated to ensure both sides always use the same framing implementation. A framing bug fixed in the codec is fixed for both sides simultaneously.

## `spectator-core`

**Type**: Library (`lib`)
**Purpose**: Pure spatial logic — no Godot, no MCP

This crate contains all reasoning that operates on spatial data but does not require Godot engine APIs or MCP infrastructure:

- **Budget trimming**: given a list of nodes and a token budget, select the highest-priority nodes to include
- **Frame diffing**: given two frame snapshots, compute which properties changed
- **Spatial queries**: geometry (radius search, bounding box, nearest) over node position data
- **Clip analysis**: reading clip files, frame range queries, condition filtering
- **Indexing**: spatial index structures for fast nearest-neighbor queries

**Dependency rules**:
- Depends on: `serde`, standard math utilities, no external heavy deps
- Depended on by: `spectator-server`
- Does NOT depend on: `spectator-protocol`, `spectator-godot`, any MCP crate

Keeping core logic here makes it testable without Godot or MCP infrastructure. Most unit tests in Theatre live in `spectator-core`.

## `spectator-godot`

**Type**: `cdylib` (Godot GDExtension)
**Purpose**: Collects spatial data from the running Godot engine

This is the crate that compiles to `libspectator_godot.so`. It uses `gdext` to register GDExtension classes that Godot can instantiate.

**Key classes**:
- `SpectatorTCPServer`: manages the TCP listener, handles the connection lifecycle, serializes/deserializes messages using `spectator-protocol`
- `SpectatorCollector`: called in `_physics_process`, walks the scene tree and writes to a ring buffer
- `SpectatorRecorder`: writes frame data to clip files on disk

**Dependency rules**:
- Depends on: `gdext`, `spectator-protocol`, `serde_json`
- Does NOT depend on: `spectator-core` (no spatial reasoning in the addon)
- Does NOT depend on: any MCP crates

The no-`spectator-core` rule keeps the GDExtension lean. The addon collects raw data; all analysis happens in the server.

### GDExtension version targeting

The crate targets `api-4-5` with `lazy-function-tables` enabled. The `api-4-5` flag requires Godot 4.5+ at runtime (API version ≤ runtime version). The `lazy-function-tables` feature defers method hash validation to first call rather than on load, providing forward compatibility with Godot 4.6+ without panicking when method hashes change between Godot versions in classes the extension never uses.

To target a newer API, bump `api-4-5` to `api-4-6` in `Cargo.toml` once godot-rust adds that feature flag.

## `spectator-server`

**Type**: Binary (`bin`)
**Purpose**: MCP server that bridges AI agents to the running Godot game

This is the binary your agent talks to via stdio. It:
- Implements the MCP protocol using `rmcp`
- Maintains a persistent TCP connection to `spectator-godot`
- Translates MCP tool calls into protocol requests
- Applies `spectator-core` logic to responses (budgeting, diffing, queries)
- Logs activity to the editor dock

**Dependency rules**:
- Depends on: `spectator-protocol`, `spectator-core`, `rmcp`, `tokio`, `tracing`, `anyhow`
- Does NOT depend on: `spectator-godot` (no GDExtension code in the server)

**Key modules**:
- `tools/` — one file per MCP tool, each implementing the tool handler
- `session.rs` — TCP connection management and request-response matching
- `activity.rs` — Activity logging to editor dock
- `budget.rs` — Response size measurement and trimming

## `director`

**Type**: Binary (`bin`)
**Purpose**: MCP server for scene/resource modification

The director crate implements the Director MCP tools. It communicates with the GDScript addon (not a GDExtension) via TCP.

**Dependency rules**:
- Depends on: `rmcp`, `tokio`, `tracing`, `anyhow`, `serde`
- No dependency on any spectator crate

**Backend routing** (`backend.rs`):
1. Try TCP connect to port 6550 (editor plugin)
2. Try TCP connect to port 6551 (daemon)
3. Fall back to spawning `godot --headless` one-shot

Each backend implements the same `Backend` trait, so tool handlers are backend-agnostic.

## `theatre-cli`

**Type**: Binary (`bin`, produces `theatre` executable)
**Purpose**: Unified CLI for installation, project setup, and deployment

The CLI replaces manual build-copy-configure workflows with four commands: `install`, `init`, `deploy`, `enable`. It has no runtime dependencies on Godot or MCP — just filesystem operations and cargo invocations.

**Key commands**:
- `theatre install` — builds all crates in release mode, copies binaries to `~/.local/bin/` and addon templates to `~/.local/share/theatre/`
- `theatre init <project>` — interactive project setup: copies addons, generates `.mcp.json`, enables plugins
- `theatre deploy <project...>` — rebuilds from source and updates target projects
- `theatre enable <project>` — toggles plugins in `project.godot`

**Dependency rules**:
- Depends on: `clap`, `dialoguer`, `console`, `serde_json`, `anyhow`
- No dependency on any spectator, director, rmcp, tokio, or gdext crate
- All operations are synchronous (`std::process::Command` for cargo builds)

## Workspace layout

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/spectator-protocol",
    "crates/spectator-core",
    "crates/spectator-godot",
    "crates/spectator-server",
    "crates/director",
    "crates/theatre-cli",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
anyhow = "1"
```

Shared dependency versions are defined once in the workspace and referenced with `{ workspace = true }` in each crate.

## Dependency graph

```
spectator-godot  ──────────────────────────┐
                                            ▼
spectator-protocol ──────────────── spectator-server
                                            │
spectator-core   ──────────────────────────┘

director ─── (no spectator deps)

theatre-cli ─── (no spectator/director/MCP deps, only clap + filesystem)
```

The diamond dependency (both `spectator-godot` and `spectator-server` depend on `spectator-protocol`) is intentional — they both need the same wire format types.
