<script setup>
import { data } from '../.vitepress/data/tools.data'

const node_add = data.params['node_add'] ?? []
const node_remove = data.params['node_remove'] ?? []
const node_set_properties = data.params['node_set_properties'] ?? []
const node_reparent = data.params['node_reparent'] ?? []
const node_find = data.params['node_find'] ?? []
const node_set_groups = data.params['node_set_groups'] ?? []
const node_set_script = data.params['node_set_script'] ?? []
const node_set_meta = data.params['node_set_meta'] ?? []

const messages0 = [
  { role: 'human', text: `Add a BoxShape3D collision to the Platform_5 node. Make it 4m wide, 0.5m tall, 4m deep.` },
  { role: 'agent', text: `Done. CollisionShape3D added under Platform_5 with a BoxShape3D (4×0.5×4m).` },
]
const messages1 = [
  { role: 'human', text: `Set collision_layer=2 and collision_mask=1 on Enemy_0, Enemy_1, and Enemy_2.` },
  { role: 'agent', text: `All three enemies updated. collision_layer=2, collision_mask=1 applied to Enemy_0, Enemy_1, and Enemy_2.` },
]
</script>

# Node Manipulation

Add, remove, and modify nodes in a Godot scene.

## Operations

### `node_add`

Add a new node to a scene.

```json
{
  "op": "node_add",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "parent_path": "Level/Platforms",
  "node_type": "StaticBody3D",
  "node_name": "Platform_5"
}
```

<ParamTable :params="node_add" />

**Response:**
```json
{
  "op": "node_add",
  "node_name": "Platform_5",
  "node_type": "StaticBody3D",
  "path": "Level/Platforms/Platform_5",
  "result": "ok"
}
```

The `path` in the response is the full scene-relative path to the newly created node.

### `node_remove`

Remove a node and all its children.

```json
{
  "op": "node_remove",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Platforms/Platform_Old"
}
```

<ParamTable :params="node_remove" />

**Response:**
```json
{
  "op": "node_remove",
  "node_path": "Level/Platforms/Platform_Old",
  "result": "ok"
}
```

### `node_set_properties`

Set one or more properties on a node.

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Enemy_0",
  "properties": {
    "collision_layer": 2
  }
}
```

<ParamTable :params="node_set_properties" />

To set multiple properties at once, pass more keys in `properties`:

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/enemy.tscn",
  "node_path": "Enemy",
  "properties": {
    "collision_layer": 2,
    "collision_mask": 1,
    "motion_mode": "grounded",
    "floor_snap_length": 0.1
  }
}
```

**Response:**
```json
{
  "op": "node_set_properties",
  "node_path": "Level/Enemy_0",
  "properties_set": ["collision_layer"],
  "result": "ok"
}
```

### `node_reparent`

Change a node's parent or rename it in place.

```json
{
  "op": "node_reparent",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Pickups/Coin_0",
  "new_parent_path": "Level/Room2/Pickups"
}
```

<ParamTable :params="node_reparent" />

**Response:**
```json
{
  "op": "node_reparent",
  "node_path": "Level/Pickups/Coin_0",
  "new_path": "Level/Room2/Pickups/Coin_0",
  "result": "ok"
}
```

### `node_find`

Search nodes by class, group, name pattern, or property value.

```json
{
  "op": "node_find",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "class_name": "CharacterBody3D",
  "group": "enemies",
  "name_pattern": "Enemy_*",
  "limit": 50
}
```

<ParamTable :params="node_find" />

**Response:**
```json
{
  "op": "node_find",
  "nodes": [
    { "name": "Enemy_0", "class": "CharacterBody3D", "path": "Level/Enemies/Enemy_0" },
    { "name": "Enemy_1", "class": "CharacterBody3D", "path": "Level/Enemies/Enemy_1" }
  ],
  "result": "ok"
}
```

### `node_set_groups`

Add or remove a node from groups.

```json
{
  "op": "node_set_groups",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Enemy_0",
  "add": ["enemies", "patrol_units"],
  "remove": ["idle"]
}
```

<ParamTable :params="node_set_groups" />

### `node_set_script`

Attach a GDScript to a node. Omit `script_path` to detach the current script.

```json
{
  "op": "node_set_script",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Enemy_0",
  "script_path": "scripts/enemy_ai.gd"
}
```

<ParamTable :params="node_set_script" />

### `node_set_meta`

Set metadata on a node.

```json
{
  "op": "node_set_meta",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "Level/Enemy_0",
  "meta": {
    "spawn_group": "wave_1",
    "difficulty": 2
  }
}
```

<ParamTable :params="node_set_meta" />

## Property types

Director maps JSON types to Godot property types automatically:

| JSON type | Godot type |
|---|---|
| `number` (integer) | `int` |
| `number` (float) | `float` |
| `boolean` | `bool` |
| `string` | `String`, `NodePath`, `StringName` |
| `[x, y]` (2-element array) | `Vector2` |
| `[x, y, z]` (3-element array) | `Vector3` |
| `[x, y, z, w]` (4-element array) | `Vector4`, `Quaternion`, `Color` |
| `[[r,g,b,a]]` | `Color` (if property expects Color) |
| `object` | `Dictionary` |
| `array` | `Array` |

For ambiguous cases (e.g., a 4-element array that could be a `Quaternion` or `Color`), Director uses the property's declared type from Godot's class database to disambiguate.

## Example conversations

### Building a collision shape

<AgentConversation :messages="messages0" />

### Fixing collision layers on multiple nodes

<AgentConversation :messages="messages1" />

## Tips

**Use `node_set_properties` for all property setting.** Setting 5 properties with one call is faster than 5 separate calls — see [Batch Operations](/director/batch).

**Node paths are relative to the scene root.** The root node itself is `"."`. A child named `Player` is `"Player"`. A grandchild is `"Player/Camera3D"`. Use `scene_read` to confirm paths.

**`node_remove` is permanent.** There is no undo within Director. If you are making destructive changes, use `scene_diff` or git to review before removing.
