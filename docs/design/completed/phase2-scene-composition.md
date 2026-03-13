# Design: Director Phase 2 — Scene Composition

## Overview

Phase 2 adds four tools that let an agent compose scenes from existing scenes,
reorganise node trees, list project scenes, and inspect resources. All tools
follow the Phase 1 patterns: headless one-shot execution, structured JSON
responses, and thin Rust handlers delegating to GDScript operations.

**New tools:**
- `scene_add_instance` — add a PackedScene instance (reference) as child
- `node_reparent` — move a node to a new parent within the same scene
- `scene_list` — list `.tscn` files with root type and node count
- `resource_read` — load any resource and serialize its properties

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Instance type | Scene reference, not copy | Standard Godot workflow; source changes propagate |
| `scene_list` node_count | Always included | Simpler; optimize with opt-in flag later if slow |
| `scene_list` directory | Default to project root | Optional `directory` filter; glob support deferred |
| `resource_read` scope | Any `load()`-able path | No artificial restriction; hint for `.tscn` → `scene_read` |
| `resource_read` depth | One level; nested resources as paths | Bounded output; agent calls again for nested |
| `node_reparent` collisions | Error by default; optional `new_name` | Safe default with escape hatch |

## Implementation Units

### Unit 1: GDScript `scene_ops.gd` — `op_scene_list`

**File**: `addons/director/ops/scene_ops.gd`

```gdscript
static func op_scene_list(params: Dictionary) -> Dictionary:
    ## List all .tscn files in the project (or a subdirectory).
    ##
    ## Params: directory (String, optional — default "")
    ## Returns: { success, data: { scenes: [{ path, root_type, node_count }] } }
```

**Implementation Notes:**
- Use `DirAccess.open("res://" + directory)` to walk the filesystem
- Recursive directory traversal collecting all `.tscn` files
- For each scene: `load()` → `instantiate()` → read `get_class()` on root → count descendants → `free()`
- `directory` param defaults to `""` (project root)
- Validate directory exists; error if not found
- Sort results by path for deterministic output
- Use the existing `_count_descendants()` helper from `node_ops.gd` — but since it's a static class, either duplicate the helper or extract to a shared utils. Recommend: add a small `_count_nodes(root)` helper inline in `scene_ops.gd` that counts root + descendants (so node_count includes the root itself, matching what an agent expects)

**Acceptance Criteria:**
- [ ] Returns all `.tscn` files recursively from project root when no directory given
- [ ] Filters to subdirectory when `directory` is provided
- [ ] Each entry has `path` (relative to project, no `res://` prefix), `root_type`, `node_count`
- [ ] `node_count` includes root node (a scene with just a root returns `1`)
- [ ] Empty directory returns `{ scenes: [] }`, not an error
- [ ] Non-existent directory returns structured error

---

### Unit 2: GDScript `scene_ops.gd` — `op_scene_add_instance`

**File**: `addons/director/ops/scene_ops.gd`

```gdscript
static func op_scene_add_instance(params: Dictionary) -> Dictionary:
    ## Add a scene instance (reference) as a child in another scene.
    ##
    ## Params:
    ##   scene_path (String) — target scene to modify
    ##   instance_scene (String) — scene to instance (e.g. "scenes/player.tscn")
    ##   parent_path (String, default ".") — parent node path
    ##   node_name (String, optional) — override instance root name
    ## Returns: { success, data: { node_path, instance_scene } }
```

**Implementation Notes:**
- Load the target scene (`scene_path`), instantiate it
- Load the instance scene (`instance_scene`) as `PackedScene`
- Validate `instance_scene` exists and is a valid scene
- Call `instance_packed.instantiate()` to create the instance node
- If `node_name` provided, set `instance_node.name = node_name`; otherwise keep the instance scene's root name
- Find parent node (same pattern as `node_add`)
- Check for name collision: `parent.has_node(instance_node.name)` → error
- `parent.add_child(instance_node)`
- Set `instance_node.owner = root`
- The instance node retains its scene reference — when packed, Godot serializes it as an instance (ext_resource), not an inline copy. This is automatic because we used `PackedScene.instantiate()`.
- `_set_owner_recursive` on the instance's children with owner = root
- `_repack_and_save(root, full_path)`
- Return `{ node_path: <path_to_instance>, instance_scene: <instance_scene_path> }`

**Key subtlety:** For the instance to serialize as a scene reference (not inlined nodes), we must NOT call `_set_owner_recursive` on the instance's children — only set `instance_node.owner = root`. The children should retain `null` owner (meaning they come from the instanced scene). Only the instance root needs an owner for the parent scene to include it.

Actually, let me correct: the standard pattern is:
1. `instance_node.owner = root` — instance root owned by scene root
2. Do NOT set owner on instance's children — they belong to the instanced scene
3. `_repack_and_save` must skip children of instance nodes

This means `_set_owner_recursive` needs to be aware of instance boundaries. We need to modify it or use a variant that skips nodes whose `scene_file_path` is non-empty (indicating they're an instance root from another scene).

**Acceptance Criteria:**
- [ ] Instance appears as child of specified parent
- [ ] Instance serializes as scene reference (not inlined nodes)
- [ ] `scene_read` on the modified scene shows the instance with its children
- [ ] Custom `node_name` overrides the instance root name
- [ ] Name collision returns structured error
- [ ] Missing `instance_scene` returns structured error
- [ ] `instance_scene` that isn't a valid `.tscn` returns structured error

---

### Unit 3: GDScript `node_ops.gd` — `op_node_reparent`

**File**: `addons/director/ops/node_ops.gd`

```gdscript
static func op_node_reparent(params: Dictionary) -> Dictionary:
    ## Move a node to a new parent within the same scene.
    ##
    ## Params:
    ##   scene_path (String)
    ##   node_path (String) — node to move
    ##   new_parent_path (String) — destination parent
    ##   new_name (String, optional) — rename during reparent
    ## Returns: { success, data: { old_path, new_path } }
```

**Implementation Notes:**
- Load scene, instantiate
- Find target node by `node_path`; error if not found
- Cannot reparent root node — error if `node_path` is `"."` or empty
- Find new parent by `new_parent_path`; error if not found
- Cannot reparent a node to itself or to one of its own descendants — check with `new_parent.is_ancestor_of(target)` wait no, check `target.is_ancestor_of(new_parent)` (target is ancestor of new parent = circular). Also check `target == new_parent`.
- Determine final name: `new_name` if provided, else `target.name`
- Name collision check: `new_parent.has_node(final_name)` → error with message suggesting `new_name` param
- Record `old_path = str(root.get_path_to(target))`
- `target.get_parent().remove_child(target)`
- If `new_name`: `target.name = new_name`
- `new_parent.add_child(target)`
- `target.owner = root` (re-assert ownership after reparent)
- `_set_owner_recursive(target, root)` to fix children ownership
- Record `new_path = str(root.get_path_to(target))`
- `_repack_and_save(root, full_path)`

**Acceptance Criteria:**
- [ ] Node moves to new parent; `scene_read` confirms new position
- [ ] `old_path` and `new_path` in response are correct
- [ ] Root reparent returns error
- [ ] Missing target node returns error
- [ ] Missing new parent returns error
- [ ] Circular reparent (node to own descendant) returns error
- [ ] Name collision without `new_name` returns error with hint
- [ ] `new_name` resolves collision successfully
- [ ] Children of reparented node are preserved

---

### Unit 4: GDScript `ops/resource_ops.gd` — `op_resource_read`

**File**: `addons/director/ops/resource_ops.gd` (new file)

```gdscript
class_name ResourceOps


static func op_resource_read(params: Dictionary) -> Dictionary:
    ## Read a resource file and serialize its properties.
    ##
    ## Params: resource_path (String)
    ## Returns: { success, data: { type, path, properties: { ... } } }


static func _get_resource_properties(resource: Resource) -> Dictionary:
    ## Extract non-default properties from a resource, one level deep.
    ## Nested Resource values serialize as their resource_path string.


static func _serialize_resource_value(value) -> Variant:
    ## Like SceneOps._serialize_value but for resources.
    ## Resource references → path string (one level deep).


static func _error(message: String, operation: String, context: Dictionary) -> Dictionary:
    return {"success": false, "error": message, "operation": operation, "context": context}
```

**Implementation Notes:**
- `load("res://" + resource_path)` — works for `.tres`, `.res`, and any loadable resource
- If path ends with `.tscn`, return success but add a `hint` field: `"Use scene_read for scene tree structure"`
- Property extraction: same approach as `SceneOps._get_serializable_properties` but for Resource instead of Node
  - `resource.get_property_list()` to enumerate
  - Filter: skip `_`-prefixed, skip `script`, skip non-editor-usage
  - Compare against default instance from `ClassDB.instantiate(resource.get_class())`
  - Only include non-default values
- Nested resources: serialize as `resource_path` string if they have one, otherwise as `"<ClassName>"` placeholder
- Value serialization: reuse the same Vector2/Vector3/Color/etc. conversion from `SceneOps._serialize_value`. To avoid duplication, the implementation can call `SceneOps._serialize_value()` directly since both files are in the same project.
- Return: `{ type: resource.get_class(), path: resource_path, properties: { ... } }`

**Acceptance Criteria:**
- [ ] Reads `.tres` resource and returns type + properties
- [ ] Reads `.res` binary resource and returns type + properties
- [ ] Non-default properties only (same filtering as scene_read)
- [ ] Nested Resource values appear as path strings, not recursive objects
- [ ] Vector2, Color, etc. serialize correctly (reuses existing helpers)
- [ ] `.tscn` path works but includes hint to use `scene_read`
- [ ] Non-existent path returns structured error
- [ ] Unloadable path returns structured error

---

### Unit 5: GDScript `operations.gd` — Dispatcher update

**File**: `addons/director/operations.gd`

Add the new operation imports and match arms:

```gdscript
const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")  # NEW

# In _init() match block, add:
        "scene_list":
            result = SceneOps.op_scene_list(args.params)
        "scene_add_instance":
            result = SceneOps.op_scene_add_instance(args.params)
        "node_reparent":
            result = NodeOps.op_node_reparent(args.params)
        "resource_read":
            result = ResourceOps.op_resource_read(args.params)
```

**Acceptance Criteria:**
- [ ] All four new operations dispatch correctly
- [ ] Unknown operations still return error (existing behavior)

---

### Unit 6: GDScript `node_ops.gd` — Fix `_set_owner_recursive` for instances

**File**: `addons/director/ops/node_ops.gd`

The existing `_set_owner_recursive` sets owner on ALL descendants. This breaks scene instances — children of an instanced scene must NOT have their owner changed, or they'll serialize as inline nodes instead of as part of the instance.

```gdscript
static func _set_owner_recursive(node: Node, owner: Node):
    ## Set owner on all descendants, but skip children of scene instances.
    ## A node with a non-empty scene_file_path is an instance root from
    ## another scene — its children belong to that scene, not this one.
    for child in node.get_children():
        child.owner = owner
        if child.scene_file_path == "":
            # Only recurse into non-instance children
            _set_owner_recursive(child, owner)
```

**Implementation Notes:**
- `node.scene_file_path` is non-empty when a node is the root of an instanced scene
- When we encounter an instance root as a child, we set its owner (so the parent scene includes it) but do NOT recurse into its children (they belong to the instanced scene)
- This change is backwards-compatible with Phase 1: scenes without instances have no nodes with `scene_file_path` set, so behavior is identical

**Acceptance Criteria:**
- [ ] Phase 1 tests still pass (no regression)
- [ ] Instance children are not inlined when packing a scene with instances
- [ ] Instance root node has correct owner (scene root)

---

### Unit 7: Rust `mcp/scene.rs` — Parameter structs for new scene tools

**File**: `crates/director/src/mcp/scene.rs`

```rust
// Existing structs unchanged

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SceneListParams {
    /// Path to the Godot project directory.
    pub project_path: String,
    /// Subdirectory to list (relative to project root). Lists entire project if omitted.
    #[serde(default)]
    pub directory: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SceneAddInstanceParams {
    /// Path to the Godot project directory.
    pub project_path: String,
    /// Scene file to modify (relative to project, e.g. "scenes/level.tscn").
    pub scene_path: String,
    /// Scene to instance (relative to project, e.g. "scenes/player.tscn").
    pub instance_scene: String,
    /// Parent node path within the target scene (default: root ".").
    #[serde(default = "default_parent")]
    pub parent_path: String,
    /// Override the instance root's name. Uses the instanced scene's root name if omitted.
    #[serde(default)]
    pub node_name: Option<String>,
}

fn default_parent() -> String {
    ".".to_string()
}
```

**Acceptance Criteria:**
- [ ] `SceneListParams` has `project_path` (required) and `directory` (optional)
- [ ] `SceneAddInstanceParams` has all fields with correct defaults
- [ ] Both derive `JsonSchema` for MCP schema generation

---

### Unit 8: Rust `mcp/node.rs` — Parameter struct for reparent

**File**: `crates/director/src/mcp/node.rs`

```rust
// Existing structs unchanged

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct NodeReparentParams {
    /// Path to the Godot project directory.
    pub project_path: String,
    /// Scene file to modify (relative to project).
    pub scene_path: String,
    /// Path to the node to move.
    pub node_path: String,
    /// Path to the new parent node.
    pub new_parent_path: String,
    /// Rename the node during reparent. Useful to avoid name collisions.
    #[serde(default)]
    pub new_name: Option<String>,
}
```

**Acceptance Criteria:**
- [ ] `new_name` is optional, defaults to `None`
- [ ] All required fields present

---

### Unit 9: Rust `mcp/resource.rs` — Parameter struct + module

**File**: `crates/director/src/mcp/resource.rs` (new file)

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResourceReadParams {
    /// Path to the Godot project directory.
    pub project_path: String,
    /// Resource file to read (relative to project, e.g. "materials/ground.tres").
    pub resource_path: String,
}
```

**Acceptance Criteria:**
- [ ] Module exists with `ResourceReadParams` struct

---

### Unit 10: Rust `mcp/mod.rs` — Tool router additions

**File**: `crates/director/src/mcp/mod.rs`

Add module declaration and imports:

```rust
pub mod node;
pub mod resource;  // NEW
pub mod scene;

// Add to imports:
use node::{NodeAddParams, NodeRemoveParams, NodeReparentParams, NodeSetPropertiesParams};
use resource::ResourceReadParams;
use scene::{SceneAddInstanceParams, SceneCreateParams, SceneListParams, SceneReadParams};
```

Add four new tool handlers inside the `#[tool_router]` block:

```rust
#[tool(
    name = "scene_list",
    description = "List all Godot scene files (.tscn) in the project or a subdirectory, \
        with root node type and node count for each scene."
)]
pub async fn scene_list(
    &self,
    Parameters(params): Parameters<SceneListParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&params.project_path, "scene_list", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "scene_add_instance",
    description = "Add a scene instance (reference) as a child node in another Godot scene. \
        The instanced scene is linked, not copied — changes to the source scene propagate. \
        Always use this tool instead of editing .tscn files directly."
)]
pub async fn scene_add_instance(
    &self,
    Parameters(params): Parameters<SceneAddInstanceParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&params.project_path, "scene_add_instance", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "node_reparent",
    description = "Move a node to a new parent within the same Godot scene. Optionally \
        rename the node during the move. Always use this tool instead of editing .tscn \
        files directly."
)]
pub async fn node_reparent(
    &self,
    Parameters(params): Parameters<NodeReparentParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&params.project_path, "node_reparent", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "resource_read",
    description = "Read a Godot resource file (.tres, .res) and return its type and \
        properties as structured data. For scene files (.tscn), prefer scene_read which \
        returns the full node tree."
)]
pub async fn resource_read(
    &self,
    Parameters(params): Parameters<ResourceReadParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&params.project_path, "resource_read", &op_params).await?;
    serialize_response(&data)
}
```

**Acceptance Criteria:**
- [ ] All four tools registered in the tool router
- [ ] Tool descriptions include anti-direct-edit guidance where applicable
- [ ] `resource_read` description mentions preferring `scene_read` for `.tscn`
- [ ] Each handler follows the same ~5-line pattern as Phase 1

---

### Unit 11: Test fixture — `.tres` resource for `resource_read`

**File**: `tests/godot-project/fixtures/test_material.tres` (new file)

A pre-created StandardMaterial3D with a few non-default properties:

```
[gd_resource type="StandardMaterial3D" format=3]

[resource]
albedo_color = Color(1, 0, 0, 1)
metallic = 0.8
roughness = 0.2
```

**Implementation Note:** This file must be hand-authored since we have no
`resource_create` tool yet. The `.tres` text format for simple resources is
stable and safe to author by hand (unlike `.tscn` which has internal cross-references).

Also create a fixtures directory:

**File**: `tests/godot-project/fixtures/.gitkeep` (ensure directory exists)

**Acceptance Criteria:**
- [ ] File loads correctly in headless Godot
- [ ] `resource_read` returns `type: "StandardMaterial3D"` with the three non-default properties

---

### Unit 12: E2E tests — `test_scene_list.rs`

**File**: `tests/director-tests/src/test_scene_list.rs` (new file)

```rust
use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_returns_project_scenes() {
    // The test project has test_scene_2d.tscn and test_scene_3d.tscn at root
    let f = DirectorFixture::new();
    let data = f.run("scene_list", json!({
        "directory": ""
    })).unwrap().unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert!(scenes.len() >= 2, "expected at least 2 scenes, got {}", scenes.len());

    // Verify structure of entries
    let first = &scenes[0];
    assert!(first["path"].is_string());
    assert!(first["root_type"].is_string());
    assert!(first["node_count"].is_number());
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_with_directory_filter() {
    let f = DirectorFixture::new();

    // Create a scene in a subdirectory
    let scene = "tmp/subdir/listed.tscn";
    f.run("scene_create", json!({
        "scene_path": scene,
        "root_type": "Node3D"
    })).unwrap().unwrap_data();

    let data = f.run("scene_list", json!({
        "directory": "tmp/subdir"
    })).unwrap().unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0]["path"], "tmp/subdir/listed.tscn");
    assert_eq!(scenes[0]["root_type"], "Node3D");
    assert_eq!(scenes[0]["node_count"], 1);
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_nonexistent_directory_returns_error() {
    let f = DirectorFixture::new();
    let err = f.run("scene_list", json!({
        "directory": "nonexistent/path"
    })).unwrap().unwrap_err();
    assert!(err.contains("not found") || err.contains("does not exist"));
}
```

**Acceptance Criteria:**
- [ ] Lists known test project scenes
- [ ] Directory filter works
- [ ] Non-existent directory returns error

---

### Unit 13: E2E tests — `test_instance.rs`

**File**: `tests/director-tests/src/test_instance.rs` (new file)

```rust
use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_basic() {
    let f = DirectorFixture::new();

    // Create the scene to be instanced
    let child_scene = DirectorFixture::temp_scene_path("instance_child");
    f.run("scene_create", json!({
        "scene_path": child_scene,
        "root_type": "CharacterBody2D"
    })).unwrap().unwrap_data();

    // Add a node to the child scene so we can verify it appears
    f.run("node_add", json!({
        "scene_path": child_scene,
        "node_type": "Sprite2D",
        "node_name": "Sprite"
    })).unwrap().unwrap_data();

    // Create the parent scene
    let parent_scene = DirectorFixture::temp_scene_path("instance_parent");
    f.run("scene_create", json!({
        "scene_path": parent_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    // Instance the child into the parent
    let data = f.run("scene_add_instance", json!({
        "scene_path": parent_scene,
        "instance_scene": child_scene
    })).unwrap().unwrap_data();

    assert_eq!(data["instance_scene"], child_scene);
    assert!(data["node_path"].is_string());

    // Read back and verify the instance appears with its children
    let tree = f.run("scene_read", json!({
        "scene_path": parent_scene
    })).unwrap().unwrap_data();

    let root = &tree["root"];
    let children = root["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["type"], "CharacterBody2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_with_custom_name() {
    let f = DirectorFixture::new();

    let child_scene = DirectorFixture::temp_scene_path("instance_named_child");
    f.run("scene_create", json!({
        "scene_path": child_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    let parent_scene = DirectorFixture::temp_scene_path("instance_named_parent");
    f.run("scene_create", json!({
        "scene_path": parent_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    let data = f.run("scene_add_instance", json!({
        "scene_path": parent_scene,
        "instance_scene": child_scene,
        "node_name": "MyPlayer"
    })).unwrap().unwrap_data();

    // Read back and check name
    let tree = f.run("scene_read", json!({
        "scene_path": parent_scene
    })).unwrap().unwrap_data();

    assert_eq!(tree["root"]["children"][0]["name"], "MyPlayer");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_missing_scene_returns_error() {
    let f = DirectorFixture::new();

    let parent_scene = DirectorFixture::temp_scene_path("instance_err_parent");
    f.run("scene_create", json!({
        "scene_path": parent_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    let err = f.run("scene_add_instance", json!({
        "scene_path": parent_scene,
        "instance_scene": "nonexistent/nope.tscn"
    })).unwrap().unwrap_err();

    assert!(err.contains("not found") || err.contains("does not exist"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_name_collision_returns_error() {
    let f = DirectorFixture::new();

    let child_scene = DirectorFixture::temp_scene_path("instance_collision_child");
    f.run("scene_create", json!({
        "scene_path": child_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    let parent_scene = DirectorFixture::temp_scene_path("instance_collision_parent");
    f.run("scene_create", json!({
        "scene_path": parent_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    // Add first instance
    f.run("scene_add_instance", json!({
        "scene_path": parent_scene,
        "instance_scene": child_scene
    })).unwrap().unwrap_data();

    // Second instance with same name should error
    let err = f.run("scene_add_instance", json!({
        "scene_path": parent_scene,
        "instance_scene": child_scene
    })).unwrap().unwrap_err();

    assert!(err.to_lowercase().contains("name") || err.to_lowercase().contains("already exists"));
}
```

**Acceptance Criteria:**
- [ ] Basic instance round-trip works
- [ ] Custom name override works
- [ ] Missing instance scene returns error
- [ ] Name collision returns error

---

### Unit 14: E2E tests — `test_reparent.rs`

**File**: `tests/director-tests/src/test_reparent.rs` (new file)

```rust
use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_basic() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_basic");

    // Create scene with structure: Root > A > Child, Root > B
    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "A"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "A", "node_type": "Sprite2D", "node_name": "Child"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "B"}))
        .unwrap().unwrap_data();

    // Reparent Child from A to B
    let data = f.run("node_reparent", json!({
        "scene_path": scene,
        "node_path": "A/Child",
        "new_parent_path": "B"
    })).unwrap().unwrap_data();

    assert_eq!(data["old_path"], "A/Child");
    assert_eq!(data["new_path"], "B/Child");

    // Verify via scene_read
    let tree = f.run("scene_read", json!({"scene_path": scene}))
        .unwrap().unwrap_data();
    let root = &tree["root"];

    // A should have no children
    let a = &root["children"].as_array().unwrap().iter()
        .find(|c| c["name"] == "A").unwrap();
    assert!(a.get("children").is_none() || a["children"].as_array().unwrap().is_empty());

    // B should have Child
    let b = &root["children"].as_array().unwrap().iter()
        .find(|c| c["name"] == "B").unwrap();
    let b_children = b["children"].as_array().unwrap();
    assert_eq!(b_children.len(), 1);
    assert_eq!(b_children[0]["name"], "Child");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_with_rename() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_rename");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Source"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Source", "node_type": "Sprite2D", "node_name": "Sprite"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Target"}))
        .unwrap().unwrap_data();

    let data = f.run("node_reparent", json!({
        "scene_path": scene,
        "node_path": "Source/Sprite",
        "new_parent_path": "Target",
        "new_name": "RenamedSprite"
    })).unwrap().unwrap_data();

    assert_eq!(data["new_path"], "Target/RenamedSprite");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_root_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_root");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Child"}))
        .unwrap().unwrap_data();

    let err = f.run("node_reparent", json!({
        "scene_path": scene,
        "node_path": ".",
        "new_parent_path": "Child"
    })).unwrap().unwrap_err();

    assert!(err.to_lowercase().contains("root"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_circular_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_circular");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Parent"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Parent", "node_type": "Node2D", "node_name": "Child"}))
        .unwrap().unwrap_data();

    // Try to reparent Parent under its own Child
    let err = f.run("node_reparent", json!({
        "scene_path": scene,
        "node_path": "Parent",
        "new_parent_path": "Parent/Child"
    })).unwrap().unwrap_err();

    assert!(err.to_lowercase().contains("circular") || err.to_lowercase().contains("descendant") || err.to_lowercase().contains("ancestor"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_name_collision_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_collision");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "A"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "A", "node_type": "Sprite2D", "node_name": "Dupe"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "B"}))
        .unwrap().unwrap_data();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "B", "node_type": "Node2D", "node_name": "Dupe"}))
        .unwrap().unwrap_data();

    // Reparent A/Dupe to B — name collision with B/Dupe
    let err = f.run("node_reparent", json!({
        "scene_path": scene,
        "node_path": "A/Dupe",
        "new_parent_path": "B"
    })).unwrap().unwrap_err();

    assert!(err.to_lowercase().contains("name") || err.to_lowercase().contains("exists"));
}
```

**Acceptance Criteria:**
- [ ] Basic reparent works with correct old/new paths
- [ ] Rename during reparent works
- [ ] Root reparent errors
- [ ] Circular reparent errors
- [ ] Name collision errors

---

### Unit 15: E2E tests — `test_resource.rs`

**File**: `tests/director-tests/src/test_resource.rs` (new file)

```rust
use crate::harness::{assert_approx, DirectorFixture};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_tres_material() {
    let f = DirectorFixture::new();
    let data = f.run("resource_read", json!({
        "resource_path": "fixtures/test_material.tres"
    })).unwrap().unwrap_data();

    assert_eq!(data["type"], "StandardMaterial3D");
    assert_eq!(data["path"], "fixtures/test_material.tres");

    let props = &data["properties"];
    // albedo_color = Color(1, 0, 0, 1)
    assert_approx(props["albedo_color"]["r"].as_f64().unwrap(), 1.0);
    assert_approx(props["albedo_color"]["g"].as_f64().unwrap(), 0.0);
    assert_approx(props["albedo_color"]["b"].as_f64().unwrap(), 0.0);
    // metallic = 0.8
    assert_approx(props["metallic"].as_f64().unwrap(), 0.8);
    // roughness = 0.2
    assert_approx(props["roughness"].as_f64().unwrap(), 0.2);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_nonexistent_returns_error() {
    let f = DirectorFixture::new();
    let err = f.run("resource_read", json!({
        "resource_path": "nonexistent/nope.tres"
    })).unwrap().unwrap_err();

    assert!(err.contains("not found") || err.contains("does not exist") || err.contains("Failed"));
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_tscn_includes_hint() {
    let f = DirectorFixture::new();
    // test_scene_2d.tscn exists in the test project
    let data = f.run("resource_read", json!({
        "resource_path": "test_scene_2d.tscn"
    })).unwrap().unwrap_data();

    // Should succeed but include a hint
    assert!(data["type"].is_string());
    assert!(data["hint"].as_str().unwrap().contains("scene_read"));
}
```

**Acceptance Criteria:**
- [ ] Reads `.tres` with correct type and property values
- [ ] Non-existent resource returns error
- [ ] `.tscn` works but includes hint

---

### Unit 16: E2E tests — `test_journey.rs` update

**File**: `tests/director-tests/src/test_journey.rs`

Add a Phase 2 journey test:

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_compose_scene_with_instances_and_reparent() {
    let f = DirectorFixture::new();

    // 1. Create an enemy scene
    let enemy_scene = DirectorFixture::temp_scene_path("journey2_enemy");
    f.run("scene_create", json!({
        "scene_path": enemy_scene,
        "root_type": "CharacterBody2D"
    })).unwrap().unwrap_data();
    f.run("node_add", json!({
        "scene_path": enemy_scene,
        "node_type": "Sprite2D",
        "node_name": "Sprite"
    })).unwrap().unwrap_data();

    // 2. Create a level scene
    let level_scene = DirectorFixture::temp_scene_path("journey2_level");
    f.run("scene_create", json!({
        "scene_path": level_scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    // 3. Add structure: Enemies group and Staging area
    f.run("node_add", json!({
        "scene_path": level_scene,
        "node_type": "Node2D",
        "node_name": "Enemies"
    })).unwrap().unwrap_data();
    f.run("node_add", json!({
        "scene_path": level_scene,
        "node_type": "Node2D",
        "node_name": "Staging"
    })).unwrap().unwrap_data();

    // 4. Instance enemy into Staging
    f.run("scene_add_instance", json!({
        "scene_path": level_scene,
        "instance_scene": enemy_scene,
        "parent_path": "Staging",
        "node_name": "Enemy1"
    })).unwrap().unwrap_data();

    // 5. Reparent Enemy1 from Staging to Enemies
    let reparent_data = f.run("node_reparent", json!({
        "scene_path": level_scene,
        "node_path": "Staging/Enemy1",
        "new_parent_path": "Enemies"
    })).unwrap().unwrap_data();
    assert_eq!(reparent_data["old_path"], "Staging/Enemy1");
    assert_eq!(reparent_data["new_path"], "Enemies/Enemy1");

    // 6. Verify final structure
    let tree = f.run("scene_read", json!({
        "scene_path": level_scene
    })).unwrap().unwrap_data();
    let root = &tree["root"];

    // Staging should be empty
    let staging = root["children"].as_array().unwrap().iter()
        .find(|c| c["name"] == "Staging").unwrap();
    assert!(staging.get("children").is_none()
        || staging["children"].as_array().unwrap().is_empty());

    // Enemies should have Enemy1
    let enemies = root["children"].as_array().unwrap().iter()
        .find(|c| c["name"] == "Enemies").unwrap();
    let enemy_children = enemies["children"].as_array().unwrap();
    assert_eq!(enemy_children.len(), 1);
    assert_eq!(enemy_children[0]["name"], "Enemy1");

    // 7. scene_list should find both scenes
    let list = f.run("scene_list", json!({
        "directory": "tmp"
    })).unwrap().unwrap_data();
    let scenes = list["scenes"].as_array().unwrap();
    assert!(scenes.len() >= 2);
}
```

**Acceptance Criteria:**
- [ ] Full composition workflow: create scenes → instance → reparent → verify
- [ ] All operations compose correctly in sequence

---

### Unit 17: Test module declarations

**File**: `tests/director-tests/src/lib.rs`

```rust
mod harness;
mod test_instance;     // NEW
mod test_journey;
mod test_node;
mod test_reparent;     // NEW
mod test_resource;     // NEW
mod test_scene;
mod test_scene_list;   // NEW
```

**Acceptance Criteria:**
- [ ] All new test modules declared
- [ ] `cargo test -p director-tests` compiles (tests are `#[ignore]` so they won't run without Godot)

---

## Implementation Order

1. **Unit 6** — Fix `_set_owner_recursive` for instance awareness (prerequisite for Unit 2)
2. **Unit 4** — `resource_ops.gd` (new file, no dependencies on other new code)
3. **Unit 1** — `op_scene_list` in `scene_ops.gd`
4. **Unit 3** — `op_node_reparent` in `node_ops.gd`
5. **Unit 2** — `op_scene_add_instance` in `scene_ops.gd` (depends on Unit 6)
6. **Unit 5** — `operations.gd` dispatcher update (requires Units 1-4)
7. **Units 7-9** — Rust param structs (all independent, can be parallel)
8. **Unit 10** — Rust tool router additions (depends on Units 7-9)
9. **Unit 11** — Test fixture `.tres` file
10. **Units 12-16** — E2E tests (depends on all GDScript + Rust units)
11. **Unit 17** — Test module declarations

## Testing

### Running tests

```bash
# Deploy Director addon to test project (already done by Phase 1 workflow)
# The operations.gd and ops/ are symlinked or copied

# Compile check (no Godot needed)
cargo build -p director
cargo test -p director-tests  # compiles but #[ignore] tests skip

# Full E2E (requires GODOT_BIN)
cargo test -p director-tests -- --include-ignored
```

### Test coverage matrix

| Tool | Happy path | Error cases | Journey |
|---|---|---|---|
| `scene_list` | list all, filter by dir | nonexistent dir | journey2 |
| `scene_add_instance` | basic, custom name | missing scene, name collision | journey2 |
| `node_reparent` | basic, with rename | root, circular, name collision | journey2 |
| `resource_read` | .tres material | nonexistent, .tscn hint | — |

## Verification Checklist

```bash
# 1. Compile
cargo build --workspace

# 2. Clippy
cargo clippy --workspace

# 3. Phase 1 regression
cargo test -p director-tests -- --include-ignored test_scene test_node test_journey

# 4. Phase 2 tests
cargo test -p director-tests -- --include-ignored test_scene_list test_instance test_reparent test_resource journey_compose
```

## Future Extensions (deferred)

- **`scene_list` glob support**: Add `pattern: string?` param for glob matching (e.g. `"scenes/**/*.tscn"`). Contract-compatible addition.
- **`resource_read` recursive depth**: Add `depth: number?` param to control nested resource serialization. Default 1 preserves current behavior.
- **`scene_list` node_count opt-in**: If performance becomes an issue on large projects, add `include_node_count: bool` (default true for backwards compat).
