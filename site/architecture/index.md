---
description: "Theatre's engine, server, authoring, feedback, transport, and security boundaries."
---

# Architecture Overview

Theatre places Godot engine access inside Godot and keeps typed orchestration,
analysis, and agent response shaping in Rust processes outside it.

## Component boundaries

- **Stage runtime:** `addons/stage` and `stage-godot` observe and act on the
  running scene tree. Godot objects stay on the engine main thread.
- **Stage server:** `stage-server` owns MCP and CLI handling, session state,
  response shaping, spatial reasoning, and retained clip analysis.
- **Shared Stage protocol:** `stage-protocol` owns the typed length-prefixed TCP
  boundary between the server and GDExtension.
- **Director:** the Rust server selects a verified editor, supervised headless
  daemon, or one-shot Godot backend. GDScript operation modules use native Godot
  serialization.
- **Human feedback:** `addons/theatre_shared` publishes project-local evidence.
  `theatre-feedback` gives both servers and the CLI one typed reader.
- **Distribution:** `theatre-cli` installs, initializes, enables, deploys, and
  configures the matched binaries, addon payloads, and optional self-contained
  client packages with Stage and Director operating skills.

See [Crate Structure](/architecture/crates) for source ownership and the
[generated API reference](/api/) for current tool schemas.

## Stage data flow

```text
agent -> stage MCP handler -> TCP query -> Godot main-thread engine access
agent <- shaped MCP result <- server/core reasoning <- typed engine response
```

The live addon listens and the Stage server connects. The addon handshake reports
the actual project and engine run. `runtime_status` queries current scene and
readiness rather than treating transport connection as readiness.

The recorder has a separate lifecycle. It owns bounded dashcam buffers, markers,
and clip persistence, so capture can continue without an agent session. The
on-demand `viewport` path reads and encodes the latest completed root viewport
without using that recorder.

## Director data flow

```text
agent -> director MCP handler -> verified editor | daemon | one-shot -> Godot API
```

Director tries its backends in that order for supported operations. Open-scene
changes use the actual editor root and native undo. They remain unsaved until
`scene_save`. Detached headless changes persist their target files. A batch runs
sequentially and reports earlier or partial effects without rollback.

`editor_run` is editor-only and controls a selected saved scene without implicitly
saving open work. Director reports the native launch state; Stage reports runtime
readiness and run identity.

## Thread and process rules

Stage scene-tree and engine calls stay on Godot's main thread. Plain owned pixel
or diagnostic data can cross a worker boundary where the implementation supports
it. Stage's runtime listener is polled through the engine lifecycle rather than
letting a socket thread touch Godot objects.

The Stage MCP server uses Tokio for asynchronous agent and TCP coordination.
Director supervises its headless daemon and keeps uncertain post-dispatch outcomes
from being replayed on another backend.

## Performance boundary

Capture and engine queries have real cost. Current viewport readback is bounded
and on-demand. The recorder uses a separate capture path for continuous evidence.
Use detail tiers, filters, pagination, and token budgets to constrain response
size. Do not infer a universal frame cost from one scene or machine.

## Security boundary

Theatre is a local development toolkit with powerful mutation operations and no
protocol authentication. Stage binds its listener to loopback. Director clients
use local addresses, but its GDScript listeners do not set an explicit bind
address.

Keep Theatre off untrusted networks. Do not expose it as a production-game
service without an appropriate security boundary.
