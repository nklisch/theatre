<script setup>
import { data } from '../.vitepress/data/tools.data'

const resource_read = data.params['resource_read'] ?? []
const material_create = data.params['material_create'] ?? []
const shape_create = data.params['shape_create'] ?? []
const style_box_create = data.params['style_box_create'] ?? []
const resource_duplicate = data.params['resource_duplicate'] ?? []

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
  "resource_path": "assets/materials/enemy_red.tres",
  "depth": 1
}
```

<ParamTable :params="resource_read" />

**Response:**
```json
{
  "op": "resource_read",
  "resource_path": "assets/materials/enemy_red.tres",
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
  "resource_path": "assets/materials/enemy_red.tres",
  "material_type": "StandardMaterial3D",
  "properties": {
    "albedo_color": [0.8, 0.2, 0.2, 1.0],
    "roughness": 0.7,
    "metallic": 0.0
  }
}
```

For a `ShaderMaterial`, provide the `shader_path`:

```json
{
  "op": "material_create",
  "project_path": "/home/user/my-game",
  "resource_path": "assets/materials/glow.tres",
  "material_type": "ShaderMaterial",
  "shader_path": "shaders/glow.gdshader",
  "properties": {
    "shader_parameter/glow_color": [1.0, 0.8, 0.2, 1.0]
  }
}
```

<ParamTable :params="material_create" />

**Response:**
```json
{
  "op": "material_create",
  "resource_path": "assets/materials/enemy_red.tres",
  "result": "ok"
}
```

### `shape_create`

Create a collision shape resource. Can optionally save to disk and/or assign to a node.

```json
{
  "op": "shape_create",
  "project_path": "/home/user/my-game",
  "shape_type": "BoxShape3D",
  "shape_params": {
    "size": [2.0, 1.0, 2.0]
  },
  "save_path": "assets/shapes/platform_box.tres"
}
```

<ParamTable :params="shape_create" />

### `style_box_create`

Create a StyleBox resource (used by UI controls).

```json
{
  "op": "style_box_create",
  "project_path": "/home/user/my-game",
  "resource_path": "assets/ui/panel_style.tres",
  "style_type": "StyleBoxFlat",
  "properties": {
    "bg_color": [0.1, 0.1, 0.2, 1.0],
    "border_width_top": 2,
    "border_color": [0.5, 0.5, 1.0, 1.0]
  }
}
```

<ParamTable :params="style_box_create" />

### `resource_duplicate`

Duplicate an existing resource file. Optionally override properties on the copy and perform a deep copy of sub-resources.

```json
{
  "op": "resource_duplicate",
  "project_path": "/home/user/my-game",
  "source_path": "assets/materials/enemy_red.tres",
  "dest_path": "assets/materials/enemy_blue.tres",
  "property_overrides": {
    "albedo_color": [0.2, 0.2, 0.8, 1.0]
  },
  "deep_copy": false
}
```

<ParamTable :params="resource_duplicate" />

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
{ "shape_type": "BoxShape3D", "shape_params": { "size": [2.0, 1.0, 2.0] } }
{ "shape_type": "SphereShape3D", "shape_params": { "radius": 0.5 } }
{ "shape_type": "CapsuleShape3D", "shape_params": { "radius": 0.4, "height": 1.8 } }
{ "shape_type": "CylinderShape3D", "shape_params": { "radius": 0.5, "height": 2.0 } }
```

### Physics materials

Used by `PhysicsBody3D` for surface friction/bounce:

```json
{
  "material_type": "PhysicsMaterial",
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
  "material_type": "StandardMaterial3D",
  "properties": {
    "albedo_color": [0.2, 0.4, 0.8, 1.0],
    "roughness": 0.5,
    "metallic": 0.3,
    "normal_enabled": true
  }
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Save resources when they will be reused.** A saved `.tres` file can be referenced by multiple scenes — better for shared materials, shapes, etc.

**Read before modifying.** Use `resource_read` to check current property values before creating or duplicating resources.

**Use `resource_duplicate` to create variants.** Duplicate an existing material with `property_overrides` to change specific properties — faster than creating from scratch.
