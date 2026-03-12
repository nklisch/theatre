# Director — Implementation Roadmap

## Scope Assessment

Director is a Godot editor-time MCP server: it lets an AI agent create and
modify scenes, resources, tilemaps, animations, and physics configuration
without touching `.tscn`/`.tres`/`.res` files directly. It pairs with Spectator
(runtime observer) to complete the build-test-iterate loop.

**Size estimate:** Medium-large. Director is architecturally comparable to
Spectator but more GDScript-heavy. Spectator's complexity lives in spatial
indexing and the GDExtension layer. Director's complexity lives in the GDScript
operations layer and the three-backend routing system.

**Rust side:** Thin. Tool handlers are ~10-15 lines each. All heavy lifting
delegates to GDScript or the operations format. Backend selection in
`backend.rs` is the most nuanced Rust logic.

**GDScript side:** The bulk of the work. Operations must correctly use Godot's
scene save/load APIs, handle type conversion via `get_property_list()`, produce
structured JSON for every success and failure, and stay headless-safe.

**Risk areas:**
- GDScript type conversion (`convert_value` + `get_property_list()`) — Godot's
  property types are underspecified and vary by node class
- Headless cold-start latency (~1-3s per one-shot op) — acceptable for dev use,
  but worth tracking UX
- Editor plugin TCP coordination — correct sequencing when editor opens/closes
  mid-session
- UID consistency — new scenes require UID registration or the editor's
  filesystem watcher may complain

---

## Prerequisites Before First Line of Code

### 1. Workspace restructuring

Add `crates/director` and `tests/director-tests` to `Cargo.toml` workspace
members. No new top-level changes needed to the existing Spectator crates.

```toml
# Cargo.toml — add to members
"crates/director",
"tests/director-tests",
```

### 2. Shared crate extraction (optional but recommended)

`VariantTarget` currently lives in `spectator-protocol`. The spec calls for
extracting it into a `theatre-common` crate so Director can import it without
depending on the full protocol crate. This can be deferred to Phase 3 when
`node_set_properties` first needs it — just import from `spectator-protocol`
directly until then.

### 3. Addon directory scaffold

```
addons/director/
  plugin.cfg
  plugin.gd        (stub — enables addon, starts TCP listener shell)
  operations.gd    (stub — parse_args + quit)
  daemon.gd        (stub)
  ops/             (empty)
```

The Godot addon must be present and loadable before any E2E test can run.

---

## Phase 1 — MVP: Read + Create + Basic Mutation

**Goal:** An agent can create a scene, add nodes, set properties, and read back
what it made. Everything headless one-shot. No daemon, no editor plugin.

### Rust crate: `crates/director`

- `Cargo.toml` — rmcp, serde_json, tokio, anyhow, thiserror
- `main.rs` — `director serve` subcommand (stdio MCP) + `director <op> <json>`
  CLI subcommand
- `server.rs` — `DirectorServer` struct, rmcp setup
- `resolve.rs` — `GODOT_PATH` env var → `which godot` resolution
- `oneshot.rs` — spawn `godot --headless --script operations.gd <op> <json>`,
  capture stdout, parse `OperationResult`
- `backend.rs` — for now: always one-shot; backend enum stub ready for later
- `mcp/mod.rs` — shared helpers (`serialize_params`, `finalize_response`, etc.)
  mirroring Spectator's pattern exactly
- `mcp/scene.rs` — `scene_read`, `scene_create`
- `mcp/node.rs` — `node_add`, `node_set_properties`, `node_remove`

### GDScript: `addons/director/operations.gd` + `ops/`

- `operations.gd` — `_init()` dispatcher: parse args, match op, print JSON,
  quit
- `ops/scene_ops.gd` — `op_scene_read`, `op_scene_create`
- `ops/node_ops.gd` — `op_node_add`, `op_node_set_properties`, `op_node_remove`
- Type conversion helper: `convert_value(value, expected_type)` using
  `get_property_list()`
- Structured error returns on every code path

### Tests: `tests/director-tests`

- `harness.rs` — `DirectorFixture` (GODOT_BIN + one-shot runner)
- `test_scene.rs` — `scene_create_then_read_round_trips`
- `test_node.rs` — `node_add_set_remove`
- `test_journey.rs` — create scene → add Node2D → set position → read back →
  assert position matches

**Exit criteria:** `cargo test -p director-tests -- --include-ignored` passes
all Phase 1 tests with `GODOT_BIN` set.

---

## Phase 2 — Scene Composition

**Goal:** Agent can compose scenes from existing scenes and reorganise the tree.

### New tools

- `scene_add_instance` — pack a scene and add as child
- `node_reparent` — move a node to a new parent within same scene
- `scene_list` — glob `.tscn` files in project, load each, return root type +
  node count
- `resource_read` — load a `.tres` resource, serialise properties to JSON

### GDScript additions

- `ops/scene_ops.gd` — `op_scene_add_instance`, `op_scene_list`
- `ops/node_ops.gd` — `op_node_reparent`
- `ops/resource_ops.gd` — `op_resource_read`

**Note:** `scene_list` with node count requires instantiating every scene
headlessly — may be slow for large projects. Consider returning node count as
optional or lazy.

**Deferred:**
- `scene_list`: Add `pattern: string?` for glob matching (e.g.
  `"scenes/**/*.tscn"`) once real projects need it. Contract-compatible.
- `scene_list`: Add `include_node_count: boolean?` (default true) opt-out for
  large projects where instantiating every scene is too slow.
- `resource_read`: Add `depth: number?` to control nested resource
  serialization depth. Default 1 (nested resources as path strings).
  Contract-compatible.

---

## Phase 3 — Headless Daemon

**Goal:** Eliminate 1-3s cold-start cost for multi-operation workflows.

### New Rust module: `crates/director/src/daemon.rs`

- Spawn `godot --headless --path <project> --script addons/director/daemon.gd`
- Wait for `{"status":"ready"}` on stdout before returning
- Maintain TCP connection to `:6550`, send operations as JSON, read responses
- Respawn once on connection failure
- `{"operation":"quit"}` on shutdown

### New GDScript: `addons/director/daemon.gd`

- Extends `SceneTree`
- TCP server on `:6550`, accepts one connection at a time
- Same JSON operation format as one-shot
- Prints `{"status":"ready"}` to stdout when listener is up
- Stays alive until `quit` operation received

### Backend selection update (`backend.rs`)

Route: one-shot → try `:6550` → if connected use daemon; if not connected,
spawn daemon first.

---

## Phase 4 — Resources & Materials

- `material_create` — `StandardMaterial3D`, `ORMMaterial3D`, `ShaderMaterial`
- `shape_create` — collision shapes: `BoxShape3D`, `SphereShape3D`,
  `CapsuleShape3D`, `ConcavePolygonShape3D`; optionally attach to node
- `style_box_create` — `StyleBoxFlat`, `StyleBoxTexture` for UI
- `resource_duplicate` — load resource, apply overrides, save to new path

**Type conversion note:** Material properties include `Color`, `Texture2D`
(resource path), and `bool`. All handled by the existing `convert_value` helper
but worth explicit test coverage.

---

## Phase 5 — TileMap & GridMap

- `tilemap_set_cells` — set cells by atlas coords; requires TileMapLayer node
- `tilemap_get_cells` — enumerate used cells in region or all
- `tilemap_clear` — clear region or entire map
- `gridmap_set_cells` — 3D GridMap cell placement

**Godot 4.3+ note:** `TileMap` is deprecated in favour of `TileMapLayer`.
Operations should detect which class is present and dispatch accordingly, or
require `TileMapLayer` and document it.

---

## Phase 6 — Animation

- `animation_create` — create `Animation` resource, set length and loop mode
- `animation_add_track` — add value/method/bezier track with keyframes
- `animation_read` — serialise all tracks and keyframes to JSON
- `animation_remove_track` — remove by index or node path

**Complexity:** Animation tracks reference node paths within the scene.
Keyframe values must be type-converted. This is the most GDScript-intensive
domain outside of scene manipulation.

---

## Phase 7 — Editor Plugin Backend

**Goal:** When the Godot editor is open with the Director plugin active,
modifications route through the live editor API instead of headless. Provides
immediate viewport feedback and undo history.

### New GDScript: `addons/director/plugin.gd`

- Extends `EditorPlugin`
- TCP listener on `:6551` (configurable via `DIRECTOR_EDITOR_PORT`)
- Receives JSON operations, dispatches to `editor_ops.gd`
- Returns same JSON response format as headless

### New GDScript: `addons/director/editor_ops.gd`

- EditorInterface API variants of all Phase 1-3 operations
- `EditorInterface.get_open_scenes()` to detect loaded scenes
- `EditorInterface.reload_scene_from_path()` after headless modifications if
  needed

### Rust: `crates/director/src/editor.rs`

- TCP client for `:6551`
- Same JSON send/receive as daemon client

### Backend selection update

Priority: `:6551` (editor) → `:6550` (daemon) → one-shot.
Creation of new files always skips `:6551` (editor plugin has no conflict to
worry about for new files, but consistency with spec: new files always headless).

---

## Phase 8 — Advanced Tools

- `visual_shader_create` — build VisualShader node graph from JSON node/edge
  description; complex but rarely needed
- `physics_set_layers` — set `collision_layer`/`collision_mask` bitmasks on a
  node
- `physics_set_layer_names` — write layer names to `project.godot`

---

## Phase 9 — Meta Operations & Utilities

- `batch` — execute multiple operations in one headless/editor invocation;
  reduces MCP round-trips significantly for level-design workflows
- `scene_diff` — structural diff between two scenes or between a scene and a
  git ref; useful for agent self-verification after a batch of changes
- `uid_get` — resolve UID for a file path
- `uid_update_project` — rescan project and regenerate missing UIDs
- `export_mesh_library` — export scene as MeshLibrary resource for GridMap use

`batch` has high leverage for the agent: creating a scene with 20 nodes
currently costs 21 MCP round-trips + 21 Godot cold-starts; with batch it costs
1. This should be prioritised as soon as Phase 1 is stable.

---

## Phase 10 — Scene Wiring & Deferred Features

**Goal:** Complete scene authoring coverage with signal connections, groups,
script attachment, metadata, and node search. Also finish three deferred
features from earlier phases.

### New tools

- `signal_connect` — connect signals between nodes in a scene
- `signal_disconnect` — remove signal connections
- `signal_list` — list all signal connections in a scene
- `node_set_groups` — add/remove node group memberships
- `node_set_script` — attach or detach a GDScript from a scene node
- `node_set_meta` — set/clear metadata entries on a node
- `node_find` — search scene tree by class, group, property, or name pattern

### Deferred features completed

- `scene_list` — add `pattern` glob filter param
- `resource_read` — add `depth` param for nested resource serialization
- `scene_diff` — add git ref support (`HEAD:path` syntax)

### GDScript additions

- `ops/signal_ops.gd` — `op_signal_connect`, `op_signal_disconnect`,
  `op_signal_list`
- `ops/node_ops.gd` — `op_node_set_groups`, `op_node_set_script`,
  `op_node_set_meta`, `op_node_find`

**Key implementation note:** Signal connections require `CONNECT_PERSIST` flag
to survive `PackedScene.pack()`. Group membership requires `persistent=true`
arg to `add_to_group()`. Both are easy to miss and cause silent data loss.

---

## Parallel Work: Project Rename to Theatre

The spec notes the repo should be renamed `spectator` → `theatre`. This is
independent of Director implementation and can happen any time:

- GitHub repo rename
- Root `Cargo.toml` workspace name (if set)
- `CLAUDE.md` and skill files updated to reference Theatre umbrella
- Root `README.md` rewritten to introduce both tools
- `docs/` audit for project-name vs tool-name references

Crate names (`spectator-*`) do not change.

---

## Dependencies & Ordering Constraints

```
Phase 1 (MVP)
    └── Phase 2 (Composition)       — extends Phase 1 GDScript layer
        └── Phase 4 (Resources)     — new ops domain, no Phase 2 dependency
        └── Phase 5 (TileMap)       — new ops domain, no Phase 2 dependency
        └── Phase 6 (Animation)     — new ops domain, no Phase 2 dependency
Phase 3 (Daemon)                    — parallel with Phase 2; depends on Phase 1
    └── Phase 7 (Editor Plugin)     — extends backend.rs from Phase 3
Phase 8 (Advanced)                  — any order after Phase 1
Phase 9 (Meta)                      — `batch` should come right after Phase 1;
                                       rest can be late
Phase 10 (Wiring + Deferred)        — after Phase 1; deferred items touch
                                       Phase 2/4/9 tools
```

Phases 4, 5, 6 are independent of each other and of Phase 3 — implement in
any order based on what the target project needs first.

---

## Key Design Decisions Already Made

| Decision | Detail |
|---|---|
| Language | Rust (rmcp + tokio) for MCP server; GDScript for all Godot API calls |
| IPC | stdout JSON for one-shot; TCP JSON for daemon and editor plugin |
| Ports | `:6551` editor plugin, `:6550` headless daemon |
| Project path | Required param on every tool — no global state |
| Binary | Single `director` binary: `director serve` + `director <op> <json>` |
| New files | Always headless, even when editor is running |
| Reads | Always headless (stateless) |
| Modifications | Editor API when available, headless otherwise |
| Error format | `{"success":false,"error":"...","operation":"...","context":{}}` always |
| Undo | Editor plugin ops enter undo history; headless ops do not |

---

## What Director Does NOT Do

- Parse or emit `.tscn`/`.tres` text format by hand — always uses Godot's API
- Provide a visual UI — agent-facing only
- Handle binary `.res` resources directly — `resource_read` covers `.tres`;
  `.res` files require export-style operations
- Manage Godot version or project upgrades
- Replace GDScript editing — `.gd` files are edited directly by the agent via
  filesystem; Director handles only what requires Godot's own serialiser

---

## Relationship to Spectator

Director and Spectator share this repository, workspace, and conventions but
have no runtime coupling. Expected agent workflow:

1. **Director** — create or modify scene structure, materials, physics layers
2. **Agent** — edit `.gd` scripts directly (filesystem)
3. **Spectator** — launch game, observe spatial state, verify behaviour
4. Iterate

Both tools must be installed for the full loop. Neither requires the other to
function.
