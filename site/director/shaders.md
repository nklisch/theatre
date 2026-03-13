<script setup>
const messages0 = [
  { role: 'human', text: `The lava material needs to glow more intensely and move faster. Update the shader params.` },
  { role: 'agent', text: `Current glow_intensity: 0.3. Let me also check flow_speed.` },
  { role: 'agent', text: `flow_speed: 0.5. Updating both values.` },
  { role: 'agent', text: `Done. glow_intensity: 0.3 → 1.2, flow_speed: 0.5 → 1.5. The lava should appear much brighter and flow at 3x the previous speed.` },
]
</script>

# Shaders

Set shader code and modify shader parameters on materials.

Director can assign shader code to a `ShaderMaterial` and get/set the uniform parameters that control shader appearance at runtime.

## Operations

### `shader_set`

Assign GLSL shader code to a material. Creates or replaces the `Shader` resource on a `ShaderMaterial`.

```json
{
  "op": "shader_set",
  "project_path": "/home/user/my-game",
  "material_path": "assets/materials/water.tres",
  "shader_code": "shader_type spatial;\nuniform float wave_height = 0.5;\nuniform vec4 water_color : source_color = vec4(0.0, 0.4, 0.8, 0.8);\n\nvoid vertex() {\n  VERTEX.y += sin(TIME + VERTEX.x * 2.0) * wave_height;\n}\n\nvoid fragment() {\n  ALBEDO = water_color.rgb;\n  ALPHA = water_color.a;\n}"
}
```

| Parameter | Type | Description |
|---|---|---|
| `material_path` | `string` | Path to a `ShaderMaterial` resource (.tres) |
| `shader_code` | `string` | Complete GLSL shader code |
| `save_shader_path` | `string` | If set, saves the Shader as a `.gdshader` file at this path |

**Response:**
```json
{
  "op": "shader_set",
  "material_path": "assets/materials/water.tres",
  "uniforms_found": ["wave_height", "water_color"],
  "result": "ok"
}
```

The response lists the uniforms detected in the shader code, which you can then set with `shader_set_param`.

### `shader_get_param`

Read the current value of a shader uniform.

```json
{
  "op": "shader_get_param",
  "project_path": "/home/user/my-game",
  "material_path": "assets/materials/water.tres",
  "param": "wave_height"
}
```

**Response:**
```json
{
  "op": "shader_get_param",
  "material_path": "assets/materials/water.tres",
  "param": "wave_height",
  "value": 0.5
}
```

### `shader_set_param`

Set the value of a shader uniform.

```json
{
  "op": "shader_set_param",
  "project_path": "/home/user/my-game",
  "material_path": "assets/materials/water.tres",
  "param": "wave_height",
  "value": 1.2
}
```

Multiple params at once:

```json
{
  "op": "shader_set_param",
  "project_path": "/home/user/my-game",
  "material_path": "assets/materials/water.tres",
  "params": {
    "wave_height": 1.2,
    "water_color": [0.0, 0.3, 0.7, 0.9]
  }
}
```

**Response:**
```json
{
  "op": "shader_set_param",
  "material_path": "assets/materials/water.tres",
  "params_set": ["wave_height", "water_color"],
  "result": "ok"
}
```

## Uniform type mapping

| Shader uniform type | JSON value format |
|---|---|
| `float` | `number` |
| `int` | `integer` |
| `bool` | `boolean` |
| `vec2` | `[x, y]` |
| `vec3` | `[x, y, z]` |
| `vec4` / `color` | `[r, g, b, a]` |
| `sampler2D` | `"res://path/to/texture.png"` (resource path) |

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Read params before changing them.** Always call `shader_get_param` first to understand current values and make informed adjustments.

**Use `shader_set` for code changes, `shader_set_param` for tuning.** Write the shader logic once (with `shader_set`), then iterate on values with `shader_set_param` without recompiling the shader.

**Texture uniforms use resource paths.** Assign textures as `"res://assets/textures/noise.png"` — Godot will resolve the resource.

**Test at runtime with `spatial_action`.** For quick iteration, use `spatial_action` to set shader params on a `ShaderMaterial` attached to a running `MeshInstance3D` node — no restart required. Use Director when you want to save the values.
