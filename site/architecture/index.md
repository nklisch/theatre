# Architecture Overview

Theatre's architecture is built around three principles: thin addon, smart server, and zero game impact.

## Design principles

### Thin addon, smart server

The GDExtension addon (`spectator-godot`) does as little as possible:
- Walk the scene tree on each physics tick
- Collect raw node data (positions, velocities, properties)
- Serialize to the wire format
- Send over TCP when requested

All spatial reasoning — budgeting, diffing, indexing, budget trimming, query geometry — happens in the Rust server (`spectator-server`). This separation means:

1. The addon stays stable across Godot versions (less surface area)
2. Bugs in spatial logic are fixed in the server without redeploying the GDExtension
3. The addon's performance impact on the game is minimal and predictable

### Zero game impact

The collector runs in `_physics_process` with O(n) complexity over tracked nodes. With 100 tracked nodes, collection takes < 0.1ms per frame — invisible to the player. Data is written to a ring buffer (not streamed); TCP transmission happens only when the MCP server requests data.

The addon never modifies game state (except for `spatial_action`, which is explicitly mutation). It is safe to leave the addon enabled in development builds.

### Ports and adapters

Both tools use the ports-and-adapters pattern internally:

```
MCP layer (stdio JSON-RPC)
    ↕
Domain layer (spatial queries, clip analysis, budgeting)
    ↕
Protocol layer (TCP codec, message types)
    ↕
Godot layer (GDExtension / GDScript addon)
```

Each layer can change without affecting the others. The MCP schema can evolve without changing the TCP protocol. The TCP codec is shared between server and addon via `spectator-protocol`.

## Component map

```
theatre/
├── crates/
│   ├── spectator-server/     MCP binary (stdio)
│   │   ├── tools/            9 MCP tool handlers
│   │   ├── session.rs        TCP connection management
│   │   ├── activity.rs       Activity logging
│   │   └── main.rs           rmcp server setup
│   │
│   ├── spectator-godot/      GDExtension cdylib
│   │   ├── tcp_server.rs     TCP listener + codec
│   │   ├── collector.rs      Scene tree walker
│   │   └── recorder.rs       Clip file writer
│   │
│   ├── spectator-protocol/   Shared TCP types
│   │   ├── codec.rs          Length-prefix framing
│   │   └── messages.rs       Request/response types
│   │
│   ├── spectator-core/       Pure spatial logic
│   │   ├── spatial.rs        Query geometry
│   │   ├── budget.rs         Token budget trimming
│   │   └── diff.rs           Frame diffing
│   │
│   └── director/             Director MCP binary
│       ├── tools/            Operation handlers
│       ├── backend.rs        Backend routing
│       └── main.rs           rmcp server setup
│
├── addons/
│   ├── spectator/            GDScript addon
│   │   ├── plugin.gd         EditorPlugin
│   │   ├── runtime.gd        GDExtension wrapper
│   │   └── dock.gd           Editor dock UI
│   │
│   └── director/             Director GDScript addon
│       ├── plugin.gd         EditorPlugin + TCP listener
│       └── daemon.gd         Headless daemon script
│
└── tests/
    ├── wire-tests/           Spectator E2E tests
    └── director-tests/       Director E2E tests
```

## Data flow: snapshot request

```
1. Agent calls spatial_snapshot tool

2. spectator-server receives MCP tool call via stdin

3. server serializes SnapshotRequest { detail, budget_tokens, ... }
   → 4-byte length prefix + JSON
   → writes to TCP socket

4. spectator-godot reads from TCP socket
   → deserializes SnapshotRequest
   → queries collector's ring buffer for most recent frame
   → serializes SnapshotResponse { frame, nodes: [...] }
   → writes back over TCP

5. spectator-server reads response
   → passes raw node list to spectator-core budget trimmer
   → trims to budget_tokens (prioritizing focus_node / include_types)
   → serializes final MCP response JSON
   → writes to stdout

6. Agent receives tool result
```

## Thread model

### spectator-godot (GDExtension)

All GDExtension code runs on Godot's **main thread**. `_physics_process` is called by the engine, and the collector accesses `Gd<Node>` only within that callback. There are no background threads in the GDExtension.

The TCP server listens on a separate thread (Rust `std::thread::spawn`), but the thread only reads/writes the TCP socket and a shared `Arc<Mutex<FrameBuffer>>`. It never accesses Godot engine APIs directly.

### spectator-server (MCP binary)

The server is a `tokio` async binary. The main TCP connection to the addon runs as a background task. MCP tool call handlers are async and await responses via `oneshot` channels stored in shared state (`Arc<Mutex<SessionState>>`).

No tool handler holds the session lock while awaiting the TCP response — locks are acquired to place the request, released, then re-acquired to read the response. This prevents deadlocks.

## Security model

Theatre is a **local development tool only**:

- All TCP ports bind to `127.0.0.1`
- No authentication (any local process can connect)
- GDExtension should not be included in production builds
- Director's `spatial_action` can execute arbitrary GDScript methods

Do not use Theatre in production games or on servers with remote access.
