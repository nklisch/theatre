# Director — MCP Server Design Spec

## Overview

Director is an MCP server that exposes Godot Engine's internal APIs for operations that are too fragile or complex to perform via raw filesystem editing. It is one of two MCP tools in the **Theatre** toolkit (this repository):

- **Director** (this spec): Scene/resource manipulation at editor-time
- **Spectator** (sibling tool): Runtime spatial observation of a running game

The agent handles scripts (`.gd`), project config (`project.godot`), and simple text files directly through filesystem access. Director handles everything that needs Godot's own API to serialize correctly.

Director uses two backends depending on context, selected automatically:

- **Editor backend**: When the Godot editor is running with the Director plugin active, modification operations are routed through the editor's live API. Changes appear immediately in the viewport, modifications enter the undo history, and there is no stale-scene conflict.
- **Headless backend**: When the editor is not running (CI, remote, editor closed), operations spawn `godot --headless`. Creation of new files always uses this path regardless of editor state — new files have no open-editor conflict to worry about.

The tool surface is identical in both cases. Backend selection is invisible to the agent.

## Repository Location

Director lives in this repository (`theatre`) as a sibling crate to Spectator:

```
crates/
  spectator-server/     — Spectator MCP binary (runtime observer)
  spectator-godot/      — GDExtension cdylib loaded by Godot
  spectator-protocol/   — Shared TCP wire format + VariantTarget types
  spectator-core/       — Shared spatial logic
  director/             — Director MCP binary (editor-time operator)
addons/spectator/       — Godot addon (GDScript + GDExtension manifest)
docs/
  director-spec.md      — This document
  ...
tests/
  wire-tests/           — Spectator E2E wire tests (Rust)
  director-tests/       — Director E2E operation tests (Rust)
  godot-project/        — Shared headless test project
```

## Pending: Project Rename to Theatre

The repository is currently named `spectator`. Before or alongside the Director implementation, the project should be aligned to the Theatre name:

- **Repo rename**: `spectator` → `theatre` (GitHub repo rename + update any clone URLs)
- **Workspace name**: Update `name` in root `Cargo.toml` if set
- **CLAUDE.md**: Update repo-level instructions to reference Theatre as the umbrella project
- **Spectator skill file** (`spectator-dev` skill): Update orientation text to describe Theatre as the project containing both Spectator and Director
- **Distributed skill files**: The `spectator` and `director` skills that ship with the tools should reference Theatre as the parent project and explain the two-tool setup
- **README**: Rewrite root README to introduce Theatre, then describe each tool
- **docs/**: Audit existing design docs for `spectator`-as-project-name references vs `spectator`-as-tool-name (the tool name stays, only the project umbrella name changes)

The crate names (`spectator-server`, `spectator-godot`, `spectator-protocol`, `spectator-core`) do not need to change — they refer to the Spectator tool, not the project.

## Design Principles

1. **Only expose what the filesystem can't do well.** If `cat` and `sed` can handle it reliably, it doesn't belong here.
2. **Godot is the API.** Every mutation goes through Godot's own classes (PackedScene, ResourceSaver, ClassDB). We never construct `.tscn`/`.tres` text by hand.
3. **Operations are independently testable.** The GDScript operations layer is callable directly from terminal: `godot --headless --script operations.gd <op> '<json>'`. The MCP server is a thin typed wrapper.
4. **Lean tool count, grow organically.** Start with what you need, add domains as projects demand them. No speculative tooling.
5. **Detect and adapt.** Director checks whether the editor is running and routes accordingly. The editor is never required, but is preferred for modifications when available. Same tool surface, two backends.
6. **Creation is always headless.** New files have no open-editor conflict. Creating a `.tscn` or `.tres` headlessly while the editor is running is safe — the editor's filesystem watcher detects the new file automatically.
7. **Consistent with Spectator conventions.** Same contract rules, same error layering, same MCP response patterns. An agent that knows Spectator should find Director immediately familiar.

## Architecture

Two backends, one tool surface:

```
MCP Client (Claude Code, Cursor, etc.)
    │
    │ stdio (MCP protocol)
    │
Director MCP Server (Rust / rmcp / tokio)
    │
    ├─── Editor running? ──yes──▶  TCP :6551 (Director EditorPlugin)
    │                                │
    │                                │ EditorInterface API
    │                                │ (live viewport, undo history)
    │
    └─── Editor absent, or ──────▶  tokio::process::Command
         creating new file            │
                                      │ godot --headless --script operations.gd
                                      │
                                      ▼
                              Project filesystem (.tscn, .tres, .res)
```

### Backend selection rules

| Operation type | File exists? | Editor running? | Backend |
|---|---|---|---|
| Read (`scene_read`, `resource_read`) | Either | Either | Headless |
| Create new file (`scene_create`, `material_create`, etc.) | No | Either | Headless |
| Modify existing (`node_add`, `node_set_properties`, `tilemap_set_cells`, etc.) | Yes | Yes | Editor API |
| Modify existing | Yes | No | Headless |

Reads are always headless — they're safe and stateless regardless. Headless for creation avoids any open-editor conflict since the file doesn't exist in the editor yet; the editor's filesystem watcher picks it up automatically after creation. Modifications go through the editor API when available, eliminating the stale-scene problem.

### Key decisions

- **Rust + rmcp + tokio** for the MCP server. Matches Spectator's stack exactly. Same `#[tool_router]`, `Parameters<T>`, `McpError` patterns. Same workspace, same build command (`cargo build --workspace`).
- **Editor detection via plugin port.** Director tries to connect to the Director EditorPlugin on `:6551` (default, configurable via `DIRECTOR_EDITOR_PORT` env var or `project.godot` setting). If it connects, the editor backend is available. If not, headless is used. No LSP port probing — detection and capability are tied to the same connection. Configurable port handles multiple Godot projects open simultaneously.
- **EditorPlugin as the editor backend.** A minimal GDScript EditorPlugin (`addons/director/plugin.gd`) listens on `:6551` and accepts JSON operation commands. It dispatches to `EditorInterface` API calls. The plugin is part of the Director addon — users install it once and it auto-connects when the editor is open.
- **Single GDScript file** (`operations.gd`) handles all headless operations. Extends `SceneTree`, runs in `_init()`, dispatches via `match` on the operation name.
- **One headless process per operation (one-shot mode).** Default headless mode. Spawns `godot --headless` for each operation via `tokio::process::Command`, exits when done. Stateless and reliable. Cold-start cost of ~1-3s per operation.
- **Persistent headless process (daemon mode).** Optional headless mode. A single Godot instance stays alive with a TCP command server, eliminating cold-start overhead. Subsequent operations complete in ~50ms. See Execution Modes below.
- **JSON in, JSON out.** All three paths (editor plugin, one-shot, daemon) speak the same JSON operation/response format. The Rust server normalises responses before returning to the MCP client.
- **Project path comes from the agent.** Every tool takes `project_path` as a required param. No global state.
- **CLI subcommand for direct use.** The `director` binary supports both `director serve` (MCP server) and `director <operation> '<json>'` (direct CLI invocation). Same binary, same resolution logic.

### Execution Modes

Three execution modes exist. The Rust server selects automatically based on what's available, in priority order: editor plugin → headless daemon → headless one-shot.

#### Editor plugin (preferred for modifications)

When the Director EditorPlugin is running in the Godot editor:

```
Director → TCP :6551 → EditorPlugin (plugin.gd) → EditorInterface API → live scene
```

Changes appear immediately in the viewport. Modifications enter the undo history. No file-reload required. The plugin is a persistent listener — no lifecycle management needed beyond the editor being open.

#### Headless daemon (preferred when editor is absent)

A persistent headless Godot process runs a TCP command server. Eliminates cold-start after the first operation (~50ms per subsequent call):

```
                                    ┌─────────────────────────────────────┐
Director → TCP (localhost:6550) →   │ godot --headless --path <project>   │
           JSON command/response    │   autoload: daemon.gd               │
                                    │   TCP server on :6550               │
                                    └─────────────────────────────────────┘
```

Lifecycle:
1. On first headless tool call: try `:6550`. If not listening, spawn daemon and wait for `{"status": "ready"}` on stdout.
2. Send operation as JSON, read JSON response.
3. On connection failure (Godot crashed): respawn once and retry.
4. On shutdown: send `{"operation": "quit"}`.
5. Fallback: if daemon fails to start, fall through to one-shot.

#### Headless one-shot (always available)

Every call spawns a fresh Godot process:

```
Director → godot --headless --script operations.gd <op> <json> → exits
```

Stateless, always clean, works in CI. Cold-start cost of ~1-3s per operation.

#### CLI and config

```bash
# CLI — auto-selects backend
director scene_read '{"scene_path":"scenes/player.tscn"}'

# Daemon control
director daemon start
director daemon stop

# MCP server
director serve
```

```json
{
  "mcpServers": {
    "director": {
      "command": "director",
      "args": ["serve"]
    }
  }
}
```

## Shared Foundations with Spectator

Director inherits the following from the Theatre codebase. These are not aspirational — they are existing, tested patterns to follow directly.

### VariantTarget (spectator-protocol)

`VariantTarget` is the type conversion system for JSON → Godot types. Director's `node_set_properties` operation needs exactly this. It will be extracted from `spectator-protocol` into a `theatre-common` crate so both Spectator and Director import it as a proper shared dependency.

```rust
// Already exists — Director uses this directly
pub enum VariantTarget {
    Nil, Bool(bool), Int(i64), Float(f64), String(String),
    Vector2(f64, f64), Vector3(f64, f64, f64),
    Array(Vec<VariantTarget>), Dictionary(Vec<(String, VariantTarget)>),
}
impl VariantTarget {
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> { ... }
}
```

### MCP Tool Pattern (mcp-tool-handler)

Director's tool handlers follow the same pattern as Spectator's. See `.claude/skills/patterns/mcp-tool-handler.md` for the full definition. In brief:

```rust
#[tool_router(vis = "pub")]
impl DirectorServer {
    #[tool(description = "...")]
    pub async fn scene_read(
        &self,
        Parameters(params): Parameters<SceneReadParams>,
    ) -> Result<String, McpError> {
        let result = run_operation("scene_read", &params).await?;
        finalize_response(&mut result, budget_limit, hard_cap)
    }
}
```

The same helper functions apply: `serialize_params`, `deserialize_response`, `parse_enum_param`, `require_param!`, `finalize_response`. These live in `mcp/mod.rs` in Spectator; Director has its own copy in `crates/director/src/mcp/mod.rs` following the same structure.

### Error Layering (error-layering)

Same three-layer pattern as Spectator. See `.claude/skills/patterns/error-layering.md`:

- `OperationError` (library, `Display + Error`) — subprocess/parse failures
- `anyhow::Result` — internal async tasks, main setup
- `McpError` — tool handlers (`internal_error` / `invalid_params`)

No `.unwrap()` in library code. Unwrap OK in tests and `main.rs` setup.

### Contract Rules (contracts)

Director follows the same wire format rules as Spectator. See `.claude/rules/contracts.md`:

- ID fields: always `<resource>_id`, never bare `id`
- Distance fields: always `distance`, never `dist`
- Schema fields must be forwarded or return an explicit error — no silent ignoring
- `results` (plural array) for ranked/filtered lists; `result` (singular) for single answers

### Serde Conventions (serde-tagged-enum)

Operation request/response types use the same serde conventions as Spectator. See `.claude/skills/patterns/serde-tagged-enum.md`:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationResult {
    Ok { data: serde_json::Value },
    Error { message: String, operation: String },
}
```

### stdout Is Sacred

`director` (the MCP binary) uses stdout for the MCP protocol. ALL logging goes to stderr via `tracing` / `eprintln!`. Never `println!` in the MCP server. The GDScript operations layer uses stdout for JSON output by design — that's its communication channel with Director, not the MCP channel.

## Test Harness

Director has its own E2E test harness in `tests/director-tests/`, following the same pattern as `tests/wire-tests/` but for subprocess/stdout communication instead of TCP.

```rust
/// A Director operation runner for E2E tests.
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
}

impl DirectorFixture {
    pub fn new(project_dir: PathBuf) -> Self {
        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        Self { godot_bin, project_dir }
    }

    pub fn run(&self, operation: &str, params: serde_json::Value) -> anyhow::Result<OperationResult> {
        let output = std::process::Command::new(&self.godot_bin)
            .args([
                "--headless",
                "--path", &self.project_dir.to_string_lossy(),
                "--script", "addons/director/operations.gd",
                operation,
                &params.to_string(),
            ])
            .output()?;
        serde_json::from_slice(&output.stdout).map_err(Into::into)
    }
}
```

Tests follow the `#[ignore = "requires Godot binary"]` convention from wire-tests:

```rust
#[test]
#[ignore = "requires Godot binary"]
fn scene_create_then_read_round_trips() {
    let f = DirectorFixture::new(test_project_dir());
    f.run("scene_create", json!({ "scene_path": "tmp/test.tscn", "root_type": "Node2D" }))
        .unwrap().unwrap_ok();
    let result = f.run("scene_read", json!({ "scene_path": "tmp/test.tscn" }))
        .unwrap().unwrap_ok();
    assert_eq!(result["root"]["type"], "Node2D");
}
```

The shared `tests/godot-project/` is extended with Director-specific test scenes. Director tests add scenes for manipulation; Spectator tests use scenes for runtime observation. Same project, different scenes.

The `GODOT_BIN` env var convention and `assert_approx` helper are shared from the wire-tests harness.

## GDScript Operations Layer

The `operations.gd` file is the only file that touches Godot's API. It follows a strict pattern:

```gdscript
extends SceneTree

func _init():
    var args = parse_args()
    var result = {}

    match args.operation:
        "scene_read": result = op_scene_read(args.params)
        "scene_create": result = op_scene_create(args.params)
        "node_add": result = op_node_add(args.params)
        # ... etc

    # Output result as JSON on stdout
    print(JSON.stringify(result))
    quit()
```

### Type Conversion System

Central to the GDScript layer is a type conversion system that maps JSON values to Godot types. This mirrors `VariantTarget` on the Rust side — the GDScript implementation is the runtime execution of what VariantTarget describes:

```gdscript
func convert_value(value, expected_type: int):
    match expected_type:
        TYPE_VECTOR2:
            return Vector2(value.x, value.y)
        TYPE_VECTOR3:
            return Vector3(value.x, value.y, value.z)
        TYPE_COLOR:
            if value is String:
                return Color.html(value)
            return Color(value.r, value.g, value.b, value.get("a", 1.0))
        TYPE_NODE_PATH:
            return NodePath(value)
        TYPE_OBJECT:
            if value is String and value.begins_with("res://"):
                return load(value)
        _:
            return value  # primitives pass through
```

When setting properties on a node, the operation queries `node.get_property_list()` to determine the expected type and applies conversion automatically.

### Error Handling

Every operation returns structured JSON, even on failure:

```json
{
  "success": false,
  "error": "Node not found: Player/Sprite2D",
  "operation": "node_set_properties",
  "context": {
    "scene_path": "scenes/player.tscn",
    "node_path": "Player/Sprite2D"
  }
}
```

Godot's own error output on stderr is captured by Director and included in failure responses but never mixed with the structured JSON on stdout.

## MCP Server Layer (Rust)

The Rust MCP server is intentionally thin. Its responsibilities:

1. **Tool registration** — expose typed schemas via `#[tool_router]`
2. **Input validation** — check required params, validate paths exist
3. **Backend selection** — probe `:6551` (editor plugin), then `:6550` (daemon), then fall back to one-shot; apply creation/modification/read routing rules
4. **Operation dispatch** — forward to whichever backend is active
5. **Output normalisation** — parse JSON from whichever backend, surface errors as `McpError`
6. **Godot path resolution** — `$GODOT_PATH` env var > `which godot`
7. **Daemon lifecycle** — spawn and manage the headless daemon process when needed

Each tool handler should be ~10-15 lines. Backend selection and dispatch live in `backend.rs`, not in individual handlers. If a handler gets longer, logic moves to the GDScript side.

### Project Structure

```
crates/director/
├── src/
│   ├── main.rs           — binary entry: `serve` or CLI dispatch
│   ├── server.rs         — DirectorServer struct, rmcp setup
│   ├── resolve.rs        — Godot binary + project root resolution
│   ├── backend.rs        — backend selection: editor plugin > daemon > one-shot
│   ├── editor.rs         — editor plugin client (TCP :6551)
│   ├── daemon.rs         — headless daemon lifecycle (TCP :6550)
│   ├── oneshot.rs        — headless one-shot subprocess
│   └── mcp/
│       ├── mod.rs        — #[tool_router] impl, shared MCP helpers
│       ├── scene.rs      — scene_read, scene_create, scene_list
│       ├── node.rs       — node_add, node_remove, node_reparent, node_set_properties
│       ├── resource.rs   — resource_read, material_create, shape_create, resource_duplicate
│       ├── tilemap.rs    — tilemap_set_cells, tilemap_get_cells, tilemap_clear
│       ├── animation.rs  — animation_create, animation_add_track, animation_read
│       ├── physics.rs    — physics_set_layers, physics_set_layer_names
│       └── project.rs    — uid_get, uid_update_project, export_mesh_library
addons/director/
├── plugin.cfg            — EditorPlugin manifest
├── plugin.gd             — EditorPlugin: TCP listener on :6551, routes to editor_ops.gd
├── editor_ops.gd         — editor backend: EditorInterface API calls
├── operations.gd         — headless one-shot entry: parse args, call ops, print JSON, quit
├── daemon.gd             — headless daemon entry: TCP server on :6550, dispatch to ops
└── ops/                  — operation functions shared by all three entries
    ├── scene_ops.gd
    ├── node_ops.gd
    ├── resource_ops.gd
    └── ...
tests/director-tests/
├── Cargo.toml
└── src/
    ├── harness.rs        — DirectorFixture, assert_approx (shared with wire-tests)
    ├── lib.rs
    ├── test_scene.rs
    ├── test_node.rs
    ├── test_resource.rs
    └── test_journey.rs   — multi-step E2E journeys
```

## Tool Surface

### Domain 1: Scene Introspection

#### `scene_read`

Read the full node tree of a scene file with types, properties, and hierarchy.

```
Params:
  project_path: string
  scene_path: string      — path relative to project (e.g., "scenes/player.tscn")
  depth: number?          — max tree depth (default: unlimited)
  properties: boolean?    — include node properties (default: true)

Returns:
  { root: { name, type, properties, children: Node[] } }
```

#### `scene_list`

List all scenes in the project with their root node types.

```
Params:
  project_path: string
  directory: string?
  pattern: string?        — glob filter (e.g. "scenes/**/*.tscn"); deferred, not in Phase 2

Returns:
  [{ path, root_type, node_count }]
```

#### `resource_read`

Read a `.tres` resource file as structured data.

```
Params:
  project_path: string
  resource_path: string

Returns:
  { type, properties: Record<string, any> }
```

### Domain 2: Scene Manipulation

#### `scene_create`

```
Params: project_path, scene_path, root_type
Returns: { path, root_type }
```

#### `node_add`

```
Params: project_path, scene_path, parent_path?, node_type, node_name, properties?
Returns: { node_path, type }
```

#### `node_remove`

```
Params: project_path, scene_path, node_path
Returns: { removed, children_removed }
```

#### `node_reparent`

```
Params: project_path, scene_path, node_path, new_parent_path
Returns: { old_path, new_path }
```

#### `node_set_properties`

```
Params: project_path, scene_path, node_path, properties: Record<string, any>
Returns: { node_path, properties_set: string[] }
```

Type conversion is handled via `get_property_list()` on the GDScript side, backed by the `VariantTarget` mapping on the Rust side.

#### `scene_add_instance`

```
Params: project_path, scene_path, instance_scene, parent_path?, node_name?
Returns: { node_path, instance_scene }
```

### Domain 3: Resource Creation

#### `material_create`

```
Params: project_path, resource_path, material_type, properties?
Returns: { path, type }
```

#### `shape_create`

```
Params: project_path, shape_type, shape_params, attach_to?, save_path?
Returns: { shape_type, attached_to?, saved_to? }
```

#### `style_box_create`

```
Params: project_path, resource_path, style_type, properties?
Returns: { path, type }
```

#### `resource_duplicate`

```
Params: project_path, source_path, dest_path, property_overrides?
Returns: { path, type, overrides_applied: string[] }
```

### Domain 4: TileMap & GridMap

#### `tilemap_set_cells`

```
Params: project_path, scene_path, node_path, cells: [{coords, source_id, atlas_coords, alternative_tile?}]
Returns: { cells_set }
```

#### `tilemap_clear`

```
Params: project_path, scene_path, node_path, region?
Returns: { cells_cleared }
```

#### `tilemap_get_cells`

```
Params: project_path, scene_path, node_path, region?
Returns: { cells: [{coords, source_id, atlas_coords}] }
```

#### `gridmap_set_cells`

```
Params: project_path, scene_path, node_path, cells: [{position, item, orientation?}]
Returns: { cells_set }
```

### Domain 5: Animation

#### `animation_create`

```
Params: project_path, resource_path, length, loop_mode?, step?
Returns: { path, length }
```

#### `animation_add_track`

```
Params: project_path, resource_path, track_type, node_path, keyframes: [{time, value, transition?}]
Returns: { track_index, keyframe_count }
```

#### `animation_remove_track`

```
Params: project_path, resource_path, track_index? | node_path?
Returns: { tracks_removed }
```

#### `animation_read`

```
Params: project_path, resource_path
Returns: { length, loop_mode, tracks: [{type, node_path, keyframes}] }
```

### Domain 6: Shaders & Materials

#### `shader_material_set_params`

```
Params: project_path, scene_path, node_path, material_property?, params
Returns: { params_set: string[] }
```

#### `visual_shader_create`

```
Params: project_path, resource_path, shader_mode, nodes: [{id, type, position, properties?}], connections
Returns: { path, node_count, connection_count }
```

### Domain 7: Physics Configuration

#### `physics_set_layers`

```
Params: project_path, scene_path, node_path, collision_layer?, collision_mask?
Returns: { node_path, layer, mask }
```

#### `physics_set_layer_names`

```
Params: project_path, layer_type, layers: Record<number, string>
Returns: { layers_set }
```

### Domain 8: Meta-Operations

#### `batch`

Run multiple operations in a single invocation. One headless process (or one editor plugin round-trip) executes all operations in sequence. Reduces MCP round-trips for multi-step work like "create scene, add 10 nodes, set properties on all."

```
Params:
  project_path: string
  operations: [{
    operation: string,      — any Director operation name
    params: Record<string, any>
  }]
  stop_on_error: boolean?   — abort remaining ops on first failure (default: true)

Returns:
  {
    results: [{
      operation: string,
      success: boolean,
      data?: any,
      error?: string
    }],
    completed: number,
    failed: number
  }
```

#### `scene_diff`

Compare two scenes structurally and return a human-readable diff of node additions, removals, reparents, and property changes. Useful for the agent to understand what changed between operations or between git commits.

```
Params:
  project_path: string
  scene_a: string           — path to first scene (or git ref: "HEAD:scenes/player.tscn")
  scene_b: string           — path to second scene

Returns:
  {
    added: [{ node_path, type }],
    removed: [{ node_path, type }],
    moved: [{ node_path, old_parent, new_parent }],
    changed: [{ node_path, property, old_value, new_value }]
  }
```

### Domain 9: Project Utilities

#### `uid_get`

```
Params: project_path, file_path
Returns: { file_path, uid }
```

#### `uid_update_project`

```
Params: project_path
Returns: { scenes_processed, scripts_processed, uids_generated }
```

#### `export_mesh_library`

```
Params: project_path, scene_path, output_path, items?: string[]
Returns: { path, items_exported }
```

## Implementation Order

Build these in the order you need them.

**Phase 1 — MVP:**
- `scene_read`
- `scene_create`
- `node_add` (with type conversion)
- `node_set_properties` (with type conversion)
- `node_remove`

**Phase 2 — Scene composition:**
- `scene_add_instance`
- `node_reparent`
- `scene_list`
- `resource_read`

**Phase 3 — Resources & materials:**
- `material_create`
- `shape_create`
- `resource_duplicate`
- `shader_material_set_params`

**Phase 4 — Level design:**
- `tilemap_set_cells`
- `tilemap_get_cells`
- `tilemap_clear`
- `gridmap_set_cells`

**Phase 5 — Animation:**
- `animation_create`
- `animation_add_track`
- `animation_read`
- `animation_remove_track`

**Phase 6 — Advanced:**
- `visual_shader_create`
- `physics_set_layers`
- `physics_set_layer_names`
- `style_box_create`

**Phase 7 — Utilities:**
- `uid_get`
- `uid_update_project`
- `export_mesh_library`

**Phase 8 — Meta:**
- `batch`
- `scene_diff`

## Relationship to Spectator

Director and Spectator are complementary tools in the same repository. They share no runtime state and no TCP protocol, but they share conventions, patterns, and the Theatre workspace.

| | Director | Spectator |
|---|---|---|
| Godot process | EditorPlugin (live) or headless (one-shot/daemon) | Persistent GDExtension in running game |
| Communication | TCP :6551 (editor) or stdout/TCP :6550 (headless) | TCP length-prefixed JSON to GDExtension |
| State | Stateless (headless) / editor-stateful (plugin) | Stateful (game is running) |
| Operations | Read/write scenes and resources | Observe spatial state, emit signals, teleport |
| When used | Building/modifying the game | Testing/verifying the game |
| GDScript layer | `addons/director/ops/`, `plugin.gd`, `operations.gd` | `addons/spectator/runtime.gd` |
| Test harness | `DirectorFixture` (subprocess/stdout) | `GodotFixture` (TCP handshake) |

An agent can have both enabled and use them in sequence: build with Director, test with Spectator, iterate.

## Agent Guardrails

AI coding agents will attempt to edit `.tscn`, `.tres`, and `.res` files directly unless explicitly told not to. These files are technically text, look structured, and the agent will convince itself it understands the format. It doesn't.

### Tool Descriptions

Every Director tool description includes anti-direct-edit guidance:

```
name: "node_add"
description: "Add a node to a Godot scene file (.tscn). Always use this tool
instead of editing .tscn files directly — the scene serialization format is
fragile and hand-editing will produce corrupt scenes."
```

### CLAUDE.md for Godot Projects

Place this in the project's `CLAUDE.md`:

```markdown
# Godot Project Rules

## File Editing Boundaries

You have two MCP servers available:
- **director**: Scene and resource manipulation (scenes, materials, tilemaps, animations)
- **spectator**: Runtime observation (spatial queries, signals, game state)

### Edit directly:
- `.gd` (GDScript), `.gdshader`, `.cfg`, `.import`, `.md`, `.json`, `project.godot`

### Never edit directly:
- `.tscn` (scene files) — search for the appropriate `director` tool instead
- `.tres` (text resources) — search for the appropriate `director` tool instead
- `.res` (binary resources) — search for the appropriate `director` tool instead

When you need to create or modify a `.tscn` or `.tres` file, use or search for
the appropriate `director` tool (e.g. search "director scene" or "director node").
Do not attempt to construct or edit these files as text — the serialization format
has strict internal consistency requirements that are not visible in the text, and
hand-editing produces scenes that appear valid but corrupt silently.

### Workflow
1. Use or search for a `director` read tool to understand current scene structure
2. Use or search for the appropriate `director` tool to make changes
3. Edit `.gd` scripts directly as normal
4. Use or search for a `spectator` tool to observe and test the running game
```

### Defense in Depth

1. **Tool descriptions** — nudge at decision time
2. **CLAUDE.md rules** — explicit policy loaded at session start
3. **Good tool coverage** — if the tool exists and works, the agent won't go rogue

The most important layer is #3. Agents attempt direct file edits primarily when the right tool doesn't exist or fails.

## Open Questions

- **Undo/history:** Operations through the editor plugin enter the undo history automatically. Headless operations do not. Is git sufficient for headless undo, or should Director offer explicit rollback support?
