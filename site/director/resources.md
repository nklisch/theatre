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

### `resource_read`

Read properties from an existing saved resource.

```json
{
  "op": "resource_read",
  "project_path": "/home/user/my-game",
  "path": "assets/materials/enemy_red.tres",
  "properties": ["albedo_color", "roughness", "metallic"]
}
```

**Response:**
```json
{
  "op": "resource_read",
  "path": "assets/materials/enemy_red.tres",
  "properties": {
    "albedo_color": [1.0, 0.0, 0.0, 1.0],
    "roughness": 0.7,
    "metallic": 0.0
  }
}
```

### `material_create`

Create a new material resource and save it to disk.

```json
{
  "op": "material_create",
  "project_path": "/home/user/my-game",
  "material_type": "StandardMaterial3D",
  "properties": {
    "albedo_color": [0.8, 0.2, 0.2, 1.0],
    "roughness": 0.7,
    "metallic": 0.0
  },
  "save_path": "assets/materials/enemy_red.tres"
}
```

**Response:**
```json
{
  "op": "material_create",
  "save_path": "assets/materials/enemy_red.tres",
  "result": "ok"
}
```

### `shape_create`

Create a collision shape resource.

```json
{
  "op": "shape_create",
  "project_path": "/home/user/my-game",
  "shape_type": "BoxShape3D",
  "properties": {
    "size": [2.0, 1.0, 2.0]
  },
  "save_path": "assets/shapes/platform_box.tres"
}
```

### `style_box_create`

Create a StyleBox resource (used by UI controls).

```json
{
  "op": "style_box_create",
  "project_path": "/home/user/my-game",
  "style_type": "StyleBoxFlat",
  "properties": {
    "bg_color": [0.1, 0.1, 0.2, 1.0],
    "border_width_top": 2,
    "border_color": [0.5, 0.5, 1.0, 1.0]
  },
  "save_path": "assets/ui/panel_style.tres"
}
```

### `resource_duplicate`

Duplicate an existing resource file.

```json
{
  "op": "resource_duplicate",
  "project_path": "/home/user/my-game",
  "source_path": "assets/materials/enemy_red.tres",
  "dest_path": "assets/materials/enemy_blue.tres"
}
```

**Response:**
```json
{
  "op": "resource_duplicate",
  "source_path": "assets/materials/enemy_red.tres",
  "dest_path": "assets/materials/enemy_blue.tres",
  "result": "ok"
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

**Save resources when they will be reused.** A saved `.tres` file can be referenced by multiple scenes — better for shared materials, shapes, etc.

**Read before modifying.** Use `resource_read` to check current property values before creating or duplicating resources.

**Use `resource_duplicate` to create variants.** Duplicate an existing material, then use `node_set_properties` to update specific properties — faster than creating from scratch.
