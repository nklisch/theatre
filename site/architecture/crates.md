---
description: "Theatre's Rust crate structure — addon, server, protocol, and shared library organization and dependencies."
---

# Crate Structure

Theatre's Rust workspace contains 5 crates. Each has a specific scope and dependency set designed to keep concerns separated.

## `stage-protocol`

**Type**: Library (`lib`)
**Purpose**: Shared wire format types between server and GDExtension

This crate owns the TCP message types — the structs that are serialized to JSON and sent across the wire. It has no Godot dependencies and no MCP dependencies — just `serde` and the codec.

**Key contents**:
- `codec.rs` — length-prefix framing (sync + async via feature flag)
- `messages.rs` — all request/response type definitions
- `CodecError` — framing error type

**Dependency rules**:
- Depends on: `serde`, `serde_json`, `tokio` (optional, async feature)
- Depended on by: `stage-server`, `stage-godot`

The codec is shared rather than duplicated to ensure both sides always use the same framing implementation. A framing bug fixed in the codec is fixed for both sides simultaneously.

## `stage-core`

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
- Depended on by: `stage-server`
- Does NOT depend on: `stage-protocol`, `stage-godot`, any MCP crate

Keeping core logic here makes it testable without Godot or MCP infrastructure. Most unit tests in Theatre live in `stage-core`.

## `stage-godot`

**Type**: `cdylib` (Godot GDExtension)
**Purpose**: Collects spatial data from the running Godot engine

This is the crate that compiles to `libstage_godot.so`. It uses `gdext` to register GDExtension classes that Godot can instantiate.

**Key classes**:
- `StageTCPServer`: manages the TCP listener, handles the connection lifecycle, serializes/deserializes messages using `stage-protocol`
- `StageCollector`: called in `_physics_process`, walks the scene tree and writes to a ring buffer
- `StageRecorder`: writes frame data to clip files on disk

**Dependency rules**:
- Depends on: `gdext`, `stage-protocol`, `serde_json`
- Does NOT depend on: `stage-core` (no spatial reasoning in the addon)
- Does NOT depend on: any MCP crates

The no-`stage-core` rule keeps the GDExtension lean. The addon collects raw data; all analysis happens in the server.

### GDExtension version targeting

The crate targets `api-4-5` with `lazy-function-tables` enabled. The `api-4-5` flag requires Godot 4.5+ at runtime (API version ≤ runtime version). The `lazy-function-tables` feature defers method hash validation to first call rather than on load, providing forward compatibility with Godot 4.6+ without panicking when method hashes change between Godot versions in classes the extension never uses.

To target a newer API, bump `api-4-5` to `api-4-6` in `Cargo.toml` once godot-rust adds that feature flag.

## `stage-server`

**Type**: Binary (crate: `stage-server`, binary: `stage`)
**Purpose**: MCP server + CLI that bridges AI agents to the running Godot game

This is the binary your agent talks to. It supports two modes:
- `stage serve` — MCP server on stdio
- `stage <tool> '<json>'` — one-shot CLI mode

It:
- Implements the MCP protocol using `rmcp` (serve mode)
- Maintains a persistent TCP connection to `stage-godot` (serve) or connects once (CLI)
- Translates tool calls into protocol requests
- Applies `stage-core` logic to responses (budgeting, diffing, queries)
- Logs activity to the editor dock (serve mode only)

**Dependency rules**:
- Depends on: `stage-protocol`, `stage-core`, `rmcp`, `tokio`, `tracing`, `anyhow`
- Does NOT depend on: `stage-godot` (no GDExtension code in the server)

**Key modules**:
- `mcp/` — one file per MCP tool, parameter structs and handlers
- `cli.rs` — CLI one-shot executor (param parsing, dispatch, JSON output)
- `tcp.rs` — TCP connection management and request-response matching
- `activity.rs` — Activity logging to editor dock

## `director`

**Type**: Binary (`bin`)
**Purpose**: MCP server for scene/resource modification

The director crate implements the Director MCP tools. It communicates with the GDScript addon (not a GDExtension) via TCP.

**Dependency rules**:
- Depends on: `rmcp`, `tokio`, `tracing`, `anyhow`, `serde`
- No dependency on any stage crate

**Backend routing** (`backend.rs`):
1. Try TCP connect to port 6551 (editor plugin)
2. Try TCP connect to port 6550 (daemon)
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
- No dependency on any stage, director, rmcp, tokio, or gdext crate
- All operations are synchronous (`std::process::Command` for cargo builds)

## Workspace layout

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/stage-protocol",
    "crates/stage-core",
    "crates/stage-godot",
    "crates/stage-server",
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

<DepGraph
  :cols="3" :rows="3"
  :nodes="[
    { id: 'godot', label: 'stage-godot', row: 0, col: 0, highlight: 'amber' },
    { id: 'protocol', label: 'stage-protocol', row: 1, col: 1 },
    { id: 'server', label: 'stage-server', row: 0, col: 2, highlight: 'brand' },
    { id: 'core', label: 'stage-core', row: 2, col: 1 },
    { id: 'director', label: 'director', row: 2, col: 0, note: 'no stage deps', highlight: 'muted' },
    { id: 'cli', label: 'theatre-cli', row: 2, col: 2, note: 'clap + filesystem', highlight: 'muted' },
  ]"
  :edges="[
    { from: 'godot', to: 'protocol' },
    { from: 'server', to: 'protocol' },
    { from: 'server', to: 'core' },
  ]"
/>

The diamond dependency (both `stage-godot` and `stage-server` depend on `stage-protocol`) is intentional — they both need the same wire format types.
