---
description: "Shader operations — create and modify shader materials, set shader parameters, and manage visual effects via Director."
---

<script setup>
import { data } from '../.vitepress/data/tools.data'

const visual_shader_create = data.params['visual_shader_create'] ?? []
</script>

# Shaders

Create VisualShader node graphs and set shader uniform values on materials.

Director provides one operation for shaders: `visual_shader_create`. To set shader uniform values on a `ShaderMaterial` node, use `node_set_properties` with the `shader_parameters` dictionary.

## Operations

### `visual_shader_create`

Create a new VisualShader resource (node-graph based shader).

```json
{
  "op": "visual_shader_create",
  "project_path": "/home/user/my-game",
  "resource_path": "assets/shaders/lava_effect.tres",
  "shader_mode": "spatial",
  "nodes": [
    { "node_id": 0, "type": "VisualShaderNodeOutput" }
  ]
}
```

<ParamTable :params="visual_shader_create" />

**Response:**
```json
{
  "op": "visual_shader_create",
  "resource_path": "assets/shaders/lava_effect.tres",
  "result": "ok"
}
```

After creation, assign the VisualShader to a ShaderMaterial node via `node_set_properties`.

## Setting shader uniform values

To set uniform values on a `ShaderMaterial` node, use `node_set_properties` with the `shader_parameters` dictionary:

```json
{
  "op": "node_set_properties",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "World/LavaMesh",
  "properties": {
    "shader_parameters": {
      "glow_intensity": 1.2,
      "flow_speed": 1.5,
      "water_color": [0.0, 0.3, 0.7, 0.9]
    }
  }
}
```

The `shader_parameters` property on a `ShaderMaterial` node holds all uniform values as a dictionary. Setting it via `node_set_properties` saves the values to the scene file.

## Tips

**Use `spatial_action` for live tuning.** For quick iteration while the game is running, use `spatial_action` to set `shader_parameters` on a `ShaderMaterial` node — no restart required. Use Director when you want to save the values to the scene file.

**Read current params with `scene_read`.** Use `scene_read` with `properties: true` to read the current `shader_parameters` dictionary from a node before modifying it.

**Texture uniforms use resource paths.** Assign textures as `"res://assets/textures/noise.png"` in the `shader_parameters` dictionary — Godot resolves the resource path.
