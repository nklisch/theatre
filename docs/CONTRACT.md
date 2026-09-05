# Theatre Contract

This document defines the semantic rules shared by Theatre's controlled JSON boundaries: MCP parameters and results, Stage's TCP messages, Director's operation messages, and persisted clip metadata that is exposed back through the tools. It explains how a field should behave and be named; the Rust types and generated schemas define the exact shape.

Use the generated references for the current parameter and output catalog:

- [Stage API schemas](../site/api/index.md)
- [Director API schemas](../site/api/director.md)
- [Stage MCP handlers](../crates/stage-server/src/mcp/)
- [Director MCP handlers](../crates/director/src/mcp/)
- [Shared Stage protocol types](../crates/stage-protocol/src/)

When this document and code disagree, code is the structural truth and this document must be corrected. Theatre changes its project-owned tool and wire surfaces in place; it does not maintain compatibility aliases or versioned MCP schemas. The developer-facing GDScript marker API is different: calls made from game code are an external developer surface and require deliberate compatibility decisions.

## Naming rules

### Identifiers use `<resource>_id`

Any JSON or MCP field carrying an identifier for a named resource uses the resource name as its prefix:

```json
{"clip_id":"clip_abc", "watch_id":"watch_1", "session_id":"sess_2", "marker_id":"marker_3"}
```

A bare `id` is not valid for a resource identifier. This rule applies consistently to create responses, status responses, list entries, delete responses, query parameters, event payloads, and nested objects. List entries are not exempt: a clip list uses `clip_id`, not `id`.

Delete/remove responses echo the identifier being acted on. Prefer `{ "result": "ok", "clip_id": "clip_abc" }` over a boolean such as `{ "deleted": true }`, so callers can correlate the result without reconstructing context.

Identifiers used only as internal database primary keys are implementation details. Once an identifier crosses a controlled JSON boundary, the resource-prefixed name applies.

### Measurements use full descriptive names

Distance and length measurements use `distance`, never `dist`. This applies to nearest/radius entries, relationship results, inspection context, trajectories, and future measured values.

Use the Godot name when a field maps to a Godot property or method:

| Godot source | Contract field |
|---|---|
| `global_position` | `global_position` |
| `position` | `position` |
| `velocity` | `velocity` |
| `scale` | `scale` |
| `visible` | `visible` |
| `collision_layer` / `collision_mask` | same names |
| `get_class()` / `get_path()` | `class` / `path` |
| `is_on_floor()` | `on_floor` |
| `rotation_degrees` | `rotation_deg` |

The `rotation_deg` form is the Stage convention for the Godot rotation-degrees value. Stage-computed fields have descriptive snake_case names such as `relative`, `bearing`, `bearing_deg`, `distance`, `occluded`, and `timestamp_ms`; do not shorten them for convenience. The complete Godot mapping is maintained in [the Godot naming skill](../.agents/skills/godot-naming/GODOT-NAMING.md).

### Echo submitted names

When a response confirms or echoes a caller-supplied value, its field name is exactly the input field name. Do not invent a second vocabulary for the same value:

```json
{"watch":{"node":"player","track":["position"]}}
```

is echoed as:

```json
{"watch_id":"watch_1","node":"player","track":["position"]}
```

not as `watching` and `tracking`. This applies to nested fields and to operation responses as well as Stage results.

## Effect rules

A field in a typed MCP parameter struct is a promise to callers. Every accepted field must either:

1. be forwarded to the relevant engine/protocol operation;
2. affect server-side computation or response shaping; or
3. be explicitly rejected when set because the capability is not implemented.

A field that is accepted and silently ignored is a contract violation. It creates an affordance that agents will rely on while having no effect. If behavior changes by action or query variant, validate the required fields for that variant and return a useful invalid-parameter error rather than guessing.

This rule applies to optional fields too. Defaults belong in the typed parameter definition or a clearly owned server default, and the effective value must have observable effect. Generated schemas come from these same Rust parameter types; do not maintain a hand-written schema that claims more than the handler implements.

## Envelope rules

Use the response envelope that matches the meaning of the result:

- **`results`** is a plural array for ranked or filtered collections, including `nearest`, `radius`, and `area` queries.
- **`result`** is a singular object for one answer, including `raycast`, `path_distance`, and `relationship` queries.

Do not mix singular and plural forms for the same query semantics. An agent should be able to infer the envelope from whether the operation returns a collection or one answer.

Tool-specific metadata (`query`, `from`, `to`, frame numbers, budget reports, and operation details) may sit beside the envelope. It must not change the singular/plural meaning of the answer.

### MCP and operation errors

MCP handlers report invalid parameters, unavailable sessions, backend failures, and engine failures through the MCP error channel. Where the tool's result is an operation payload, its successful JSON may also contain a tool-specific `result` or `success` field; these are not substitutes for the MCP transport envelope.

Director's internal Godot operation protocol is normalized as:

```json
{"success":true,"data":{}}
```

or:

```json
{"success":false,"error":"...","operation":"...","context":{}}
```

The Rust Director server validates and converts this result before returning the typed MCP output. Stage's TCP protocol separately distinguishes `response` and `error` messages by the shared `Message` enum. Do not conflate an internal operation envelope, a Stage query answer, and an MCP protocol error.

Errors should preserve an actionable operation or node context where available. Do not turn an engine failure into a successful empty result: absence, truncation, unavailable capture, and failure are different states.

## Stage semantics

Stage serves the state of a running Godot project through MCP tools. The exact fields are generated from [`stage-server`](../crates/stage-server/src/mcp/) parameter and response types.

### Session and live state

- The Stage server connects to the addon's TCP listener. The addon sends the
  first handshake message; the server acknowledges a compatible protocol version.
  The current version is defined in [`handshake.rs`](../crates/stage-protocol/src/handshake.rs).
- A Stage MCP session owns its live connection, effective configuration, spatial index, delta baseline, watches, and session identifier. The engine owns a separate run identifier, which survives client reconnects and changes on process restart.
- When a selected Godot project is available, Stage checks the handshake project before connection publication or project configuration changes. `runtime_status` reports actual identity, current scene, and readiness; disconnected status does not claim last-known identity is current.
- Each one-shot CLI invocation creates fresh session state. A snapshot from one
  invocation cannot establish the next invocation's delta baseline, and watches
  do not persist across those invocations. The CLI rejects delta, watch operations,
  session configuration updates, and actions requesting a delta before connecting
  or changing the game. Use persistent MCP for these workflows. Empty configuration
  reads, ordinary observations/actions, and addon-owned clip operations remain available.
- A snapshot establishes or replaces the delta baseline. `spatial_delta` requires a baseline and reports changes since the last stored snapshot/delta; the current parameter surface does not provide a caller-supplied `since_frame` cursor.
- A game restart or disconnected connection resets live frame comparison. The server may reconnect and retain watch/config intent, but old live observations are not treated as current state.
- Live `spatial_action` mutations are temporary engine state. They are not saved into scenes or scripts; use Director or direct code authoring for a durable change.

### Observation and query families

- `spatial_snapshot` requests summary, standard, or full engine data from a camera, node, or point perspective. Radius and visibility/filter inputs narrow the returned set; the server computes relative position and response budgeting.
- `spatial_inspect` focuses on one node and can select categories such as transform, physics, state, children, signals, script, spatial context, and resources.
- `scene_tree` reports hierarchy independently of spatial calculations.
- `spatial_query` uses `results` for nearest/radius/area collections and `result` for raycast, path-distance, and relationship answers. Nearest/radius/area depend on a current spatial index; engine raycast and navigation work are delegated to Godot.
- `spatial_delta` reports applicable moved, state-changed, entered, exited, signal, and watch-trigger information. Empty categories may be omitted; omission means no entries were returned, not that the category is unsupported.
- `spatial_watch` stores node/group subscriptions in session state. Triggers are delivered through later delta results; adding a watch does not itself constitute a trigger.
- `spatial_config` changes the current session's static patterns, state-property selection, clustering, bearing format, internal-variable exposure, polling interval, and token hard cap. It returns the effective config.
- `spatial_action` validates fields by action type. In addition to pause, frame/time advance, teleport, property/method/signal operations, spawn/remove, it supports named input actions, key/mouse injection, and bounded interaction sequences. A sequence requires an already paused game, advances a bounded number of physics frames, releases its held actions on normal completion or supported cleanup paths, and leaves the game paused. It does not promise deterministic gameplay. `return_delta` only produces a useful delta when a baseline exists.
- `runtime_diagnostics` reads bounded errors, warnings, script errors and shader errors captured by the running game's native logger. Results identify the actual run and distinguish retained, evicted and response-omitted entries. Reads do not consume diagnostics. Capture starts at logger registration; engine initialization, disabled log streams and unavailable release backtraces are not recovered. A disconnected call does not return stale current-run evidence.
- `viewport` returns a bounded, aspect-preserving JPEG of the latest completed root-viewport render, with actual run identity and readback counters. It is independent of recording. Headless, missing-viewport and empty-pixel outcomes explicitly report unavailability; spatial observation remains available. A physics counter at readback is not the simulation frame represented by the pixels.
- `clips` manages the dashcam buffer and analyzes saved clips. Capture runs
  continuously while enabled; projects can disable dashcam startup. It covers markers, saves, status, list/delete, frame snapshots, trajectories, range conditions, frame diffs, events, screenshots, visual artifacts, and opaque dashcam configuration. Visual results may contain a text manifest and an image content block; unavailable screenshots and generation degradation are content-level outcomes, not proof that spatial capture failed.

Stage's response budget is approximate, is derived from serialized JSON size, and is capped by the session hard cap. Detail tiers and filtering are response-shaping semantics, not guarantees that every engine property is available. Engine state that is not exposed by the collector remains unavailable.

### Configuration precedence

For Stage's effective server configuration, session `spatial_config` overrides project defaults loaded from `stage.toml`; Godot project settings provide addon-side defaults where defined. Dashcam configuration is a separate runtime surface: `clips` config can apply it for the current connection, and an explicit project `[dashcam]` section is pushed after handshake. Do not assume every built-in default is pushed on every one-shot connection.

### Persistence ownership

The Stage addon owns capture buffers and clip persistence. It writes spatial frame data and optional screenshot data to SQLite in its configured user storage. The Stage server resolves the storage path and reads clips for analysis; it does not own the live capture buffer. This distinction matters when a game exits, when screenshots are unavailable, and when a server session is replaced.

## Director semantics

Director provides one MCP operation surface for Godot scene, resource, project, and editor utilities. Every operation includes `project_path`; scene and resource paths are interpreted relative to that project by the operation layer.

### Native serialization boundary

Use Director for `.tscn`, `.tres`, `.res`, and other changes that need Godot's resource system. Its GDScript operation modules use Godot classes to validate types, ownership, resources, UIDs, scene connections, and saving. Reading and diffing serialized files is supported. GDScript, shader source, and ordinary text remain direct code-owned files. Director's project utilities cover operations such as autoloads, project settings, UID updates, reload/diagnostics, and editor status; the Theatre CLI owns installation and project setup wiring.

`engine_api` queries the selected engine's ClassDB for one class at a time.
Start with a summary, then request focused member metadata with explicit pagination.
Defaults distinguish JSON values, Director serialization, text-only descriptions,
and unavailable values. Discovery is not a promise that every default can be
round-tripped into an authoring request.

The operation surface is grouped by scenes, nodes, resources, tile/grid maps, animation, shaders, physics layers, signals, batch/diff, and project utilities. The generated Director reference is the complete catalog; this document intentionally does not duplicate its parameter structs or large schemas.

### Backend selection

The Rust backend tries the editor plugin, then the persistent headless daemon,
then a headless one-shot process. Individual and batch calls share mutation
validation and scene-context routing. Open-scene edits preserve live objects and
use native undo, including actual partial changes from failed entries. They remain
unsaved until explicit save. Detached scene and resource operations write files.

Mutating responses report this operation's saved paths and changed unsaved scenes,
not the editor's complete dirty state. `scene_save` checks serialization and saves
only the selected scene. It retains undo and does not flush unrelated edited
external resources. The native editor dirty marker may remain afterward. A saved
scene reference does not imply that an externally edited resource was also saved.
Director verifies the editor-reported project root before dispatch and keys
cached reuse by canonical project and port. After editor dispatch, a transport
failure reports an unknown outcome instead of replaying the operation. Inspect
the result before retrying an edit whose response was lost.

Director `batch` runs operations sequentially and can stop on error. It is not atomic and has no rollback guarantee. A successful earlier operation remains successful if a later operation fails. Both batch modes retain per-entry results and partial persistence in error data. Reads and later entries observe earlier live changes; batch does not reload open scenes from disk.

`editor_run` controls a saved scene through a verified editor connection. Start
and restart require a selected scene; stop is idempotent and status is
observational. Launch suppresses the editor's native automatic save only around
the synchronous play request, then restores the previous setting. A successful
launch request reports editor play state, not Stage readiness. Use
`runtime_status` to establish the connected run and current scene. Director does
not replay run control on a headless backend.

Director resolves the standalone Godot executable from `GODOT_BIN`, then `GODOT_PATH`, then `godot` on `PATH`. An explicit path override is therefore an execution setting for the Director process, not an MCP operation field; `project_path` still identifies the Godot project being operated on.

### Director result and error behavior

The Rust layer validates `project_path`, resolves the standalone Godot executable, selects a backend, deserializes the structured Godot result, and returns typed data. A missing Godot executable, invalid project, timeout, backend failure, and operation-level engine error remain distinguishable in the server's error path. Do not represent a failed write as an empty successful data object.

## Human feedback semantics

The runtime and editor integrations publish deliberate feedback into the
selected project's `.theatre/feedback` directory. An item records its source,
scene, process or run identity, selection or pointer context, capture provenance,
and an optional note. An image can be unavailable while the remaining context
stays valid. The shared Rust types define the exact item, operation, and response
shapes.

Status and retrieval are non-destructive. Retrieval does not mark an item
handled. Handling suppresses pending notices for all readers but keeps the item
retrievable. Deletion is a separate explicit operation. Incomplete publication
storage is not feedback and requires an explicit cleanup request.

The queue is project-local and bounded. Admission preserves existing evidence
when storage is full and does not silently expire unhandled items. The ignore
rule prevents accidental version-control inclusion; it does not encrypt evidence
or restrict local access.

Stage, Director, and the Theatre CLI share this handling state. A pending notice
can be attached to later results without changing a tool's typed result, image
content, success meaning, or error meaning. Notice failures remain best-effort.
The optional Claude and Codex hooks inject only this textual notice at a
supported post-tool boundary. They neither consume evidence nor deliver images,
and they do not wake or steer an idle agent. An explicit `THEATRE_PROJECT_DIR`
in the client environment selects the hook's project and does not fall through
to an ancestor when invalid; when unset, the hook finds the nearest
`project.godot` above the tool event's working directory. Environment configured
only on the Stage MCP server does not propagate back to the client hook.

## Wire and transport rules

### Stage TCP

Stage frames are:

```text
[4-byte big-endian payload length][UTF-8 JSON payload]
```

The shared codec rejects payloads larger than 16 MiB. Stage uses typed handshake, query, response, error, and event messages from [`stage-protocol`](../crates/stage-protocol/src/messages.rs). The addon sends the initial handshake; the server sends an acknowledgement or a version-mismatch error. This protocol ships as a matched Stage unit, so compatibility aliases and negotiation beyond the current handshake are not part of the contract.

### Director TCP

Director editor and daemon connections use the same length-prefixed JSON framing utility, but their first message is an operation request rather than a Stage handshake. Requests carry `operation` and `params`; responses use the structured Director operation result described above. The one-shot path uses Godot subprocess stdout for the operation result and captures stderr separately.

### MCP stdio

`stage serve` and `director serve` use stdio JSON-RPC for MCP. Stdout is reserved for protocol output; logs belong on stderr. Direct CLI modes also emit machine-readable JSON on stdout, with diagnostics on stderr.

Pattern implementations and examples live under [`.agents/skills/patterns/`](../.agents/skills/patterns/); they support these rules but do not replace the typed code or generated schemas.
