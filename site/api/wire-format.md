# Wire Format

The TCP protocol used between Theatre's MCP servers and the Godot addons.

## Overview

Both Stage and Director communicate with their Godot-side components over TCP using **length-prefixed JSON**:

```
[4 bytes: big-endian u32 length][JSON payload of exactly `length` bytes]
```

This framing ensures that both sides can read exactly one complete message per call to `recv()`, regardless of how TCP segments the data stream.

## Ports

| Component | Port | Direction |
|---|---|---|
| Stage | `9077` | Addon listens; server connects |
| Director (editor plugin) | `6551` | Plugin listens; director binary connects |
| Director (headless daemon) | `6550` | Daemon listens; director binary connects |

All ports bind to `127.0.0.1` only. No remote access.

The **addon listens, server connects** pattern for Stage means the MCP server can be started and stopped without affecting the running game — the game always has a socket open. The server connects when it needs data and can reconnect automatically after a game restart.

## Message framing

### Encoding

To send a message:

1. Serialize the payload to UTF-8 JSON (no trailing newline)
2. Compute the byte length: `len = payload.len()` (UTF-8 byte count, not character count)
3. Write the length as a 4-byte big-endian unsigned integer
4. Write the payload bytes

Example — sending `{"type":"ping"}` (14 bytes):

```
Hex: 00 00 00 0e 7b 22 74 79 70 65 22 3a 22 70 69 6e 67 22 7d
      ^---------^  ^----------------------------------------------^
      4-byte len   14 bytes of JSON
```

### Decoding

To receive a message:

1. Read exactly 4 bytes → `u32` big-endian → `length`
2. Read exactly `length` bytes → JSON payload
3. Parse JSON

If either read returns fewer bytes than requested (socket closed), the connection has terminated.

## Stage protocol

### Request types

All requests from the server to the addon are JSON objects with a `"type"` field:

```json
{"type": "snapshot", "detail": "summary", "token_budget": 2000}
{"type": "delta", "since_frame": 400, "token_budget": 1000}
{"type": "query", "query_type": "radius", "from": [0,0,0], "radius": 5.0}
{"type": "inspect", "node": "Player", "include": ["properties"]}
{"type": "config", "tick_rate": 30}
{"type": "action", "node": "Player", "action": "set_property", "property": "health", "value": 100}
{"type": "scene_tree", "max_depth": 3}
{"type": "watch_create", "node": "Player", "track": ["position", "velocity"]}
{"type": "watch_delete", "watch_id": "w_a1b2c3"}
{"type": "watch_list"}
{"type": "recording_marker", "source": "agent", "label": "bug_moment"}
{"type": "recording_markers", "clip_id": "clip_01"}
{"type": "recording_list"}
{"type": "recording_delete", "clip_id": "clip_01"}
{"type": "recording_resolve_path"}
{"type": "dashcam_status"}
{"type": "dashcam_flush"}
{"type": "dashcam_config"}
```

Clip analysis actions (`snapshot_at`, `trajectory`, `query_range`, `diff_frames`, `find_event`) are handled server-side by reading clip SQLite files directly — they do not use the TCP wire protocol.

### Response types

Responses always have a `"result"` field (`"ok"` on success) or `"error"` field on failure:

```json
{"result": "ok", "frame": 412, "nodes": {...}}
{"result": "error", "error": "Node 'NonExistent' not found"}
```

### Handshake

On connection, the addon sends a handshake message:

```json
{"type": "handshake", "version": "0.2.1", "godot_version": "4.3.stable", "project": "my-game"}
```

The server responds:

```json
{"type": "handshake_ack", "version": "0.2.1"}
```

If versions are incompatible, the server sends:

```json
{"type": "handshake_reject", "reason": "Version mismatch: server 0.2.1, addon 0.0.9"}
```

And closes the connection.

## Director protocol

Director uses the same framing (4-byte length prefix + JSON) but a different request schema:

```json
{"op": "scene_create", "path": "scenes/player.tscn", "root_class": "CharacterBody3D"}
```

Responses:

```json
{"op": "scene_create", "result": "ok", "path": "scenes/player.tscn"}
{"op": "scene_create", "result": "error", "error": "Directory 'scenes/' does not exist"}
```

## Connection lifecycle

### Stage

```
[Game starts] → addon starts TCP listener on 0.0.0.0:9077 (only accepts 127.0.0.1)
[Agent call]  → server connects to 127.0.0.1:9077
              → server receives handshake message
              → server sends handshake_ack
              → connection established; requests flow
[Game exits]  → addon closes listener; server detects disconnect
[Next call]   → server reconnects automatically
```

The server keeps the connection open across multiple tool calls (persistent connection). If the game restarts, the old connection dies and the server reconnects on the next tool call.

### Director

```
[Editor opens] → plugin starts TCP listener on 127.0.0.1:6551
[Tool call]    → director binary connects to 127.0.0.1:6551
               → sends operation JSON
               → receives response JSON
               → closes connection (not persistent)
[Editor closes] → plugin stops listener
[Next call]    → director binary tries 6551 (fail), tries 6550 (daemon), or uses one-shot
```

Director uses a per-request connection model — each operation is a new TCP connection. This keeps the protocol simple and avoids state management on the director binary side.

## Error handling

### Connection errors

If the TCP connection fails or is reset:

- **Stage server**: returns an MCP error to the agent with the message "Game not running or not reachable. Start the game and try again."
- **Director binary**: tries the next backend (6550, then one-shot).

### Message errors

If the JSON payload cannot be parsed, the receiving side sends an error response and closes the connection.

If a request refers to a non-existent node or resource, the response includes `"result": "error"` with a descriptive `"error"` message. The connection stays open.

## Implementation notes

The codec is implemented in `crates/stage-protocol/src/codec.rs` (shared between server and GDExtension):

```rust
// Synchronous write
pub fn write_message(writer: &mut impl Write, payload: &[u8]) -> Result<(), CodecError>

// Synchronous read
pub fn read_message(reader: &mut impl Read) -> Result<Vec<u8>, CodecError>

// Async write (tokio)
#[cfg(feature = "async")]
pub async fn write_message_async(stream: &mut TcpStream, payload: &[u8]) -> Result<(), CodecError>

// Async read (tokio)
#[cfg(feature = "async")]
pub async fn read_message_async(stream: &mut TcpStream) -> Result<Vec<u8>, CodecError>
```

Maximum message size: 16 MB (enforced by the decoder to prevent memory exhaustion from malformed length fields).
