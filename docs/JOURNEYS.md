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

Ask the user before installing or enabling missing Theatre components in a
project. Machine installation, project wiring and per-run activation are distinct.
Unattended init requires `--yes --accept-project-install`; existing addons and
MCP configuration require separate `--overwrite-existing` intent to replace.

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
- Ordinary launches keep Stage present, with observation and recording off by
  default. Enable observation explicitly for live access; configure capture
  phases independently.
- **Director** requires a `project_path` in every operation. It can use the editor plugin, a headless daemon, or a one-shot Godot process; the agent does not need to select the backend. The standalone Godot executable is resolved from `GODOT_BIN`, then `GODOT_PATH`, then `godot` on `PATH`.
- **Nested projects and switching:** initialize each Godot project once. Keep one root MCP configuration rather than loading duplicate nested configurations. `THEATRE_PROJECT_DIR` selects Stage's startup project; `project_select` switches the running MCP server explicitly without a restart. Selection discards watches, baselines, spatial indexes, session overrides and the cached clip location, even for the same project. Take a fresh snapshot and recreate watches afterward. A stopped or unreachable target remains selected and reconnecting; Stage never returns to the previous project automatically. Director continues to select its absolute `project_path` per call. Separate live games/editors need distinct listener ports, or stop the old process before reusing its port.
- **Feedback after switching:** Stage's tool results and feedback calls use the selected project's queue. A client feedback hook runs outside the MCP server and keeps its own environment/working-directory selection; `project_select` does not change that. Launch the client with the intended absolute `THEATRE_PROJECT_DIR` when its hook should select a nested queue. One-off Stage CLI calls still use explicit environment selection; `project_select` requires persistent MCP.
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

Ordinary runs record nothing. Ask before installing or enabling missing project
components; an installation is not permission for every launch.

1. Choose the evidence needed. Use `theatre run` or Director `editor_run.launch`
   with independent observation, startup and play options. Minimal records
   explicit metrics only; Light/Standard require spatial scope; Heavy permits
   broad coverage. Review effective sources and channel availability.
2. For asynchronous loading, add `StageRuntime.notify_ready()` at the project's
   actual successful initial-ready transition. Do not guess from elapsed time or
   one completed background task. Use the explicit scene fallback only for
   synchronous scenes.
3. Startup capture precedes target loading and normally saves at readiness.
   Human play then waits without capture; agent play starts at readiness unless
   an explicit preference overrides it. Missing readiness and capture bounds
   never manufacture a ready event.
4. Use Continue for a separate segment in the same recording or Start new for
   an independent recording. The gap remains uncaptured. Preset selection and
   `clips(config)` only stage next-play settings.
5. Mark moments while recording. Stop freezes a draft; Keep persists it and
   Discard removes only that draft. A failed Keep retains it for retry.
   Unkept drafts can be lost when the game exits.
6. Select rolling retention when recent history is needed. Automatic marker
   saves require separate on-trigger opt-in; they use a five-second post-window.
   Stop shortens pending coverage. Failed saving prevents automatic resumption.
7. Inspect saved segment metadata and markers, then `clips(metrics)` for
   counters/timing, spatial tools for recorded state, and images/artifacts only
   when captured. Do not infer historical properties from live inspect calls.

Provider callbacks are explicit and run only during active metric capture.
Use cheap already-computed values and unit-bearing names. Callback timing,
native probe timing and persistence cost are different measurements; presets
are not a promise about frame-time overhead.

Native controls support keyboard access without taking gameplay focus.
Drag their header to avoid game UI, or minimize to the single restore button.
Neither changes capture state; dragged placement and minimization last for the
run, with viewport changes keeping controls reachable. Release the game's mouse
before interacting: the non-modal child window isolates clicks from gameplay
event handlers, while captured input passes through. Global input polling still
belongs to the game's own UI policy. Marker/pause bindings and initial corner
placement remain configurable. Mark, Keep and Share note + still are separate
actions. Hiding controls does not disable their
shortcuts; an ordinary off launch creates neither controls nor shortcuts.

Saved segments remain available after the game exits using the project-local
storage hint. Each has its own settings, time range, run and requested target.
Use actual channel coverage; absent images, gaps and uncaptured intervals do not
prove that nothing happened. Old clips remain readable with unknown new metadata.

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
   selected saved scene without saving unrelated open work. On Windows, select
   `automated` for agent visual review, `deferred` when preparing later human
   testing, or `interactive` for a user-requested visible review. Background tests
   use headless execution when it retains the required evidence and automated
   presentation otherwise. Omission preserves legacy behavior. Linux and macOS
   retain existing graphical, headless, and wrapper conventions. A shell or manual
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

For a successful deferred Windows run, wait for the project's actual readiness
condition and use the normal agent response to say the scene is ready and identify
the checks the user should perform. The user focuses the ordinary Godot window;
there is no Theatre handoff command. If presentation is degraded, read the retained
launch/running facts, stop the run before fallback, and explain the limitation
instead of announcing it as ready.

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
3. Capture launch policy resolved before runtime activation, with `clips(config)`
   staging settings for the next play segment in that run. Legacy `[dashcam]`
   handshake settings do not override this policy.

The effective config is session state. It is not a replacement for the project's source-controlled configuration. Keep frequently reused defaults in project configuration and use session changes for focused investigations.

Director's editor port can be selected through `DIRECTOR_EDITOR_PORT` or its
project setting; its daemon port uses `DIRECTOR_DAEMON_PORT`. Stage's listener
port can be overridden through project settings or `THEATRE_PORT`; its server
also reads a project `stage.toml` connection port. Keep the actual listener and
client port settings aligned.

## Recover and narrow

### Stage cannot connect

Confirm the launch explicitly enables observation; an ordinary off launch is
not a broken installation. Then check the running project, Stage plugin/autoload,
matching extension and Godot versions, and listener port. If the extension is
missing, the GDScript layer degrades with a deployment error rather than providing
runtime data. A stopped game or dropped connection returns an unavailable-session
error; retry after the selected observation-enabled run is available.

### Stage returns no useful delta

A delta requires a live baseline. Take a fresh snapshot first. After a game restart, treat the previous baseline as invalid. Use a full snapshot when the question is about current state rather than change since an earlier observation.

### Director cannot use the editor

This is not necessarily a failure. Director next tries the headless daemon and then one-shot Godot. Inspect the structured operation error and Godot stderr when all paths fail. Confirm `project_path`, the Godot executable resolution, and that the project contains `project.godot`.

The native `editor_run` workflow is different: it requires the selected editor
and never substitutes a headless authoring backend or a second game launcher.

### Godot rejects a resource operation

Read the target with Director, verify node/resource types and required assigned resources, and rerun the smallest operation. Do not repair serialized text by hand as a fallback; that bypasses the native validation the operation exists to provide.

### Visual evidence is unavailable

Headless or editor-hint runs may still provide spatial frames while producing no
rendered screenshots. A graphical session alone also does not prove that a usable
capture backend exists. Check `screenshot_capture` in `clips` status for current
capability and its reason, separately from retained screenshot coverage.
Automatic readback uses the available native asynchronous OpenGL path or leaves
visual capture unavailable while spatial recording continues. If pixels are
necessary and that path is unavailable, synchronous readback is an explicit
recovery choice that can stall gameplay; it is not an automatic fallback.
Continue with spatial analysis when that is sufficient.

## Contract and source references

- Cross-boundary semantics and operation families: [`CONTRACT.md`](CONTRACT.md)
- Component ownership and process boundaries: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Stage schemas and descriptions: [`site/api/index.md`](../site/api/index.md)
- Director schemas and descriptions: [`site/api/director.md`](../site/api/director.md)
- Stage handler source: [`crates/stage-server/src/mcp/`](../crates/stage-server/src/mcp/)
- Director handler source: [`crates/director/src/mcp/`](../crates/director/src/mcp/)
- Godot operation modules: [`addons/director/ops/`](../addons/director/ops/)
