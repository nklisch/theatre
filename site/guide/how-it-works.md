---
description: "How Theatre connects AI agents to Godot games — GDExtension addon, TCP protocol, and MCP tool architecture."
---

# How It Works

This page explains Theatre's architecture: how data flows from the running Godot engine to your AI agent's tool call response.

## The big picture

<FlowDiagram :steps="[
  { label: 'AI Agent (Claude)', subtitle: '&quot;Where is the player?&quot;' },
  { label: 'stage server (Rust)', subtitle: 'Translates MCP ↔ TCP protocol' },
  { label: 'stage GDExtension (Rust)', subtitle: 'Runs inside your Godot game' },
  { label: 'Running Godot game', subtitle: 'CharacterBody3D, Area3D, ...' },
]" :connectors="[
  { label: 'MCP (stdio)' },
  { label: 'TCP port 9077' },
  { label: 'Godot engine APIs' },
]" />

Live tool queries start from the agent through the MCP server. The runtime polls
its local listener on physics frames and answers explicit requests. The recorder
separately retains capture data without an agent connection, and a developer can
publish deliberate feedback for later retrieval.

## Stage: GDExtension architecture

### The addon

The Stage GDExtension (`libstage_godot.so`) is a compiled Rust library loaded by Godot at startup. It registers several GDExtension classes:

- **`StageTCPServer`** — manages the TCP listener on port 9077, handles framing
- **`StageCollector`** — walks the scene tree on each `_physics_process` tick, collecting positions, velocities, and properties of tracked nodes into an in-memory frame buffer
- **`StageRecorder`** — manages the dashcam ring buffer and writes clip files to disk

These classes are instantiated by the `StageRuntime` autoload in
`addons/stage/runtime.gd`. The GDScript EditorPlugin manages the autoload, editor
dock, debugger bridge, and project settings.

The collector reads current scene state at the engine boundary. The recorder owns
bounded dashcam buffers, markers, and clip persistence. Keeping those roles
separate lets current observation work without recording and lets recording
continue without an agent connection.

### The GDScript layer

`addons/stage/runtime.gd`:
- Checks extension classes through `ClassDB.class_exists`.
- Instantiates them dynamically so a missing platform binary does not cause a parse failure.
- Registers the bounded current-process Logger.
- Owns runtime controls, overlays, markers, and the feedback composer entrypoint.

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

In serve mode, the server maintains a persistent TCP connection. If the game restarts, it automatically reconnects. In CLI mode, it connects once and exits after a supported tool completes. Deltas, watches, session configuration updates, and actions requesting a delta require persistent MCP and are rejected by the CLI before connection.

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

**One-shot fallback**: If neither TCP backend is reachable, Director can spawn a
temporary headless Godot process for supported operations when the selected Godot
executable and project are available. Editor-only run control does not fall back.

The Rust `director` binary handles the routing logic — it tries port 6551, then port 6550, then falls back to one-shot. You never need to manage this manually.

### Operations flow

```
AI Agent → director binary (MCP) → [editor | daemon | one-shot] → scene file on disk
```

The GDScript addon receives operations as TCP messages, executes them using Godot's scene API (`PackedScene`, `Resource`, `TileMap`, `AnimationPlayer`, etc.), and returns success/error responses. Open-scene edits use the live root and native undo, and remain unsaved until `scene_save`. Batch entries run sequentially and preserve partial effects without rollback.

Director also verifies editor project identity, queries ClassDB through
`engine_api`, and controls selected saved-scene runs through `editor_run`. Stage
remains the authority for runtime readiness and run identity.

## Project-local human feedback

Stage runtime and Director editor controls can capture a viewport, pointer or
selection context, and an optional note. The shared `addons/theatre_shared`
payload publishes complete immutable item directories under
`.theatre/feedback`. Stage, Director, and the Theatre CLI use the same typed Rust
reader.

Retrieval does not consume evidence. Handling suppresses notices for all readers
but keeps the item, while deletion remains explicit. Optional Claude and Codex
hooks invoke a thin CLI helper after client tool calls. They inject a text notice
only and do not wake idle agents, steer a session asynchronously, or deliver the
JPEG as text.

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

A spatial response identifies the collected physics frame that produced its
state. It does not freeze the world while the server shapes the response. The
on-demand `viewport` result identifies readback counters and engine run identity,
but its pixels are not atomic with a separate spatial query.

For changes over time, establish a baseline with `spatial_snapshot`, then use
`spatial_delta` or a watch in the same persistent MCP session. One-shot CLI calls
do not share that state.

## Security model

Theatre is a local development tool. Stage binds its runtime listener to
loopback. Director clients use local addresses, but the GDScript editor and daemon
listeners do not set an explicit bind address. None of these protocols authenticate
callers.

Keep Theatre listeners off untrusted networks. Do not expose them as a
production-game service without adding an appropriate security boundary.
