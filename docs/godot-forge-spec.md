# Godot Forge — MCP Server Design Spec

## Overview

Godot Forge is an MCP server that exposes Godot Engine's internal APIs for operations that are too fragile or complex to perform via raw filesystem editing. It is one half of a two-server Godot tooling setup:

- **Godot Forge** (this spec): Scene/resource manipulation at editor-time via headless Godot
- **Godot Runtime** (separate server): Game execution, input simulation, screenshots, live debugging

The agent handles scripts (`.gd`), project config (`project.godot`), and simple text files directly through filesystem access. Forge handles everything that needs Godot's own API to serialize correctly.

## Design Principles

1. **Only expose what the filesystem can't do well.** If `cat` and `sed` can handle it reliably, it doesn't belong here.
2. **Godot is the API.** Every mutation goes through Godot's own classes (PackedScene, ResourceSaver, ClassDB). We never construct `.tscn`/`.tres` text by hand.
3. **Operations are independently testable.** The GDScript operations layer is callable directly from terminal: `godot --headless --script godot_operations.gd <op> '<json>'`. The MCP server is a thin typed wrapper.
4. **Lean tool count, grow organically.** Start with what you need, add domains as projects demand them. No speculative tooling.
5. **No editor required.** Everything runs headless. The Godot editor is never launched or controlled by this server.

## Architecture

```
MCP Client (Claude Code, Cursor, etc.)
    │
    │ stdio (MCP protocol)
    │
MCP Server (TypeScript / Bun)
    │
    │ child_process.execFile()
    │
godot --headless --path <project> --script godot_operations.gd <operation> <json_params>
    │
    │ Godot API (ClassDB, ResourceLoader, ResourceSaver, PackedScene, etc.)
    │
Project filesystem (.tscn, .tres, .res files)
```

### Key decisions

- **Bun + TypeScript** for the MCP server. Matches Agent Lens stack. MCP SDK is TypeScript-first.
- **Single GDScript file** (`godot_operations.gd`) handles all Godot-side operations. Extends `SceneTree`, runs in `_init()`, dispatches via `match` on the operation name.
- **One headless process per operation (one-shot mode).** Default mode. Spawns `godot --headless` for each operation, exits when done. Stateless and reliable — no connection management, no stale state. Cold-start cost of ~1-3s per operation depending on project size.
- **Persistent headless process (daemon mode).** Optional mode for interactive development. A single Godot instance stays alive with a TCP command server, eliminating cold-start overhead after the first invocation. Subsequent operations complete in ~50ms. See Execution Modes below.
- **JSON in, JSON out.** Params go in as a JSON string argument (one-shot) or JSON over TCP (daemon). Results come back as structured JSON on stdout or TCP response. Errors on stderr.
- **Project path comes from the agent.** The agent knows where it's working. Every tool takes `project_path` as a required param. No global state.
- **CLI wrapper for direct use.** The GDScript operations layer is independently callable without the MCP server, via a thin CLI that handles Godot path resolution and project detection.

### Execution Modes

#### One-shot mode (default)

Every tool call spawns a fresh Godot process:

```
MCP/CLI → godot --headless --path <project> --script godot_operations.gd <op> <json> → exits
```

Pros: Stateless, no connection management, always clean environment, works in CI.
Cons: ~1-3s cold-start per operation. A sequence of 10 operations takes 10-30s.

#### Daemon mode

A persistent headless Godot process runs a TCP command server. The MCP server and CLI connect to it instead of spawning new processes.

```
                                    ┌─────────────────────────────────────┐
MCP/CLI → TCP (localhost:6550) →    │ godot --headless --path <project>   │
         JSON command/response      │   autoload: godot_forge_daemon.gd   │
                                    │   TCP server on :6550               │
                                    │   dispatches to same op functions    │
                                    └─────────────────────────────────────┘
```

The daemon GDScript (`godot_forge_daemon.gd`) is a simple TCP server loop:

```gdscript
extends SceneTree

var server: TCPServer
var operations  # reference to shared operation functions

func _init():
    server = TCPServer.new()
    server.listen(6550, "127.0.0.1")
    print(JSON.stringify({"status": "ready", "port": 6550}))

func _process(_delta):
    if server.is_connection_available():
        var connection = server.take_connection()
        handle_connection(connection)

func handle_connection(stream: StreamPeerTCP):
    var request = JSON.parse_string(stream.get_utf8_string())
    var result = dispatch_operation(request.operation, request.params)
    stream.put_utf8_string(JSON.stringify(result))
```

The MCP server / CLI handles the lifecycle:

1. **On first tool call:** Check if daemon is running (try TCP connect to :6550). If not, spawn it and wait for the `{"status": "ready"}` message on stdout.
2. **Send operation:** JSON over TCP, read JSON response.
3. **On failure:** If TCP connection fails mid-session (Godot crashed), respawn the daemon and retry once.
4. **On shutdown:** `SIGTERM` the daemon process, or send a `{"operation": "quit"}` command.
5. **Fallback:** If daemon mode fails to start, fall back to one-shot mode transparently.

The shared operation functions live in a separate GDScript file that both `godot_operations.gd` (one-shot) and `godot_forge_daemon.gd` (daemon) import. Same logic, different execution harness.

#### Mode selection

```bash
# One-shot (default, always works)
godot-forge scene_read '{"scene_path":"scenes/player.tscn"}'

# Start daemon explicitly
godot-forge daemon start
godot-forge scene_read '{"scene_path":"scenes/player.tscn"}'  # uses daemon automatically
godot-forge daemon stop

# Auto-daemon: start on first call, keep alive
godot-forge --daemon scene_read '{"scene_path":"scenes/player.tscn"}'
```

MCP server config:

```json
{
  "mcpServers": {
    "godot-forge": {
      "command": "godot-forge",
      "args": ["serve"],
      "env": {
        "GODOT_FORGE_MODE": "daemon"
      }
    }
  }
}
```

#### Implementation order

Build one-shot first. It's simpler, testable, and correct. Add daemon mode once one-shot is working and the cold-start cost is actually bothering you in practice. The operation functions are shared, so adding daemon mode later doesn't require rewriting anything — it's just a new execution harness wrapping the same logic.

### CLI Wrapper

The raw Godot invocation is too verbose for manual use and debugging:

```bash
# Nobody wants to type this
godot --headless --path /home/nathan/my-game --script /path/to/godot_operations.gd scene_read '{"scene_path":"scenes/player.tscn"}'
```

A thin CLI (`godot-forge`) wraps this:

```bash
# Auto-detects project root (walks up looking for project.godot, like git finds .git)
# Resolves godot binary from $GODOT_PATH or $PATH
godot-forge scene_read '{"scene_path":"scenes/player.tscn"}'

# Explicit project path
godot-forge --project /home/nathan/my-game node_add '{"scene_path":"scenes/player.tscn","node_type":"Sprite2D","node_name":"Icon"}'

# Pipe through jq for readable output
godot-forge scene_read '{"scene_path":"scenes/player.tscn"}' | jq '.root.children[].name'
```

The CLI is a single Bun script (~30 lines) that:

1. Resolves project root: `--project` flag > walk up from `cwd` looking for `project.godot`
2. Resolves Godot binary: `$GODOT_PATH` > `which godot`
3. Resolves the `godot_operations.gd` script path relative to its own install location
4. Forwards the operation name and JSON params as args to `godot --headless --path <project> --script <ops.gd> <op> <json>`
5. Passes through stdout/stderr, exits with Godot's exit code

The MCP server and the CLI share the same GDScript operations file and the same project/Godot resolution logic (extracted into a shared module). The MCP server is effectively the CLI with an MCP transport bolted on top.

## Tool Surface

### Domain 1: Scene Introspection

Tools for reading and understanding scene structure. These are the "give me context" tools the agent calls before making changes.

#### `scene_read`

Read the full node tree of a scene file with types, properties, and hierarchy.

```
Params:
  project_path: string    — absolute path to project root
  scene_path: string      — path relative to project (e.g., "scenes/player.tscn")
  depth: number?          — max tree depth to return (default: unlimited)
  properties: boolean?    — include node properties (default: true)

Returns:
  {
    root: {
      name: string,
      type: string,
      properties: Record<string, any>,
      children: Node[]  // recursive
    }
  }
```

Why MCP: `.tscn` text format uses section-based serialization with `ext_resource`/`sub_resource` ID indirection. Parsing it correctly requires resolving resource references, inherited scenes, and instance overrides. Godot's own scene loader handles all of this.

#### `scene_list`

List all scenes in the project with their root node types.

```
Params:
  project_path: string
  directory: string?      — subdirectory to search (default: entire project)

Returns:
  [{ path: string, root_type: string, node_count: number }]
```

Why MCP: While `find . -name "*.tscn"` gets paths, extracting root node type requires parsing the `.tscn` header correctly.

#### `resource_read`

Read a `.tres` resource file and return its properties as structured data.

```
Params:
  project_path: string
  resource_path: string   — path relative to project

Returns:
  {
    type: string,
    properties: Record<string, any>
  }
```

Why MCP: `.tres` files use Godot's custom serialization with nested sub-resources, external references, and type-specific encoding. The text format is technically parseable but fragile across Godot versions.

### Domain 2: Scene Manipulation

Tools for creating and modifying scene trees. This is the core "things too fragile to edit as text" set.

#### `scene_create`

Create a new scene file with a specified root node type.

```
Params:
  project_path: string
  scene_path: string      — where to save (relative to project)
  root_type: string       — Godot class name (e.g., "Node2D", "CharacterBody3D", "Control")

Returns:
  { path: string, root_type: string }
```

#### `node_add`

Add a node to an existing scene.

```
Params:
  project_path: string
  scene_path: string
  parent_path: string?    — NodePath within scene (default: root node)
  node_type: string       — Godot class name
  node_name: string
  properties: Record<string, any>?  — properties to set (with type conversion)

Returns:
  { node_path: string, type: string }
```

#### `node_remove`

Remove a node (and its children) from a scene.

```
Params:
  project_path: string
  scene_path: string
  node_path: string       — NodePath to the node to remove

Returns:
  { removed: string, children_removed: number }
```

#### `node_reparent`

Move a node to a different parent within the same scene.

```
Params:
  project_path: string
  scene_path: string
  node_path: string       — current NodePath
  new_parent_path: string — NodePath of the new parent

Returns:
  { old_path: string, new_path: string }
```

#### `node_set_properties`

Set properties on an existing node with proper Godot type conversion.

```
Params:
  project_path: string
  scene_path: string
  node_path: string
  properties: Record<string, any>

Returns:
  { node_path: string, properties_set: string[] }
```

**Type conversion is critical here.** This is the main thing Coding-Solo's implementation gets wrong. The GDScript side must convert JSON representations to proper Godot types:

| JSON Input | Godot Type |
|---|---|
| `{"x": 10, "y": 20}` | `Vector2(10, 20)` |
| `{"x": 1, "y": 2, "z": 3}` | `Vector3(1, 2, 3)` |
| `{"r": 1.0, "g": 0.5, "b": 0.0, "a": 1.0}` | `Color(1, 0.5, 0, 1)` |
| `"#FF8800"` | `Color.html("FF8800")` |
| `"res://path/to/resource.tres"` | `load("res://path/to/resource.tres")` |
| `"NodePath/To/Target"` (when property type is NodePath) | `NodePath("NodePath/To/Target")` |
| `[1.0, 2.0, 3.0, ...]` (when property type is PackedFloat32Array) | `PackedFloat32Array([...])` |

The GDScript side should use `node.get_property_list()` to determine the expected type for each property and convert accordingly. Fall back to raw assignment only for primitive types (int, float, string, bool).

#### `scene_add_instance`

Add a scene as an instanced child of another scene. This creates the `ext_resource` reference and instance node.

```
Params:
  project_path: string
  scene_path: string        — the scene to modify
  instance_scene: string    — path to the scene to instance (e.g., "scenes/enemy.tscn")
  parent_path: string?      — where to add it (default: root)
  node_name: string?        — override the instance name

Returns:
  { node_path: string, instance_scene: string }
```

Why MCP: Scene instancing requires correct `ext_resource` ID management and the `instance` property on the node. Getting this wrong breaks the scene entirely.

### Domain 3: Resource Creation

Tools for creating `.tres` resource files that have complex internal structure.

#### `material_create`

Create a material resource.

```
Params:
  project_path: string
  resource_path: string     — where to save (e.g., "materials/metal.tres")
  material_type: string     — "standard_3d" | "shader" | "canvas_item"
  properties: Record<string, any>?

Returns:
  { path: string, type: string }
```

For `standard_3d`: accepts `albedo_color`, `metallic`, `roughness`, `emission`, `normal_map` (as resource path), etc.
For `shader`: accepts `shader_path` (path to `.gdshader` file) and `shader_params` dict.

#### `shape_create`

Create a collision shape resource and optionally attach it to a node.

```
Params:
  project_path: string
  shape_type: string        — "rectangle_2d" | "circle_2d" | "capsule_2d" | "box_3d" | "sphere_3d" | "capsule_3d" | "convex_polygon_3d" | "concave_polygon_3d"
  shape_params: Record<string, any>  — type-specific: size, radius, height, points, etc.
  attach_to: {              — optional: attach to a CollisionShape node in a scene
    scene_path: string,
    node_path: string
  }?
  save_path: string?        — optional: save as standalone .tres

Returns:
  { shape_type: string, attached_to?: string, saved_to?: string }
```

#### `style_box_create`

Create StyleBox resources for UI theming.

```
Params:
  project_path: string
  resource_path: string
  style_type: string        — "flat" | "texture" | "line" | "empty"
  properties: Record<string, any>?

Returns:
  { path: string, type: string }
```

#### `resource_duplicate`

Load a resource, modify properties, save as a new file. Useful for creating variants.

```
Params:
  project_path: string
  source_path: string
  dest_path: string
  property_overrides: Record<string, any>?

Returns:
  { path: string, type: string, overrides_applied: string[] }
```

### Domain 4: TileMap & GridMap

Tools for level layout. These operate on data formats that are completely opaque in `.tscn` text.

#### `tilemap_set_cells`

Set cells in a TileMapLayer (Godot 4.3+ uses TileMapLayer nodes, not the old TileMap).

```
Params:
  project_path: string
  scene_path: string
  node_path: string         — path to the TileMapLayer node
  cells: [{
    coords: { x: number, y: number },
    source_id: number,
    atlas_coords: { x: number, y: number },
    alternative_tile: number?
  }]

Returns:
  { cells_set: number }
```

Why MCP: TileMap cell data is stored as packed integer arrays in `.tscn`. Editing by hand is not realistic.

#### `tilemap_clear`

Clear all cells or a region of a TileMapLayer.

```
Params:
  project_path: string
  scene_path: string
  node_path: string
  region: {                 — optional: clear only this region
    from: { x: number, y: number },
    to: { x: number, y: number }
  }?

Returns:
  { cells_cleared: number }
```

#### `tilemap_get_cells`

Read the current cell data from a TileMapLayer.

```
Params:
  project_path: string
  scene_path: string
  node_path: string
  region: {
    from: { x: number, y: number },
    to: { x: number, y: number }
  }?

Returns:
  { cells: [{ coords: {x, y}, source_id: number, atlas_coords: {x, y} }] }
```

#### `gridmap_set_cells`

Set cells in a GridMap (3D equivalent of TileMap).

```
Params:
  project_path: string
  scene_path: string
  node_path: string
  cells: [{
    position: { x: number, y: number, z: number },
    item: number,           — MeshLibrary item index
    orientation: number?    — rotation index (0-23)
  }]

Returns:
  { cells_set: number }
```

### Domain 5: Animation

Tools for creating animation resources and tracks. Animation data in `.tres` is positional and dense — authoring by hand is extremely error-prone.

#### `animation_create`

Create a new Animation resource.

```
Params:
  project_path: string
  resource_path: string     — where to save
  length: number            — duration in seconds
  loop_mode: string?        — "none" | "linear" | "ping_pong" (default: "none")
  step: number?             — snap step for keyframes (default: 0.05)

Returns:
  { path: string, length: number }
```

#### `animation_add_track`

Add a track to an existing Animation resource.

```
Params:
  project_path: string
  resource_path: string
  track_type: string        — "value" | "position_3d" | "rotation_3d" | "scale_3d" | "method" | "bezier" | "audio" | "animation"
  node_path: string         — NodePath the track targets (e.g., "Player:position")
  keyframes: [{
    time: number,
    value: any,             — type depends on track type
    transition: number?     — easing curve (default: 1.0 = linear)
  }]

Returns:
  { track_index: number, keyframe_count: number }
```

#### `animation_remove_track`

Remove a track from an Animation by index or by node path.

```
Params:
  project_path: string
  resource_path: string
  track_index: number?
  node_path: string?        — remove all tracks targeting this path

Returns:
  { tracks_removed: number }
```

#### `animation_read`

Read all tracks and keyframes from an Animation resource.

```
Params:
  project_path: string
  resource_path: string

Returns:
  {
    length: number,
    loop_mode: string,
    tracks: [{
      type: string,
      node_path: string,
      keyframes: [{ time: number, value: any, transition: number }]
    }]
  }
```

### Domain 6: Shaders & Materials

Tools for programmatic shader and material management.

#### `shader_material_set_params`

Set shader parameters on a ShaderMaterial attached to a node.

```
Params:
  project_path: string
  scene_path: string
  node_path: string         — node with the material
  material_property: string? — which material slot (default: "material" for 2D, "material_override" for 3D)
  params: Record<string, any>  — shader uniform values (with type conversion)

Returns:
  { params_set: string[] }
```

#### `visual_shader_create`

Create a VisualShader resource with nodes and connections.

```
Params:
  project_path: string
  resource_path: string
  shader_mode: string       — "spatial" | "canvas_item" | "particles" | "sky" | "fog"
  nodes: [{
    id: number,
    type: string,           — VisualShader node class name
    position: { x: number, y: number },
    properties: Record<string, any>?
  }]
  connections: [{
    from_node: number,
    from_port: number,
    to_node: number,
    to_port: number
  }]

Returns:
  { path: string, node_count: number, connection_count: number }
```

Why MCP: VisualShader resources store node graphs as serialized sub-resources with port indices. Constructing this manually in `.tres` format would be nightmarish.

### Domain 7: Physics Configuration

Tools for collision layer/mask setup and physics body configuration.

#### `physics_set_layers`

Set collision layer and mask on a physics body or area node.

```
Params:
  project_path: string
  scene_path: string
  node_path: string
  collision_layer: number?  — bitmask
  collision_mask: number?   — bitmask

Returns:
  { node_path: string, layer: number, mask: number }
```

#### `physics_set_layer_names`

Configure named physics layers in project settings.

```
Params:
  project_path: string
  layer_type: string        — "2d_physics" | "3d_physics" | "2d_render" | "3d_render"
  layers: Record<number, string>  — layer number (1-32) to name

Returns:
  { layers_set: number }
```

Note: This one might be doable via filesystem (editing `project.godot`), but doing it through Godot's API ensures the format is correct and the settings are validated.

### Domain 8: Project Utilities

Miscellaneous tools that need Godot's API.

#### `uid_get`

Get the UID for a resource file (Godot 4.4+).

```
Params:
  project_path: string
  file_path: string

Returns:
  { file_path: string, uid: string }
```

#### `uid_update_project`

Resave all resources to generate/update UIDs (Godot 4.4+).

```
Params:
  project_path: string

Returns:
  { scenes_processed: number, scripts_processed: number, uids_generated: number }
```

#### `export_mesh_library`

Export a scene's meshes as a MeshLibrary resource for use with GridMap.

```
Params:
  project_path: string
  scene_path: string
  output_path: string
  items: string[]?          — specific mesh names to include

Returns:
  { path: string, items_exported: number }
```

## GDScript Operations Layer

The `godot_operations.gd` file is the only file that touches Godot's API. It follows a strict pattern:

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

Central to the GDScript layer is a type conversion system that maps JSON values to Godot types:

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

When setting properties on a node, the operation queries `node.get_property_list()` to determine the expected type and applies conversion automatically. This is the key thing that differentiates this from Coding-Solo's raw `node.set()` approach.

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

Godot's own error output on stderr is captured by the MCP server and included in failure responses but never mixed with the structured JSON on stdout.

## MCP Server Layer

The TypeScript MCP server is intentionally thin. Its responsibilities:

1. **Tool registration** — expose typed schemas for each operation
2. **Input validation** — check required params, validate paths exist
3. **Operation dispatch** — call `execFile('godot', ['--headless', '--path', projectPath, '--script', operationsScript, operation, JSON.stringify(params)])`
4. **Output parsing** — parse JSON from stdout, surface errors from stderr
5. **Godot path management** — resolve the Godot executable via `$GODOT_PATH` env var or `which godot`

Each tool handler should be ~10-15 lines. If a handler gets longer than that, logic should move to the GDScript side.

### Project Structure

```
godot-forge/
├── src/
│   ├── index.ts              — MCP server entry, tool registration, dispatch
│   ├── cli.ts                — CLI entry point (godot-forge command)
│   ├── resolve.ts            — shared project root + Godot binary resolution
│   ├── execute.ts            — shared operation execution (one-shot + daemon connect)
│   ├── types.ts              — shared TypeScript types for params/results
│   └── scripts/
│       ├── operations.gd     — shared operation functions (the actual Godot API work)
│       ├── oneshot.gd        — one-shot entry: parse args, call operations, print JSON, quit
│       └── daemon.gd         — daemon entry: TCP server loop, dispatch to operations
├── package.json              — bin: { "godot-forge": "./src/cli.ts" }
├── tsconfig.json
└── README.md
```

No build step beyond `bun build`. No runtime dependencies beyond `@modelcontextprotocol/sdk`. The CLI entry point can run directly via `bun src/cli.ts` or be linked globally via `bun link`.

## Implementation Order

Build these in the order you need them. The spec is the full vision; implementation is incremental.

**Phase 1 — MVP (first project session):**
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

**Phase 7 — Utilities (as needed):**
- `uid_get`
- `uid_update_project`
- `export_mesh_library`

## Relationship to Godot Runtime Server

The runtime server is a completely separate MCP server with a different lifecycle:

| | Godot Forge | Godot Runtime |
|---|---|---|
| Godot process | Spawns headless, exits after each op | Manages a running game process |
| Communication | stdin/stdout via script args | UDP/TCP bridge to injected autoload |
| State | Stateless | Stateful (game is running) |
| Operations | Read/write scenes and resources | Screenshots, input sim, live eval, UI discovery |
| When used | Building/modifying the game | Testing/verifying the game |

They share no code and no state. An agent can have both enabled and use them in sequence: build with Forge, test with Runtime, iterate.

## Agent Guardrails

AI coding agents will attempt to edit `.tscn`, `.tres`, and `.res` files directly unless explicitly told not to. These files are technically text, look structured, and the agent will convince itself it understands the format. It doesn't — Godot's serialization has implicit ordering, resource ID management, type-specific encoding, and version-dependent formatting that breaks silently when hand-edited.

### Tool Descriptions

Every Forge tool description should include anti-direct-edit guidance. The tool description is what the agent sees when deciding how to act. Example:

```
name: "node_add"
description: "Add a node to a Godot scene file (.tscn). Always use this tool
instead of editing .tscn files directly — the scene serialization format is
fragile and hand-editing will produce corrupt scenes."
```

This nudges the agent toward the tool path at decision time, before it even considers filesystem edits.

### CLAUDE.md / Project Rules

Place this in the project's `CLAUDE.md` (for Claude Code) or equivalent rules file for other agents:

```markdown
# Godot Project Rules

## File Editing Boundaries

You have two MCP servers available for Godot work:
- **godot-forge**: Scene and resource manipulation (scenes, materials, tilemaps, animations, etc.)
- **godot-runtime**: Game execution, testing, screenshots, input simulation

### Files you SHOULD edit directly:
- `.gd` (GDScript source files)
- `.gdshader` (shader code)
- `.cfg`, `.import` (config files)
- `.md`, `.txt`, `.json` (documentation and data)
- `project.godot` (project settings — but prefer godot-forge for physics layer names)

### Files you MUST NOT edit directly:
- `.tscn` (scene files) — use godot-forge tools
- `.tres` (text resources) — use godot-forge tools
- `.res` (binary resources) — use godot-forge tools

These files use Godot's internal serialization format. They contain:
- Resource IDs that must be globally consistent within the file
- Type-specific property encoding (Vector2, Color, NodePath, etc.)
- Scene instance references with ext_resource/sub_resource indirection
- Packed data formats (TileMap cells, animation tracks) that are not human-readable

Hand-editing these files produces scenes that appear valid but crash at runtime,
lose data silently, or cause the Godot editor to corrupt them further on next save.

### Reading .tscn/.tres for context
You MAY read these files to understand current state (e.g., `cat scenes/player.tscn`
to see what nodes exist). But all modifications must go through godot-forge tools.

### Workflow
1. Read the scene with `scene_read` to understand current structure
2. Make changes with Forge tools (`node_add`, `node_set_properties`, etc.)
3. Edit scripts (`.gd` files) directly as normal
4. Test with godot-runtime tools
```

### Godot Docs Context

If using a Godot documentation MCP or RAG system alongside Forge, include this in the documentation context:

```markdown
## AI Agent Note on Scene Files

Godot scene files (.tscn) and resource files (.tres) use a custom text serialization
format that is NOT safely editable by text tools. While they appear to be
human-readable, the format has strict internal consistency requirements:

- ext_resource and sub_resource IDs must be sequential and referenced correctly
- Property values must match Godot's type serialization exactly
- Node ordering and ownership metadata affects scene instantiation
- PackedScene packing rules are not documented in the text format

Always use Godot's API (via GDScript, C#, or MCP tools) to create and modify
scenes and resources. Direct text editing is the #1 cause of corrupt scene files
in AI-assisted Godot workflows.
```

### Defense in Depth

These layers work together:

1. **Tool descriptions** — nudge at decision time ("use this, not direct edit")
2. **CLAUDE.md rules** — explicit policy the agent loads at session start
3. **Docs context** — reinforces via domain knowledge ("the format is fragile")
4. **Good tool coverage** — if the tool exists and works, the agent won't need to go rogue

The most important layer is #4. Agents attempt direct file edits primarily when the right tool doesn't exist or fails. If Forge reliably handles the operation, the agent will prefer it naturally because it's less risky than guessing at serialization formats.

## Open Questions

- **Batch operations:** Should there be a `batch` meta-tool that runs multiple operations in a single invocation (one-shot) or single TCP round-trip (daemon)? Would reduce overhead for sequences like "create scene, add 10 nodes, set properties on all of them." Daemon mode already mitigates this but a batch tool could still reduce MCP round-trips.
- **Scene diffing:** Would a `scene_diff` tool (compare two scenes structurally) be valuable for the agent to understand what changed?
- **Undo/history:** Should operations support any form of undo, or is git sufficient?
