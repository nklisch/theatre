# Stage — Technical Choices & Strategy

## Stack Overview

| Component | Technology | Rationale |
|---|---|---|
| MCP Server | Rust (binary) | Performance, memory safety, single language with GDExtension |
| MCP SDK | `rmcp` 0.16+ | Official Rust MCP SDK, tokio-based, procedural macros |
| Async Runtime | `tokio` | Industry standard, required by rmcp, drives TCP client |
| GDExtension | Rust via `gdext` 0.4+ | Same language as server, real perf gains for bulk data collection |
| Serialization | `serde` + `serde_json` | Rust standard, used by both MCP and TCP protocol |
| Spatial Indexing | `rstar` (R-tree) | Proven spatial index for 3D nearest/radius queries |
| 2D Spatial Index | Flat grid hash | Simpler, faster for 2D scenes with uniform distribution |
| Recording Storage | `rusqlite` (SQLite + WAL) | Queryable, single-file portable, handles 60fps writes |
| JSON Schema | `schemars` | Required by rmcp for MCP tool parameter validation |
| Editor Plugin | GDScript (`@tool`) | Required — GDExtension classes can't be EditorPlugin bases |
| Runtime Autoload | GDScript | Bridges GDExtension classes with scene tree lifecycle |
| Editor Dock UI | Godot scenes (`.tscn`) | Native Godot UI, built in editor, used by GDScript plugin |
| Godot Minimum | 4.5 | api-4-5 feature flag requires Godot 4.5+ at runtime |
| License | MIT | Matches Godot's license, maximally permissive |

## Repository Layout

```
stage/
├── Cargo.toml                      # Workspace manifest
├── LICENSE                         # MIT
├── README.md
├── CLAUDE.md                       # Agent instructions for working in this repo
│
├── crates/
│   ├── stage-server/           # MCP server binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs             # Entry point: MCP stdio + TCP client
│   │       ├── mcp/                # MCP tool handlers
│   │       │   ├── mod.rs
│   │       │   ├── snapshot.rs     # spatial_snapshot
│   │       │   ├── delta.rs        # spatial_delta
│   │       │   ├── query.rs        # spatial_query
│   │       │   ├── inspect.rs      # spatial_inspect
│   │       │   ├── watch.rs        # spatial_watch
│   │       │   ├── config.rs       # spatial_config
│   │       │   ├── action.rs       # spatial_action
│   │       │   ├── scene_tree.rs   # scene_tree
│   │       │   └── recording.rs    # recording
│   │       ├── tcp/                # TCP client to Godot addon
│   │       │   ├── mod.rs
│   │       │   ├── client.rs       # Connection management, reconnection
│   │       │   └── codec.rs        # Length-prefixed JSON framing
│   │       ├── session.rs          # Per-session state (watches, config, cache)
│   │       └── budget.rs           # Token budget calculation & enforcement
│   │
│   ├── stage-godot/            # GDExtension cdylib
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # gdext entry point
│   │       ├── collector.rs        # StageCollector: scene tree observation
│   │       ├── tcp_server.rs       # StageTCPServer: listen + respond
│   │       ├── recorder.rs         # StageRecorder: frame capture
│   │       ├── query_handler.rs    # Dispatches incoming queries to collectors
│   │       └── action_handler.rs   # Executes spatial_action operations
│   │
│   ├── stage-protocol/         # Shared TCP wire format
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── messages.rs         # Request/Response enums, Query/Event types
│   │       ├── handshake.rs        # Handshake message types
│   │       └── codec.rs            # Length-prefixed JSON encoding/decoding
│   │
│   └── stage-core/             # Shared logic
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── spatial.rs          # Bearing calculation, distance, clustering
│           ├── index.rs            # R-tree wrapper, 2D grid hash
│           ├── delta.rs            # Diff computation between snapshots
│           ├── budget.rs           # Token estimation, truncation logic
│           └── types.rs            # Entity, Transform, RelativePosition, etc.
│
├── addons/
│   └── stage/                    # Godot addon (user copies this into their project)
│       ├── plugin.cfg              # Godot plugin metadata
│       ├── plugin.gd               # @tool EditorPlugin: dock, autoload registration
│       ├── runtime.gd              # Autoload: instantiates GDExtension, handles input
│       ├── dock.tscn               # Editor dock panel scene
│       ├── dock.gd                 # Dock panel script
│       ├── stage.gdextension   # GDExtension manifest (platform binary paths)
│       ├── icons/                  # Dock/button icons
│       └── bin/                    # Platform binaries (populated by CI)
│           ├── linux/
│           │   └── libstage_godot.so
│           ├── windows/
│           │   └── stage_godot.dll
│           └── macos/
│               └── libstage_godot.dylib
│
├── docs/
│   ├── VISION.md
│   ├── SPEC.md
│   ├── TECH.md                     # (this file)
│   ├── UX.md
│   ├── CONTRACT.md
│   └── USER_STORIES.md
│
├── skills/
│   └── stage.md                # Agent skill file (teaches agents how to use Stage)
│
├── examples/
│   ├── 3d-debug-demo/              # Godot 3D project with enemies, patrols, etc.
│   └── 2d-platformer-demo/         # Godot 2D project demonstrating 2D support
│
└── .github/
    └── workflows/
        ├── ci.yml                  # Lint, test, build all crates
        ├── release.yml             # Build platform bundles, GitHub release
        └── godot-test.yml          # Integration tests against running Godot
```

## Cargo Workspace

```toml
# theatre/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/stage-server",
    "crates/stage-godot",
    "crates/stage-protocol",
    "crates/stage-core",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/USER/theatre"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.32", features = ["bundled"] }
rstar = "0.12"
```

### Crate Dependency Graph

```
stage-server
├── stage-protocol
├── stage-core
├── rmcp (MCP SDK)
├── tokio
├── rusqlite
└── serde / serde_json

stage-godot
├── stage-protocol
├── godot (gdext)
└── serde / serde_json

stage-protocol
└── serde / serde_json

stage-core
├── rstar
└── serde / serde_json
```

Note: `stage-godot` depends on `stage-protocol` but NOT on `stage-core`. The core logic (spatial indexing, delta computation, budget management) lives exclusively in the server. The GDExtension is deliberately thin — it collects raw data and responds to queries. The server does the thinking.

## Two Rust Artifacts

### stage-server (Binary)

```toml
# crates/stage-server/Cargo.toml
[package]
name = "stage-server"

[[bin]]
name = "stage-server"
path = "src/main.rs"

[dependencies]
stage-protocol.workspace = true
stage-core.workspace = true
rmcp = { version = "0.16", features = ["server"] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
rusqlite.workspace = true
schemars = "1"
```

Built with `cargo build --release -p stage-server`. Produces a standalone binary. Distributed via GitHub releases and `cargo install stage-server`.

### stage-godot (cdylib)

```toml
# crates/stage-godot/Cargo.toml
[package]
name = "stage-godot"

[lib]
crate-type = ["cdylib"]

[dependencies]
stage-protocol.workspace = true
godot = { version = "0.4", features = ["register-docs"] }
serde.workspace = true
serde_json.workspace = true
```

Built with `cargo build --release -p stage-godot`. Produces a platform-specific shared library (`.so` / `.dll` / `.dylib`). Copied into `addons/stage/bin/` and distributed as part of the Godot addon.

## GDExtension Architecture

### The Hybrid Pattern

Godot has a known limitation: a GDScript inheriting from a GDExtension-derived EditorPlugin cannot function as an editor plugin (godot#85268). The solution is clean separation:

```
GDScript Layer (UI + lifecycle)          Rust GDExtension Layer (data + networking)
─────────────────────────────            ──────────────────────────────────────────
plugin.gd (EditorPlugin)         uses → StageCollector (Node)
  - adds dock panel                      - scene tree traversal
  - registers autoload                   - property collection
  - editor lifecycle                     - frame snapshots

runtime.gd (Autoload)            uses → StageTCPServer
  - instantiates Rust classes            - TCP listener on port 9077
  - handles F8/F9/F10 input              - protocol framing
  - bridges scene tree events            - query dispatch

dock.gd (Control)                uses → StageRecorder
  - recording UI                         - frame buffer
  - status display                       - SQLite write (via server)
  - marker controls                      - marker storage
```

GDScript calls Rust via `#[func]`-annotated methods. Rust accesses the scene tree via gdext's Node API. The GDScript layer is thin glue; the Rust layer does the work.

### GDExtension Classes Exposed to Godot

```rust
// Registered as Godot classes, usable from GDScript
StageCollector : Node
  - fn get_visible_nodes() -> Array
  - fn get_near(position, radius) -> Array
  - fn get_node_state(path) -> Dictionary
  - fn get_node_transform(path) -> Dictionary
  - fn get_scene_tree() -> Dictionary
  - fn raycast(from, to) -> Dictionary
  // ... all addon query methods from CONTRACT.md

StageTCPServer : Node
  - fn start(port: i32)
  - fn stop()
  - fn is_connected() -> bool
  - fn get_client_count() -> i32
  // Called by runtime.gd in _physics_process to pump messages
  - fn poll()

StageRecorder : Node
  - fn flush_dashcam_clip(label: String) -> String   // returns clip_id or ""
  - fn add_marker(source: String, label: String)     // TCP/human markers
  - fn add_code_marker(label: String, tier: String)  // game script markers (tier: "system"|"deliberate"|"silent")
  - fn get_frame(index: i32) -> Dictionary
  // signals: dashcam_clip_saved(clip_id, tier, frames), dashcam_clip_started(frame, tier), marker_added(frame, source, label)
```

### Threading Model

Godot's scene tree is **not thread-safe**. All scene tree access must happen on the main thread. The GDExtension follows this constraint:

1. `runtime.gd` calls `StageTCPServer.poll()` every `_physics_process`
2. `poll()` checks for incoming TCP messages (non-blocking)
3. For each query, `poll()` calls into `StageCollector` to gather data (main thread, scene tree safe)
4. Response is serialized and queued for TCP send
5. TCP send happens in the same `poll()` call (non-blocking write)

This is single-threaded and frame-locked. For 60fps physics, each frame has ~16ms. Property collection for ~100 nodes takes <1ms in Rust. TCP I/O is non-blocking. No threading complexity needed.

For dashcam capture at 60fps:
1. `StageRecorder` captures frames every `_physics_process` into a ring buffer
2. On trigger (marker, F9, agent `save`, or `StageRuntime.marker()` from game code), the ring buffer window is written to SQLite
3. Clip is identified by a `clip_id` (e.g. `clip_001a2b3c`)

## MCP Server Architecture

The server runs as a tokio async process with two concurrent tasks:

```
┌─────────────────────────────────────────────────┐
│  stage-server process                       │
│                                                 │
│  ┌─────────────────────────────────────────┐    │
│  │  Task 1: MCP Handler (rmcp)             │    │
│  │  - Reads MCP JSON-RPC from stdin        │    │
│  │  - Dispatches to tool handlers          │    │
│  │  - Writes MCP JSON-RPC to stdout        │    │
│  └─────────────┬───────────────────────────┘    │
│                │ Arc<SharedState>                │
│  ┌─────────────▼───────────────────────────┐    │
│  │  Task 2: TCP Client (tokio)             │    │
│  │  - Connects to Godot addon on :9077     │    │
│  │  - Sends queries, receives responses    │    │
│  │  - Receives push events (signal/watch)  │    │
│  │  - Handles reconnection                 │    │
│  └─────────────────────────────────────────┘    │
│                                                 │
│  Shared State (Arc<Mutex<SessionState>>):       │
│  - Last snapshot frame                          │
│  - Spatial index (rstar R-tree)                 │
│  - Static node cache                            │
│  - Watch list + conditions                      │
│  - Config (clustering, properties, format)      │
│  - Node classification (static/dynamic)         │
│  - Recording index (SQLite handle)              │
└─────────────────────────────────────────────────┘
```

### Tool Call Flow

```
AI Client → MCP (stdio) → stage-server
  → tool handler builds query
  → TCP query → Godot addon
  → addon collects data from scene tree
  → TCP response → stage-server
  → server processes (spatial index, bearing calc, delta, budget)
  → MCP response (stdio) → AI Client
```

Typical round-trip: <50ms for most queries (TCP localhost + scene tree collection + processing).

## Distribution Strategy

### Primary: GitHub Releases (Platform Bundles)

Each release produces per-platform archives:

```
stage-v0.1.0-linux-x86_64.tar.gz
stage-v0.1.0-macos-arm64.tar.gz
stage-v0.1.0-macos-x86_64.tar.gz
stage-v0.1.0-windows-x86_64.zip
```

Each archive contains:
```
stage-v0.1.0-linux-x86_64/
├── bin/
│   └── stage-server           # MCP server binary
├── addons/
│   └── stage/                    # Complete Godot addon with matching GDExtension
│       ├── plugin.cfg
│       ├── plugin.gd
│       ├── runtime.gd
│       ├── dock.tscn
│       ├── dock.gd
│       ├── stage.gdextension
│       └── bin/
│           └── linux/
│               └── libstage_godot.so
└── README.md                       # Quick start instructions
```

**Install steps:**
1. Download the archive for your platform
2. Copy `addons/stage/` into your Godot project
3. Enable the plugin in Project Settings → Plugins
4. Add the MCP server to your AI client config (path to `stage-server` binary)

### Secondary: Cargo Install (Server Only)

```bash
cargo install stage-server
```

For Rust developers who want the server binary. The Godot addon (including GDExtension binaries) still comes from GitHub releases.

### Tertiary: Godot Asset Library (Addon Only)

The `addons/stage/` directory published to the Godot Asset Library. Includes pre-built GDExtension binaries for all platforms. Users install the server binary separately.

### Future: One-Click Install Script

An install script (or the addon itself on first run) that:
1. Detects the platform
2. Downloads the matching `stage-server` binary from GitHub releases
3. Places it in a known location
4. Outputs the MCP config snippet to paste into the AI client

## Build & CI

### CI Pipeline (GitHub Actions)

```yaml
# Triggered on push to main and PRs
ci:
  - cargo fmt --check
  - cargo clippy --workspace
  - cargo test --workspace
  - cargo build --release -p stage-server
  - cargo build --release -p stage-godot

# Triggered on tag push (v*)
release:
  matrix:
    - target: x86_64-unknown-linux-gnu
    - target: aarch64-apple-darwin
    - target: x86_64-apple-darwin
    - target: x86_64-pc-windows-msvc
  steps:
    - Build stage-server for target
    - Build stage-godot for target
    - Package addon + server into archive
    - Upload to GitHub release
```

### Cross-Compilation

Both `stage-server` and `stage-godot` must be cross-compiled for each target. The GDExtension cdylib requires the Godot API headers for the target platform — gdext handles this via its build script.

Required Rust targets:
- `x86_64-unknown-linux-gnu` (Linux)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-apple-darwin` (macOS Intel)
- `x86_64-pc-windows-msvc` (Windows)

## Testing Strategy

### Unit Tests

- `stage-core`: Bearing calculation, spatial indexing, delta computation, budget estimation. Pure logic, no external dependencies. High coverage target.
- `stage-protocol`: Serialization round-trips, codec correctness.

### Integration Tests

- `stage-server`: Mock TCP connection simulating addon responses. Verify MCP tool handlers produce correct output for given inputs.
- `stage-godot`: Requires a running Godot instance. Test scene tree collection, property reading, TCP server behavior. Run via Godot's `--headless` mode in CI.

### End-to-End Tests

- Launch Godot with a test project + addon enabled
- Launch `stage-server` connected to the addon
- Send MCP tool calls, verify responses against known scene state
- These are slow and run only on main branch merges, not every PR

## Versioning Strategy

Stage uses **workspace versioning** — all crates share the same version number. This simplifies compatibility: `stage-server` v0.3.0 works with `stage-godot` v0.3.0. Mismatched versions are detected during the TCP handshake.

The `.gdextension` manifest sets `compatibility_minimum = "4.5"` to match the `api-4-5` feature flag. The `lazy-function-tables` feature provides forward compatibility with 4.6+ releases.

### Version Compatibility Matrix

| stage-server | stage-godot | TCP Protocol | Godot |
|---|---|---|---|
| 0.x | 0.x (matching) | v1 | 4.5+ |
| 1.x | 1.x (matching) | v1 or v2 | 4.5+ |

Protocol version is exchanged in the TCP handshake. The server and addon negotiate the highest mutually supported version.

## Agent Skill File

Distributed as `skills/stage.md`, this file teaches AI agents *when and how* to use Stage's tools effectively. It includes:

- When to use each tool (decision tree)
- Token-efficient querying patterns (start summary, drill down)
- Common debugging workflows (collision, pathfinding, state machine)
- Tool parameter cheat sheet
- Example conversations

Users add this to their agent's skill/context configuration. For Claude Code, this goes in `.claude/skills/` or is referenced in `CLAUDE.md`.

## Long-Term Considerations

### Performance Telemetry (Future)

Stage is spatial-first. Performance data (FPS, draw calls, physics tick time) is out of scope for v1. However, the architecture supports adding a `spatial_perf` tool later — the GDExtension can collect `Engine.get_frames_per_second()` and `Performance` singleton data alongside spatial state. The TCP protocol and MCP tool surface can be extended without breaking changes.

### Exported Build Support (Future)

The GDExtension autoload can technically run in exported (release) builds, enabling spatial debugging of packaged games. This requires:
- A build flag to include/exclude the addon from exports
- Security considerations (the TCP server in a release build)
- Conditional compilation in the GDExtension

Desirable for debugging platform-specific issues (e.g., "this only clips on the Steam Deck"). Not in v1 scope but the architecture should not prevent it.

### Extensibility (Data Layer)

The 9 MCP tools are the stable API surface. Game developers extend Stage at the **data layer**, not the tool layer:

- Register custom properties to track per node class or group
- Define custom state extractors (GDScript callables returning data)
- Configure custom groups and categories
- All custom data flows through existing tools (appears in `state`, `spatial_inspect`, etc.)

This keeps the agent interface stable while allowing game-specific data richness.

### Plugin Marketplace Presence

Once stable, Stage should be listed on:
- Godot Asset Library (addon)
- crates.io (stage-server)
- MCP tool registries (when they exist)
- Agent skill registries / skilltap
