<script setup>
import { data } from '../.vitepress/data/tools.data'

const scene_create = data.params['scene_create'] ?? []
const scene_read = data.params['scene_read'] ?? []
const scene_list = data.params['scene_list'] ?? []
const scene_add_instance = data.params['scene_add_instance'] ?? []
const scene_diff = data.params['scene_diff'] ?? []
const uid_get = data.params['uid_get'] ?? []
const uid_update_project = data.params['uid_update_project'] ?? []
const export_mesh_library = data.params['export_mesh_library'] ?? []

const messages0 = [
  { role: 'human', text: `Create a new player scene with a CharacterBody3D root, and add a CapsuleShape3D collision shape.` },
  { role: 'agent', text: `Player scene created at scenes/player.tscn. Now adding the collision shape.` },
  { role: 'agent', text: `Done. Player scene has a CapsuleShape3D collision with radius 0.4 and height 1.8. Open scenes/player.tscn in the editor to verify.` },
]
</script>

# Scene Operations

Director can create, read, list, instance, and compare Godot scenes.

## Operations

### `scene_create`

Create a new empty scene with a root node.

```json
{
  "op": "scene_create",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/player.tscn",
  "root_type": "CharacterBody3D"
}
```

<ParamTable :params="scene_create" />

**Response:**
```json
{
  "op": "scene_create",
  "scene_path": "scenes/player.tscn",
  "root_type": "CharacterBody3D",
  "result": "ok"
}
```

### `scene_read`

Read the structure of an existing scene — all nodes, their classes, properties, and hierarchy.

```json
{
  "op": "scene_read",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/player.tscn",
  "depth": 4
}
```

<ParamTable :params="scene_read" />

**Response:**
```json
{
  "op": "scene_read",
  "scene_path": "scenes/player.tscn",
  "nodes": [
    {
      "name": "Player",
      "class": "CharacterBody3D",
      "path": ".",
      "properties": {
        "collision_layer": 1,
        "collision_mask": 3,
        "motion_mode": "grounded"
      },
      "children": ["CollisionShape3D", "MeshInstance3D", "Camera3D", "AnimationPlayer"]
    }
  ]
}
```

### `scene_list`

List all `.tscn` files in the project (or a subdirectory).

```json
{
  "op": "scene_list",
  "project_path": "/home/user/my-game",
  "directory": "scenes/enemies"
}
```

<ParamTable :params="scene_list" />

**Response:**
```json
{
  "scenes": [
    { "path": "scenes/enemies/basic_enemy.tscn", "root_class": "CharacterBody3D" },
    { "path": "scenes/enemies/boss.tscn", "root_class": "CharacterBody3D" }
  ]
}
```

### `scene_add_instance`

Add an instance of another scene as a child node.

```json
{
  "op": "scene_add_instance",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "instance_scene": "scenes/enemies/basic_enemy.tscn",
  "parent_path": "Level/Enemies",
  "node_name": "Enemy_4"
}
```

<ParamTable :params="scene_add_instance" />

**Response:**
```json
{
  "op": "scene_add_instance",
  "node_name": "Enemy_4",
  "instance_scene": "scenes/enemies/basic_enemy.tscn",
  "result": "ok"
}
```

### `scene_diff`

Compare two scenes and return a list of differences. Supports git refs.

```json
{
  "op": "scene_diff",
  "project_path": "/home/user/my-game",
  "scene_a": "scenes/level_01.tscn",
  "scene_b": "scenes/level_01_backup.tscn"
}
```

<ParamTable :params="scene_diff" />

**Response:**
```json
{
  "differences": [
    {
      "type": "node_added",
      "path": "Level/Platform_5",
      "class": "StaticBody3D",
      "in": "scene_a"
    },
    {
      "type": "property_changed",
      "path": "Level/Enemy_0",
      "property": "position",
      "value_a": [5.0, 0.0, -3.0],
      "value_b": [3.0, 0.0, -3.0]
    }
  ]
}
```

### `uid_get`

Look up the UID for a resource path.

```json
{
  "op": "uid_get",
  "project_path": "/home/user/my-game",
  "file_path": "scenes/enemies/basic_enemy.tscn"
}
```

<ParamTable :params="uid_get" />

**Response:**
```json
{
  "op": "uid_get",
  "file_path": "scenes/enemies/basic_enemy.tscn",
  "uid": "uid://abc123xyz"
}
```

### `uid_update_project`

Rescan all resources and rebuild the project UID cache. Run after adding or moving resource files outside of the editor.

```json
{
  "op": "uid_update_project",
  "project_path": "/home/user/my-game"
}
```

<ParamTable :params="uid_update_project" />

### `export_mesh_library`

Export meshes from a scene into a MeshLibrary resource for use with GridMap.

```json
{
  "op": "export_mesh_library",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/dungeon_tiles.tscn",
  "output_path": "assets/dungeon_tiles.meshlib"
}
```

<ParamTable :params="export_mesh_library" />

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Use `scene_read` before modifying.** Know the existing structure before adding nodes. This prevents duplicate additions or wrong parent paths.

**Use `scene_list` to find existing scenes.** Before creating a new enemy scene, check if one already exists. `scene_list` with a directory filter is fast.

**`scene_add_instance` vs `node_add`.** Use `scene_add_instance` when you want to place a pre-built scene (like an enemy prefab) into a level. Use `node_add` when building node hierarchy from scratch.

**`scene_diff` for auditing AI changes.** After a batch of Director operations, diff the modified scene against its last git version to see exactly what changed.
