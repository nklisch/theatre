<script setup>
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
  "path": "scenes/player.tscn",
  "root_class": "CharacterBody3D",
  "root_name": "Player"
}
```

| Parameter | Type | Description |
|---|---|---|
| `project_path` | `string` | Absolute path to Godot project |
| `path` | `string` | Relative path for the new scene (from project root) |
| `root_class` | `string` | Godot class for the root node |
| `root_name` | `string` | Name of the root node (default: class name) |

**Response:**
```json
{
  "op": "scene_create",
  "path": "scenes/player.tscn",
  "root_name": "Player",
  "root_class": "CharacterBody3D",
  "result": "ok"
}
```

### `scene_read`

Read the structure of an existing scene — all nodes, their classes, properties, and hierarchy.

```json
{
  "op": "scene_read",
  "project_path": "/home/user/my-game",
  "path": "scenes/player.tscn",
  "max_depth": 4
}
```

**Response:**
```json
{
  "op": "scene_read",
  "path": "scenes/player.tscn",
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
    },
    {
      "name": "CollisionShape3D",
      "class": "CollisionShape3D",
      "path": "CollisionShape3D",
      "properties": {
        "shape": "CapsuleShape3D(radius=0.4, height=1.8)"
      },
      "children": []
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

**Response:**
```json
{
  "scenes": [
    { "path": "scenes/enemies/basic_enemy.tscn", "root_class": "CharacterBody3D" },
    { "path": "scenes/enemies/boss.tscn", "root_class": "CharacterBody3D" },
    { "path": "scenes/enemies/flying_enemy.tscn", "root_class": "CharacterBody3D" }
  ]
}
```

### `scene_instance`

Add an instance of another scene as a child node.

```json
{
  "op": "scene_instance",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "parent": "Level/Enemies",
  "source_scene": "scenes/enemies/basic_enemy.tscn",
  "name": "Enemy_4",
  "position": [10.0, 0.0, -5.0]
}
```

| Parameter | Type | Description |
|---|---|---|
| `scene` | `string` | The scene to add the instance to |
| `parent` | `string` | Node path within the scene to parent the instance under |
| `source_scene` | `string` | The scene to instantiate |
| `name` | `string` | Name for the instance node |
| `position` | `[x,y,z]` | Initial position (optional) |

**Response:**
```json
{
  "op": "scene_instance",
  "name": "Enemy_4",
  "source_scene": "scenes/enemies/basic_enemy.tscn",
  "result": "ok"
}
```

### `scene_diff`

Compare two scenes and return a list of differences.

```json
{
  "op": "scene_diff",
  "project_path": "/home/user/my-game",
  "scene_a": "scenes/level_01.tscn",
  "scene_b": "scenes/level_01_backup.tscn"
}
```

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

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Use `scene_read` before modifying.** Know the existing structure before adding nodes. This prevents duplicate additions or wrong parent paths.

**Use `scene_list` to find existing scenes.** Before creating a new enemy scene, check if one already exists. `scene_list` with a directory filter is fast.

**`scene_instance` vs `node_add`.** Use `scene_instance` when you want to place a pre-built scene (like an enemy prefab) into a level. Use `node_add` when building node hierarchy from scratch.

**`scene_diff` for auditing AI changes.** After a batch of Director operations, diff the modified scene against its last git version to see exactly what changed.
