<script setup>
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
  "scene": "scenes/level_01.tscn",
  "parent": "Level/Platforms",
  "name": "Platform_5",
  "class": "StaticBody3D"
}
```

| Parameter | Type | Description |
|---|---|---|
| `scene` | `string` | Scene file path (relative to project) |
| `parent` | `string` | Node path to parent under. Use `"."` for scene root. |
| `name` | `string` | Name for the new node |
| `class` | `string` | Godot class name |
| `position` | `[x,y,z]` | Initial position in 3D space (optional) |
| `properties` | `object` | Initial property values to set (optional) |

**Response:**
```json
{
  "op": "node_add",
  "name": "Platform_5",
  "class": "StaticBody3D",
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
  "scene": "scenes/level_01.tscn",
  "node": "Level/Platforms/Platform_Old"
}
```

**Response:**
```json
{
  "op": "node_remove",
  "node": "Level/Platforms/Platform_Old",
  "result": "ok"
}
```

### `node_set_property`

Set one or more properties on a node.

```json
{
  "op": "node_set_property",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "Level/Enemy_0",
  "property": "collision_layer",
  "value": 2
}
```

To set multiple properties at once, use `properties` (object):

```json
{
  "op": "node_set_property",
  "project_path": "/home/user/my-game",
  "scene": "scenes/enemy.tscn",
  "node": "Enemy",
  "properties": {
    "collision_layer": 2,
    "collision_mask": 1,
    "motion_mode": "grounded",
    "floor_snap_length": 0.1
  }
}
```

Setting `Vector3` values:

```json
{
  "op": "node_set_property",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "Level/Spawn",
  "property": "position",
  "value": [5.0, 1.0, -3.0]
}
```

**Response:**
```json
{
  "op": "node_set_property",
  "node": "Level/Enemy_0",
  "properties_set": ["collision_layer"],
  "result": "ok"
}
```

### `node_get_property`

Read one or more properties from a node.

```json
{
  "op": "node_get_property",
  "project_path": "/home/user/my-game",
  "scene": "scenes/enemy.tscn",
  "node": "Enemy/DetectionZone",
  "properties": ["collision_layer", "collision_mask", "monitoring"]
}
```

**Response:**
```json
{
  "op": "node_get_property",
  "node": "Enemy/DetectionZone",
  "properties": {
    "collision_layer": 2,
    "collision_mask": 0,
    "monitoring": true
  }
}
```

### `node_move`

Change a node's parent (reparent).

```json
{
  "op": "node_move",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "Level/Pickups/Coin_0",
  "new_parent": "Level/Room2/Pickups"
}
```

**Response:**
```json
{
  "op": "node_move",
  "node": "Level/Pickups/Coin_0",
  "new_path": "Level/Room2/Pickups/Coin_0",
  "result": "ok"
}
```

### `node_rename`

Rename a node (changes its name in the scene tree, not its parent).

```json
{
  "op": "node_rename",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "Level/Enemy_0",
  "new_name": "GuardCaptain"
}
```

**Response:**
```json
{
  "op": "node_rename",
  "old_path": "Level/Enemy_0",
  "new_path": "Level/GuardCaptain",
  "result": "ok"
}
```

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

**Use `properties` (plural) for batch property setting.** Setting 5 properties with one call is faster than 5 separate `node_set_property` calls — see [Batch Operations](/director/batch).

**Node paths are relative to the scene root.** The root node itself is `"."`. A child named `Player` is `"Player"`. A grandchild is `"Player/Camera3D"`. Use `scene_read` to confirm paths.

**`node_remove` is permanent.** There is no undo within Director. If you are making destructive changes, use `scene_diff` or git to review before removing.

**Position is a shortcut.** The `position` parameter in `node_add` sets `Node3D.position` (local position). For global position, use `node_set_property` with `"property": "position"` after adding.
