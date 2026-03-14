---
name: stage-dev
description: Orientation for working on the Spectator codebase itself. Covers repo layout, crate responsibilities, design decisions, key patterns, and how the pieces connect.
---

# Theatre — Developer Orientation

Theatre is a Godot AI agent toolkit containing two tools:
- **Spectator**: Rust MCP server + GDExtension for runtime spatial observation
- **Director**: Rust MCP server + GDScript addon for editor-time scene manipulation

This skill covers Spectator development. See the Director crate and
`docs/director-spec.md` for Director specifics.

## Crate Map

```
theatre/
├── crates/
│   ├── stage-server/     # Stage MCP binary — Claude Code connects here
│   ├── stage-godot/      # Spectator GDExtension cdylib — Godot loads this
│   ├── stage-protocol/   # Shared: TCP wire format, message types
│   ├── stage-core/       # Shared: spatial math, bearing, indexing, budget
│   └── director/             # Director MCP binary
├── addons/stage/         # Spectator Godot addon (copy into user projects)
│   ├── plugin.gd             # @tool EditorPlugin (GDScript)
│   ├── runtime.gd            # Autoload singleton (GDScript)
│   ├── dock.tscn / dock.gd   # Editor dock UI
│   └── spectator.gdextension # Manifest pointing to GDExtension binaries
├── addons/director/          # Director Godot addon
├── docs/                     # All design docs
└── tests/
    ├── wire-tests/           # Spectator E2E tests
    └── director-tests/       # Director E2E tests
```

## The Two-Artifact Rule

There are two separate Rust compilation targets for Spectator:

| Target | Crate | Type | Process |
|---|---|---|---|
| `stage-server` | `crates/stage-server` | binary | Spawned by AI client (stdio) |
| `stage-godot` | `crates/stage-godot` | cdylib | Loaded by Godot (GDExtension) |

They **never link against each other**. They communicate over TCP using types from `stage-protocol`. `stage-core` is pure logic shared by both — no Godot API, no MCP API.

## The Thin Addon Principle

The GDExtension addon is **deliberately thin**. It answers "what does the engine say right now?" and nothing more. All spatial reasoning lives in the server.

**Addon does:**
- Traverse the scene tree and read node properties
- Answer TCP queries (get_visible_nodes, get_node_state, raycast, etc.)
- Capture frame data for recordings
- Execute actions (teleport, set_property, call_method)
- Listen on :9077

**Addon does NOT:**
- Compute bearings or distances
- Diff state between frames
- Cluster nodes
- Manage token budgets
- Store watches or session state
- Write to SQLite

Keep computation in the server where it's easy to test without Godot.

## TCP Architecture

```
AI Client ──stdio──► stage-server ──TCP :9077──► Godot Addon
```

- **Addon listens** (persistent — lives with the game)
- **Server connects** (ephemeral — spawned per AI session)
- Protocol: length-prefixed JSON (`[4-byte u32 big-endian][JSON]`)
- Types defined in `stage-protocol` crate
- Server reconnects every 2s on disconnect

## Data Flow for a Tool Call

```
1. AI agent calls spatial_snapshot via MCP (stdio)
2. stage-server MCP handler receives call
3. Handler acquires Arc<Mutex<SessionState>>
4. Handler sends TCP query to addon: { id, method: "get_visible_nodes", params }
5. Addon receives query in _physics_process poll()
6. Addon traverses scene tree, collects raw data
7. Addon sends TCP response: { id, data: [...] }
8. Server receives response
9. Server processes: builds spatial index, computes bearings, applies budget
10. Server returns MCP response (JSON string)
```

## Key Design Decisions

**Why addon-listens not server-listens?** Addon is persistent (runs with Godot all day). Server is ephemeral (spawned per AI session). Ephemeral connects to persistent. Server restarts don't require Godot restarts.

**Why GDScript EditorPlugin + GDExtension classes?** Godot bug #85268: GDScript can't inherit from GDExtension-derived EditorPlugin. So the EditorPlugin is pure GDScript; GDExtension classes are instantiated and used by that GDScript.

**Why SQLite for recordings?** Temporal queries (query_range, diff_frames, find_event) map cleanly to SQL. WAL mode handles 60fps writes without blocking reads. Single portable file per recording.

**Why MessagePack for frame data in SQLite?** Compact binary (2-5KB per frame vs ~15KB JSON). 30 seconds at 60fps = ~270-450MB JSON vs ~54-90MB MessagePack.

**Why `stage-core` separate?** Spatial indexing, bearing math, budget estimation, and delta computation are pure logic. Testable without Godot or MCP. Both server and addon-adjacent code can use it.

## Adding a New MCP Tool

1. Add params struct in `crates/stage-server/src/mcp/<tool>.rs`:
```rust
#[derive(Deserialize, JsonSchema)]
pub struct MyToolParams { ... }
```

2. Add async method to the `#[tool_box]` impl in `src/mcp/mod.rs` (or the tool's module):
```rust
#[tool(description = "...")]
async fn my_tool(&self, params: MyToolParams) -> Result<String, McpError> { ... }
```

3. Add any new TCP query methods to `stage-protocol`:
```rust
// In messages.rs, add variant to QueryMethod enum
QueryMethod::GetMyData { ... }
```

4. Handle new query in `crates/stage-godot/src/query_handler.rs`

5. Add acceptance criteria to `docs/USER_STORIES.md`, update `docs/CONTRACT.md`

## Adding a New Addon Query Method

1. Define the method in `stage-protocol/src/messages.rs` (both request params and response data types)
2. Implement in `crates/stage-godot/src/collector.rs` as a Rust function using gdext APIs
3. Expose as `#[func]` if GDScript needs to call it directly, or just call from the query handler
4. Register in the query dispatch in `query_handler.rs`
5. Call from the appropriate server-side tool handler

## Testing

```bash
# Unit tests (no Godot needed)
cargo test -p stage-core
cargo test -p stage-protocol

# Build server
cargo build --release -p stage-server

# Build GDExtension (requires Godot headers via gdext build script)
cargo build --release -p stage-godot

# Copy built library to addon
cp target/release/libspectator_godot.so addons/stage/bin/linux/
```

For integration testing, run Godot with a test project in `--headless` mode and point it at the built addon.

## Skill Cross-References

- Working on GDExtension Rust code → `/gdext`
- Working on MCP server Rust code → `/rmcp`
- Working on GDScript plugin/autoload/dock → `/godot-addon`
- Using Spectator to debug a Godot game (end-user) → `/spectator` (in `skills/stage/`)
