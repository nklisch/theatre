<script setup>
const messages0 = [
  { role: 'human', text: `Create a bouncy physics material for the player and assign it.` },
  { role: 'agent', text: `PhysicsMaterial created and saved at assets/physics/player_bouncy.tres. Assigning to the player.` },
  { role: 'agent', text: `Done. Player now uses the bouncy physics material (bounce=0.8, friction=0.3). Run the game to test.` },
]
</script>

# Resources

Create and modify Godot resources — the building blocks that nodes reference.

Resources in Godot are data objects separate from the scene tree: `PhysicsMaterial`, `StandardMaterial3D`, `BoxShape3D`, `CapsuleShape3D`, `AudioStream`, and hundreds more. Nodes hold references to resources; Director can create and configure those resources.

## Operations

### `resource_create`

Create a new resource and optionally save it to disk.

```json
{
  "op": "resource_create",
  "project_path": "/home/user/my-game",
  "type": "StandardMaterial3D",
  "properties": {
    "albedo_color": [0.8, 0.2, 0.2, 1.0],
    "roughness": 0.7,
    "metallic": 0.0
  },
  "save_path": "assets/materials/enemy_red.tres"
}
```

| Parameter | Type | Description |
|---|---|---|
| `type` | `string` | Godot resource class name |
| `properties` | `object` | Initial property values |
| `save_path` | `string` | Where to save the resource (optional — omit to create an embedded resource) |

**Response:**
```json
{
  "op": "resource_create",
  "type": "StandardMaterial3D",
  "resource_id": "@resource_id_789",
  "save_path": "assets/materials/enemy_red.tres",
  "result": "ok"
}
```

The `resource_id` is a temporary identifier used within Director operations to reference the resource before it has a saved path. You can use it in subsequent `node_set_property` calls:

```json
{
  "op": "node_set_property",
  "scene": "scenes/enemy.tscn",
  "node": "Enemy/MeshInstance3D",
  "property": "material_override",
  "value": "@resource_id_789"
}
```

### `resource_set`

Modify properties of an existing saved resource.

```json
{
  "op": "resource_set",
  "project_path": "/home/user/my-game",
  "path": "assets/materials/enemy_red.tres",
  "properties": {
    "albedo_color": [1.0, 0.0, 0.0, 1.0],
    "emission_enabled": true,
    "emission": [0.5, 0.0, 0.0, 1.0]
  }
}
```

**Response:**
```json
{
  "op": "resource_set",
  "path": "assets/materials/enemy_red.tres",
  "properties_set": ["albedo_color", "emission_enabled", "emission"],
  "result": "ok"
}
```

### `resource_get`

Read properties from an existing saved resource.

```json
{
  "op": "resource_get",
  "project_path": "/home/user/my-game",
  "path": "assets/materials/enemy_red.tres",
  "properties": ["albedo_color", "roughness", "metallic"]
}
```

**Response:**
```json
{
  "op": "resource_get",
  "path": "assets/materials/enemy_red.tres",
  "properties": {
    "albedo_color": [1.0, 0.0, 0.0, 1.0],
    "roughness": 0.7,
    "metallic": 0.0
  }
}
```

### `resource_list`

List resource files in the project.

```json
{
  "op": "resource_list",
  "project_path": "/home/user/my-game",
  "directory": "assets/materials",
  "type_filter": "StandardMaterial3D"
}
```

**Response:**
```json
{
  "resources": [
    { "path": "assets/materials/enemy_red.tres", "type": "StandardMaterial3D" },
    { "path": "assets/materials/player_blue.tres", "type": "StandardMaterial3D" },
    { "path": "assets/materials/ground.tres", "type": "StandardMaterial3D" }
  ]
}
```

## Common resource types

### Shape resources

Used by `CollisionShape3D` and `CollisionPolygon3D`:

```json
{ "type": "BoxShape3D", "properties": { "size": [2.0, 1.0, 2.0] } }
{ "type": "SphereShape3D", "properties": { "radius": 0.5 } }
{ "type": "CapsuleShape3D", "properties": { "radius": 0.4, "height": 1.8 } }
{ "type": "CylinderShape3D", "properties": { "radius": 0.5, "height": 2.0 } }
```

### Physics materials

Used by `PhysicsBody3D` for surface friction/bounce:

```json
{
  "type": "PhysicsMaterial",
  "properties": {
    "friction": 0.8,
    "rough": true,
    "bounce": 0.1,
    "absorbent": false
  }
}
```

### Standard materials

Used by `MeshInstance3D.material_override` or surface materials:

```json
{
  "type": "StandardMaterial3D",
  "properties": {
    "albedo_color": [0.2, 0.4, 0.8, 1.0],
    "roughness": 0.5,
    "metallic": 0.3,
    "normal_enabled": true
  }
}
```

### Curves and animation

```json
{ "type": "Curve", "properties": {} }
{ "type": "Animation", "properties": { "length": 1.0, "loop_mode": 1 } }
```

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Save resources when they will be reused.** An embedded resource (no `save_path`) exists only within the scene that uses it. A saved `.tres` file can be referenced by multiple scenes — better for shared materials, shapes, etc.

**Use `resource_list` before creating.** Check if the material/shape you need already exists. Reuse it rather than creating a duplicate.

**`resource_id` is only valid within the current operation session.** The `@resource_id_xxx` identifier returned by `resource_create` can be used in subsequent operations in the same session. After the session ends, use the `save_path` to reference the resource.
