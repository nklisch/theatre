# How It Works

This page explains Theatre's architecture: how data flows from the running Godot engine to your AI agent's tool call response.

## The big picture

```
┌─────────────────────────────────┐
│         AI Agent (Claude)       │
│  "Where is the player?"         │
└──────────────┬──────────────────┘
               │ MCP (stdio)
               ▼
┌─────────────────────────────────┐
│      stage server (Rust)        │
│  Translates MCP ↔ TCP protocol  │
└──────────────┬──────────────────┘
               │ TCP port 9077
               │ (length-prefixed JSON)
               ▼
┌─────────────────────────────────┐
│  stage GDExtension (Rust)       │
│   Runs inside your Godot game   │
│   Reads scene tree every tick   │
└──────────────┬──────────────────┘
               │ Godot engine APIs
               ▼
┌─────────────────────────────────┐
│         Running Godot game      │
│   CharacterBody3D, Area3D, ...  │
└─────────────────────────────────┘
```

The flow is always initiated by the AI agent (via the MCP server). The GDExtension does not push data unprompted — it collects data on every physics tick and serves it when the server requests.

## Stage: GDExtension architecture

### The addon

The Stage GDExtension (`libstage_godot.so`) is a compiled Rust library loaded by Godot at startup. It registers several GDExtension classes:

- **`StageTCPServer`** — manages the TCP listener on port 9077, handles framing
- **`StageCollector`** — walks the scene tree on each `_physics_process` tick, collecting positions, velocities, and properties of tracked nodes into an in-memory frame buffer
- **`StageRecorder`** — manages the dashcam ring buffer and writes clip files to disk

These classes are instantiated by `addons/stage/plugin.gd` (the GDScript `EditorPlugin`), which also manages the editor dock.

The collector runs at Godot's physics tick rate (default 60 Hz). It captures data in a ring buffer — old frames are dropped to keep memory bounded. The buffer depth determines how far back a `clips` query can look without an explicit clip file.

### The GDScript layer

`addons/stage/runtime.gd` is a thin GDScript file that:
- Checks if the GDExtension loaded via `ClassDB.class_exists`
- Instantiates extension classes using direct constructors (e.g. `StageTCPServer.new()`)
- Provides graceful degradation if the extension is missing (logs a warning, no crash)

This design means the addon can be enabled in a project even if the GDExtension binary is missing — it just won't collect any data. This prevents parse errors when the `.so` is not yet deployed.

### The server

The Stage server (`stage` binary, crate: `stage-server`) is a Rust binary that supports two modes:
- **`stage serve`** — MCP server on stdio (persistent TCP connection, auto-reconnect)
- **`stage <tool> '<json>'`** — CLI one-shot mode (connect once, execute, exit)

When a tool is called (via MCP or CLI), the server:

1. Receives the tool call (stdin JSON-RPC in serve mode, or CLI arg/stdin in CLI mode)
2. Serializes the request to length-prefixed JSON
3. Sends it over the TCP socket to the GDExtension
4. Waits for the response (with timeout)
5. Deserializes the response
6. Applies token budget trimming
7. Serializes the result as JSON
8. Writes it to stdout

In serve mode, the server maintains a persistent TCP connection. If the game restarts, it automatically reconnects. In CLI mode, it connects once and exits after the tool completes.

## TCP Protocol

All messages between the server and the GDExtension use the same framing:

```
[4 bytes: big-endian u32 length][JSON payload of `length` bytes]
```

Example: sending `{"type":"snapshot","detail":"summary"}` (38 bytes):

```
00 00 00 26  7b 22 74 79 70 65 22 3a 22 73 6e 61 70 ...
```

The 4-byte length prefix allows both sides to read exactly one message per `recv()` call, regardless of how TCP splits the data.

Messages are typed JSON objects. Every request has a `"type"` field identifying the operation. Every response has a `"result"` field (or `"error"` on failure).

See [Wire Format](/api/wire-format) for the full protocol specification.

## Director: GDScript architecture

Director's architecture differs from Stage's because it needs to **modify** scene files, which requires Godot's resource system.

### Three backends

Director auto-selects which backend to use for each operation:

**Editor plugin backend** (port 6551): When the Director addon is running in the Godot editor, it listens on 6551 and can process operations using the full editor API — including `EditorScenePostImport`, resource saving, and script reloading.

**Headless daemon backend** (port 6550): A separate Godot instance runs headless (`godot --headless`), loads your project, and processes operations. Used when the editor is not running.

**One-shot fallback**: If neither TCP backend is reachable, Director spawns a temporary Godot headless process, executes the operation, and exits. Slower (one Godot startup per operation) but always available.

The Rust `director` binary handles the routing logic — it tries port 6551, then port 6550, then falls back to one-shot. You never need to manage this manually.

### Operations flow

```
AI Agent → director binary (MCP) → [editor | daemon | one-shot] → scene file on disk
```

The GDScript addon receives operations as TCP messages, executes them using Godot's scene API (`PackedScene`, `Resource`, `TileMap`, `AnimationPlayer`, etc.), and returns success/error responses.

## MCP Transport

Both Stage (`stage serve`) and Director (`director serve`) use the **stdio transport** for MCP. This means:

- The agent launcher starts the binary as a child process
- The binary reads JSON-RPC requests from stdin
- The binary writes JSON-RPC responses to stdout
- Logs go to stderr (never stdout — stdout is sacred for MCP)

This is the most compatible MCP transport — it works with every MCP-capable agent without any network configuration.

## Token budget system

Spatial snapshots can be large. A 200-node scene fully described would easily exceed 50,000 tokens. Theatre addresses this in two ways:

**`detail` levels**: `"summary"` returns only class and global_position per node. `"standard"` returns position, velocity, rotation, and common flags. `"full"` returns all tracked properties.

**`token_budget`**: The server measures the response as it builds it and stops adding nodes once the budget is reached. It always includes the most spatially relevant nodes first (nodes closest to a `focal_node`, or nodes matching `class_filter`).

The agent can always request more detail by narrowing scope — use `spatial_inspect` for one node, `spatial_query` for a region, or `scene_tree` for structure without spatial data.

## Data freshness

Stage data is always one physics tick old. When you call `spatial_snapshot`, the server requests the most recent collected frame from the GDExtension. The GDExtension collects data at the end of each `_physics_process` call, so the data is current to within ~16ms (at 60 Hz).

For monitoring changes over time, use `spatial_delta` (returns only what changed since a given frame) or `spatial_watch` (set up a watch that the server polls automatically).

## Security model

Theatre is a local development tool. Both TCP ports (9077, 6550, 6551) bind to `127.0.0.1` only — they do not accept connections from other machines on the network. The MCP servers run as local processes, not as network services.

There is no authentication — any process on localhost can connect. This is intentional for a developer tool. Do not run Theatre in production game builds.
