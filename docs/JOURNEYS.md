# Theatre Journeys

This document describes the operating loops that Theatre is built to support. It is for contributors and coding agents: start with the journey that matches the task, then use the linked contract and source when details matter.

## The short version

Theatre supports this working loop:

```text
set up -> author -> save -> run -> observe -> act -> verify -> persist the fix
```

A verified open editor can start, stop, and restart a selected saved scene through
Director. Stage separately establishes when the running scene is ready.

Director owns durable Godot authoring. Stage owns live-game observation and explicit debugging actions. The CLI owns installation and project wiring. A runtime action changes only the current game session; a permanent fix belongs in project code or a Godot resource authored through Director.

## Choose a journey

| Goal | Start here | Finish with |
|---|---|---|
| Connect an existing Godot project | [Setup and connect](#setup-and-connect) | Both MCP servers discoverable, as selected |
| Understand a running game | [Observe a scene](#observe-a-scene) | A focused snapshot, inspection, or query |
| Diagnose behavior over time | [Observe, act, verify](#observe-act-verify) or [Record and analyze](#record-and-analyze) | Evidence at a known frame or state |
| Change scenes or resources | [Author with Director](#author-with-director) | A Godot-serialized change and validation |
| Prove a change in the engine | [Build, run, verify](#build-run-verify) | A real running-game observation |
| Share a human observation | [Share feedback](#share-feedback) | Retained context that the agent can retrieve explicitly |
| Recover from a connection or backend issue | [Recover and narrow](#recover-and-narrow) | A known available backend or an actionable error |

## Setup and connect

### Install once

1. Install Theatre so the `theatre`, `stage`, and `director` executables and addon templates are available in the user-level share location.
2. In a Godot project, run project setup or deploy the addon payload.
3. Enable the plugins selected for the project. Stage also needs its `StageRuntime` autoload when runtime observation is wanted.
4. Generate or review `.mcp.json` so the agent starts `stage serve` and/or `director serve` over MCP stdio.
5. On Windows, deploy with the platform-aware CLI and ensure the checkout or copied addon contains the correct platform GDExtension. Native Git symlink support is required only for the repository's linked-addon development workflow.

`init` is project setup; `deploy` rebuilds and updates an existing installation; `enable` changes plugin enablement without copying files. Setup may perform an initial Godot import when a Godot executable is available. Contributor setup guidance is in [`AGENTS.md`](../AGENTS.md); verification
commands are in [`.work/CONVENTIONS.md`](../.work/CONVENTIONS.md).

### Start a session

Use Director `editor_status` to identify the responding project/process and Stage
`runtime_status` to identify the running game. A ready runtime has a current scene
that completed its ready notification; an editor connection alone does not prove
this. Compare engine run identifiers across restarts, not client session identifiers.

- **Stage** connects to the running project's Stage listener on `127.0.0.1:9077` by default. The game must be running before a useful Stage query can complete.
- **Director** requires a `project_path` in every operation. It can use the editor plugin, a headless daemon, or a one-shot Godot process; the agent does not need to select the backend. The standalone Godot executable is resolved from `GODOT_BIN`, then `GODOT_PATH`, then `godot` on `PATH`.
- **Nested projects and switching:** initialize each Godot project once. A root MCP configuration can select a nested project for Stage with `THEATRE_PROJECT_DIR`; do not also load the nested generated MCP configuration. Restart the Stage MCP process or begin a new agent session after changing its startup environment. One-off Stage CLI calls can override `THEATRE_PROJECT_DIR` directly. The Stage MCP entry's environment applies only to that subprocess; launch the client with the same absolute variable when its optional feedback hook should select the nested queue. An explicit hook selection does not fall through to another project. Director switches per call through its absolute `project_path`. Stop an old game before another project uses the same local ports.
- Both MCP servers use stdout for MCP protocol traffic and stderr for logs. Do not use log output as a data channel.

After the Stage server connects, the addon sends the initial handshake. If the
protocol versions do not match, the session is rejected rather than interpreting
incompatible messages. A new game session also means a new live frame history.

Use persistent MCP for workflows that need baselines, watches, or session
configuration across calls. Each one-shot Stage CLI invocation starts fresh:
a CLI snapshot followed by a separate CLI delta does not share a baseline.
The CLI rejects delta, watches, session configuration updates, and actions with
`return_delta` before connection or mutation. Use `stage serve` through an MCP
client for these workflows; configuration reads and ordinary actions still work.

## Observe a scene

Use this as the default Stage investigation path:

1. **Orient with structure.** Use `scene_tree` when the node hierarchy or a node path is unknown.
2. **Take a summary snapshot.** Start with `spatial_snapshot` at summary detail to establish the current frame, scene dimensions, and broad spatial state.
3. **Narrow the question.** Use groups, classes, radius, or a lower token budget rather than repeatedly requesting the whole scene.
4. **Inspect one node.** Use `spatial_inspect` for selected transform, physics, state, children, signals, script, spatial context, or resources.
5. **Ask geometry questions.** Use `spatial_query` for nearest/radius/area, raycast, path distance, or the relationship between two origins.
6. **Establish a change baseline.** The snapshot establishes the server's delta baseline. Call `spatial_delta` after the next meaningful interval or action.

The engine supplies raw observations; the server calculates relative positions, bearings, indexes, deltas, watches, and response budgets. A snapshot is current to the most recently collected physics frame, not a promise of a frozen world.

## Observe, act, verify

Use this loop when reproducing or testing behavior without changing files:

1. Snapshot or inspect the relevant nodes.
2. If repeated observation matters, add a `spatial_watch` for a node or group and select the properties or conditions that matter.
3. Apply one explicit `spatial_action`: pause/resume, advance frames or time while paused, teleport, set a property, call a method, emit a signal, spawn/remove a node, or inject input.
4. Request a delta. `return_delta` can attach the follow-up delta to an action when a baseline exists; without a baseline, the response explains that a snapshot is needed first.
5. Compare the observed result with the intended behavior.
6. If the result suggests a durable change, stop using live mutation and author the fix through the appropriate code or Director journey.

Actions are debugging controls, not persistence. They can invoke arbitrary node methods and change the live scene, so agents should describe consequential actions in their user-facing summary. Stage's addon exposes activity and marker signals for the human-facing dock.

## Record and analyze

Stage's dashcam buffers spatial frames while active and can also buffer rendered screenshots when a graphical runtime permits capture. It is not a start/stop recording API:

1. Check `clips` status to confirm dashcam state and capture health.
2. Mark a moment with an agent marker, a human marker, or a game-code marker, or force-save the current buffer.
3. Select the resulting clip explicitly when more than one exists; otherwise analysis defaults to the most recent clip.
4. Use markers to locate the investigation window.
5. Use `snapshot_at` for state at a frame, `trajectory` for a node's time series, `query_range` for conditions, `diff_frames` for before/after comparison, and `find_event` for recorded events.
6. Use `screenshot_at` or a deterministic `visual_artifact` when visual evidence is useful and screenshots were captured.
7. Treat gaps, unavailable screenshots, and degraded artifact responses as evidence limits, not as proof that nothing happened.

The addon owns capture buffers and writes clip SQLite files under its configured user storage. The server reads those files for analysis. Spatial clip data can exist in headless runs; rendered screenshots require a usable graphical display and capture path.

Markers have different origins and trigger tiers. Code markers can be deliberate, system, or silent; silent markers annotate without triggering a clip. System anomaly capture is rate-limited. The relevant capture configuration and status are part of the `clips` contract rather than this workflow overview.

## Author with Director

Use Director whenever a change must be represented by Godot scene/resource serialization:

1. Read the target scene or resource first. Direct file reads and diffs are useful alongside Director summaries.
2. Use `engine_api` when a class, property type, signal or enum is uncertain. Start with a class summary and narrow to the relevant member.
3. Choose the smallest operation that expresses the change: create/read/list scenes, add/remove/reparent/find nodes, set properties/groups/scripts/metadata, instance scenes, create or duplicate resources, edit tile/grid cells, create or edit animations/shaders, configure physics layers, wire signals, or use project utilities.
4. Supply the target `project_path`; scene and resource paths are project-relative operation inputs.
5. Let Director route to the editor plugin, daemon, or one-shot path.
6. Read the result and, for a multi-step change, use `batch` only when sequential execution is sufficient. A batch stops or continues according to `stop_on_error`; it does not roll back earlier successful operations.
7. Re-read or diff the result, then run `project_reload` after direct script edits when Godot validation is needed.

Director uses Godot's own APIs to preserve resource references, UIDs, types, owners, and serialization details. Do not hand-construct `.tscn`, `.tres`, or `.res` files. Edit GDScript, shader source, and ordinary text directly, then use Director's project and validation operations where applicable. For unusual procedural construction that typed operations express poorly, an ordinary project-owned GDScript is the supported escape hatch. Theatre does not provide a general script-execution operation.

The editor backend uses the actual root of any open target scene. Individual and
batch mutations create native undo entries and remain unsaved until `scene_save`.
The save operation serializes only the selected scene, retains undo, and does not
flush unrelated external resources. Its native dirty marker may remain. Detached
headless scene and resource operations persist their target files. Read each
operation's persistence data, especially after a partial batch failure.

Without an editor, the daemon provides a persistent headless process; if it
cannot start or answer, Director falls back to one-shot execution. Editor routing verifies Godot’s actual project root before dispatch and checks
the project and port when reusing a connection. If a dispatched edit loses its
response, inspect the editor before retrying: Director does not replay that
uncertain edit on a different backend.

## Build, run, verify

This is the preferred cross-tool journey:

1. Use direct code edits for scripts and Director for serialized scenes/resources.
2. Use `project_reload` or an equivalent Godot-backed validation pass after script changes.
3. With a verified open editor, use Director `editor_run` to start or restart a
   selected saved scene without saving unrelated open work. A shell or manual
   editor launch remains valid when run control is unavailable.
4. Use `runtime_status` to verify the current run and readiness. A successful
   Director launch request alone does not establish Stage readiness. Read
   `runtime_diagnostics` for captured errors from that run, not historical editor
   log lines. Then use `scene_tree`, `spatial_snapshot`, `spatial_inspect`, and
   targeted queries to verify the real engine state.
5. Use actions only to set up a temporary test scenario; do not mistake a successful runtime mutation for a saved fix.
6. Use `viewport` for the latest completed root-viewport render, without enabling
   recording or saving a clip. Its readback counters do not make the image atomic
   with a separate spatial query. For temporal behavior, capture a marker and
   analyze the saved clip instead; clip screenshots and visual artifacts remain
   retained-evidence operations.
7. Persist the accepted fix in code or a Director-authored resource, then repeat the real loop.

For a bounded input script, pause the game and use an `interaction_sequence` action.
It applies named InputMap changes across selected physics-frame counts and releases
its held inputs during supported completion and cleanup paths. It leaves the game
paused for follow-up state and viewport observation. It does not make gameplay
deterministic, and a stopped or natively hung engine cannot run cleanup callbacks.

Contributor verification has separate pure, transport, Godot operation, and live-engine layers. A fast schema or unit check can show that a boundary is well formed; it cannot prove that Godot loaded the addon or serialized the resource correctly. The complete test requirements and commands live in [`.work/CONVENTIONS.md`](../.work/CONVENTIONS.md).

## Share feedback

A developer can deliberately share evidence from either the running game or the
Godot editor:

1. Use **Share feedback** or its configured shortcut in the relevant Godot surface.
2. Review the captured viewport and copied context in the native composer.
3. Add an optional note and queue the item explicitly.
4. Let the agent follow a pending notice or call `feedback` status.
5. Retrieve the matching item. This does not consume or handle it.
6. Handle the item after addressing it, or delete it explicitly when it is no longer needed.

Runtime feedback captures the root viewport and pointer context without pausing
the game. Editor feedback captures the active 2D or 3D scene viewport and current
selection without changing the selection, scene dirty state, or saved files. A
headless or unavailable viewport can still produce useful context and a note.
Feedback remains in the project-local `.theatre/feedback` directory after the
engine exits.

Stage, Director, and the Theatre CLI share handling state. Handling suppresses
future pending notices for all readers but preserves retrieval. Optional Claude
and Codex hooks can add the notice to a later post-tool response after explicit
installation and trust. They do not deliver image data, handle evidence, wake an
idle agent, or provide asynchronous steering.

## Configure the session

Stage configuration has three practical scopes:

1. Project defaults loaded from `stage.toml` and Godot project settings where supported.
2. The current Stage MCP session's `spatial_config` overrides for tracking, state properties, clustering, bearings, internal variables, polling, and token hard cap.
3. Dashcam-specific runtime configuration through the clips config action; an explicit project `[dashcam]` section is pushed after handshake.

The effective config is session state. It is not a replacement for the project's source-controlled configuration. Keep frequently reused defaults in project configuration and use session changes for focused investigations.

Director's editor port can be selected through `DIRECTOR_EDITOR_PORT` or its
project setting; its daemon port uses `DIRECTOR_DAEMON_PORT`. Stage's listener
port can be overridden through project settings or `THEATRE_PORT`; its server
also reads a project `stage.toml` connection port. Keep the actual listener and
client port settings aligned.

## Recover and narrow

### Stage cannot connect

Confirm the project is running, the Stage plugin/autoload is enabled, the extension binary matches the host platform and Godot version, and the listener port is consistent. If the extension is missing, the GDScript layer intentionally degrades rather than crashing, but no runtime data will be available. A stopped game or dropped connection returns an unavailable-session error; retry after the game is running.

### Stage returns no useful delta

A delta requires a live baseline. Take a fresh snapshot first. After a game restart, treat the previous baseline as invalid. Use a full snapshot when the question is about current state rather than change since an earlier observation.

### Director cannot use the editor

This is not necessarily a failure. Director next tries the headless daemon and then one-shot Godot. Inspect the structured operation error and Godot stderr when all paths fail. Confirm `project_path`, the Godot executable resolution, and that the project contains `project.godot`.

### Godot rejects a resource operation

Read the target with Director, verify node/resource types and required assigned resources, and rerun the smallest operation. Do not repair serialized text by hand as a fallback; that bypasses the native validation the operation exists to provide.

### Visual evidence is unavailable

Headless or editor-hint runs may still provide spatial frames while producing no rendered screenshots. Check `clips` status and screenshot metadata, then continue with spatial analysis or run a graphical session when the question genuinely depends on pixels.

## Contract and source references

- Cross-boundary semantics and operation families: [`CONTRACT.md`](CONTRACT.md)
- Component ownership and process boundaries: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Stage schemas and descriptions: [`site/api/index.md`](../site/api/index.md)
- Director schemas and descriptions: [`site/api/director.md`](../site/api/director.md)
- Stage handler source: [`crates/stage-server/src/mcp/`](../crates/stage-server/src/mcp/)
- Director handler source: [`crates/director/src/mcp/`](../crates/director/src/mcp/)
- Godot operation modules: [`addons/director/ops/`](../addons/director/ops/)
