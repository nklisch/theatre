# Design: Milestone 10 — Resource Inspection

## Overview

M10 adds a `resources` category to `spatial_inspect`, allowing agents to inspect loaded
resources on nodes — meshes, materials, animations, collision shapes, navigation agents,
and shader parameters. This completes the deep inspection capability and enables agents
to diagnose visual, animation, and collision issues without reading source files.

**Depends on:** M2 (spatial_inspect and scene_tree are working)

**User Story:** S4.3 — Resource Inspection (P2)

**Exit Criteria:**
- Agent calls `spatial_inspect(node: "enemies/scout_02", include: ["resources"])` and gets
  mesh info, collision shape dimensions, current animation state, shader parameters.
- Agent can diagnose "why is this invisible" (no mesh/material), "why is it T-posing"
  (no animation), "what's the collider shape" problems.
- Inline resources are distinguished from file-based resources.
- Resource data appears only when `"resources"` is in the `include` list.
- Token budget is respected.

---

## Architecture Decision: Resources as an Inspect Category

**Decision:** Add `Resources` as an eighth `InspectCategory` variant. The addon collects
resource data by walking the inspected node's children for known resource-bearing types.
The server passes through the data with no additional processing (unlike `spatial_context`
which requires bearing computation).

**Rationale:**
- The `spatial_inspect` tool already supports selective `include` categories. Adding
  `Resources` follows the established pattern exactly.
- Resource data is inherently node-local — it doesn't need server-side spatial reasoning,
  so the addon collects and the server passes through.
- Walking children (not the node itself) catches resources attached to child nodes like
  `CollisionShape3D`, `MeshInstance3D`, and `AnimationPlayer`, which is how Godot scenes
  are structured.

**Consequence:** The protocol gains one new enum variant and one new struct. The collector
gains one new collection function. The MCP handler gains one new include string. Minimal
surface area change.

---

## Architecture Decision: Resource Collection Strategy

**Decision:** Collect resources from both the inspected node and its immediate children.
Walk the node's direct children looking for known resource-bearing classes. Do NOT recurse
deeper — the agent can inspect deeper children explicitly.

**Rationale:**
- A typical Godot node like `CharacterBody3D` has direct children: `CollisionShape3D`,
  `MeshInstance3D`, `AnimationPlayer`, `NavigationAgent3D`, `Sprite2D`, etc.
- Recursing deeper would include resources from sub-scenes and nested compositions,
  producing excessive data.
- One-level child walk matches how `collect_inspect_children` already works.

**Resource types collected:**

| Child Class | Resource Data |
|---|---|
| `MeshInstance3D` | mesh resource path/type, surface count, material overrides |
| `MeshInstance2D` | mesh resource path/type |
| `CollisionShape3D` | shape type, dimensions, inline vs file |
| `CollisionShape2D` | shape type, dimensions, inline vs file |
| `AnimationPlayer` | current animation, available anims, position, length, looping |
| `AnimationTree` | active flag, current state (if StateMachine) |
| `NavigationAgent3D` | target position, path postprocessing, avoidance |
| `NavigationAgent2D` | target position, path postprocessing, avoidance |
| `Sprite2D` | texture resource path, region, flip state |
| `Sprite3D` | texture resource path, flip state |
| `GPUParticles3D` / `GPUParticles2D` | emitting flag, amount, process material path |

Additionally, the inspected node itself is checked for:
- Shader material on any `CanvasItem` or `GeometryInstance3D`: exported shader params
- `ShaderMaterial` overrides: uniform values

---

## Architecture Decision: Shader Parameter Collection

**Decision:** Collect shader parameters from `ShaderMaterial` found on the inspected node
or its `MeshInstance3D` children. Only collect uniforms that have non-default values or
are listed in the material's shader parameter list.

**Rationale:**
- Shader parameters are the primary way developers control visual effects (damage flash,
  outline color, dissolve progress). These are critical for debugging visual issues.
- Godot's `ShaderMaterial` exposes uniforms via `get_shader_parameter()` and the shader's
  parameter list is available via `shader.get_shader_uniform_list()`.
- Collecting all uniforms is feasible because typical game shaders have 5-20 parameters.

---

## Implementation Units

### Unit 1: Protocol Types — `InspectResources` struct

**File:** `crates/spectator-protocol/src/query.rs`

```rust
/// Add Resources variant to InspectCategory enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectCategory {
    Transform,
    Physics,
    State,
    Children,
    Signals,
    Script,
    SpatialContext,
    Resources,  // NEW
}

/// Resource data collected from a node and its children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResources {
    /// Mesh data from MeshInstance3D/MeshInstance2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<MeshResourceData>,

    /// Collision shape data from CollisionShape3D/CollisionShape2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collision_shapes: Vec<CollisionShapeData>,

    /// Animation player data from AnimationPlayer children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animation_players: Vec<AnimationPlayerData>,

    /// Navigation agent data from NavigationAgent3D/2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub navigation_agents: Vec<NavigationAgentData>,

    /// Sprite data from Sprite2D/Sprite3D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sprites: Vec<SpriteData>,

    /// Particle system data from GPUParticles3D/2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub particles: Vec<ParticleData>,

    /// Shader parameters from ShaderMaterial on the node or mesh children.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub shader_params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshResourceData {
    /// Name of the child node holding the mesh.
    pub child: String,
    /// Resource path (e.g. "res://models/scout.tres") or null if inline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Mesh class name (e.g. "ArrayMesh", "BoxMesh", "SphereMesh").
    #[serde(rename = "type")]
    pub mesh_type: String,
    /// Number of surfaces in the mesh.
    pub surface_count: u32,
    /// Material overrides per surface index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_overrides: Vec<MaterialOverrideData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialOverrideData {
    /// Surface index.
    pub surface: u32,
    /// Material resource path or null if inline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Material class name (e.g. "StandardMaterial3D", "ShaderMaterial").
    #[serde(rename = "type")]
    pub material_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionShapeData {
    /// Name of the child node holding the shape.
    pub child: String,
    /// Shape class name (e.g. "CapsuleShape3D", "BoxShape3D", "CircleShape2D").
    #[serde(rename = "type")]
    pub shape_type: String,
    /// Shape dimensions as key-value pairs (e.g. {"radius": 0.5, "height": 1.8}).
    pub dimensions: serde_json::Map<String, serde_json::Value>,
    /// Whether the shape is an inline resource (true) or loaded from a file (false).
    pub inline: bool,
    /// Disabled flag from the CollisionShape node.
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationPlayerData {
    /// Name of the AnimationPlayer child node.
    pub child: String,
    /// Currently playing animation name (null if stopped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_animation: Option<String>,
    /// List of available animation names.
    pub animations: Vec<String>,
    /// Current playback position in seconds.
    pub position_sec: f64,
    /// Length of current animation in seconds (0.0 if stopped).
    pub length_sec: f64,
    /// Whether the current animation loops.
    pub looping: bool,
    /// Whether the player is currently playing.
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationAgentData {
    /// Name of the NavigationAgent child node.
    pub child: String,
    /// Target position the agent is navigating toward.
    pub target_position: Vec<f64>,
    /// Whether the target has been reached.
    pub target_reached: bool,
    /// Remaining distance to the target.
    pub distance_remaining: f64,
    /// Path postprocessing mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_postprocessing: Option<String>,
    /// Whether avoidance is enabled.
    pub avoidance_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteData {
    /// Name of the Sprite child node.
    pub child: String,
    /// Texture resource path (null if no texture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// Whether the sprite is visible.
    pub visible: bool,
    /// Flip flags.
    pub flip_h: bool,
    pub flip_v: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleData {
    /// Name of the particle system child node.
    pub child: String,
    /// Whether particles are currently emitting.
    pub emitting: bool,
    /// Number of particles.
    pub amount: i32,
    /// Process material resource path (null if inline/none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_material: Option<String>,
}

/// Add resources field to NodeInspectResponse
// In the existing NodeInspectResponse struct, add:
//   #[serde(skip_serializing_if = "Option::is_none")]
//   pub resources: Option<InspectResources>,
```

**Implementation Notes:**
- `resource_path()` helper: given a Godot `Resource`, return `Some(path)` if
  `resource.get_path()` is non-empty, else `None` (inline resource).
- All `serde_json::Map` fields use the existing `variant_to_json` converter.
- Shape dimensions vary by type — see Unit 3 for extraction logic.

**Acceptance Criteria:**
- [ ] `InspectCategory::Resources` variant exists and deserializes from `"resources"`
- [ ] All resource data structs serialize/deserialize round-trip correctly
- [ ] `NodeInspectResponse` has an optional `resources` field
- [ ] Empty collections are omitted from serialized output (skip_serializing_if)

---

### Unit 2: MCP Handler — parse `"resources"` include

**File:** `crates/spectator-server/src/mcp/inspect.rs`

```rust
// Update default_include to NOT include "resources" by default
// (resources is opt-in to save tokens)
fn default_include() -> Vec<String> {
    vec![
        "transform".into(),
        "physics".into(),
        "state".into(),
        "children".into(),
        "signals".into(),
        "script".into(),
        "spatial_context".into(),
        // NOTE: "resources" deliberately excluded from defaults
    ]
}

// Update parse_include lookup table to include Resources
pub fn parse_include(strings: &[String]) -> Result<Vec<InspectCategory>, McpError> {
    super::parse_enum_list(strings, "include category", &[
        ("transform", InspectCategory::Transform),
        ("physics", InspectCategory::Physics),
        ("state", InspectCategory::State),
        ("children", InspectCategory::Children),
        ("signals", InspectCategory::Signals),
        ("script", InspectCategory::Script),
        ("spatial_context", InspectCategory::SpatialContext),
        ("resources", InspectCategory::Resources),  // NEW
    ])
}
```

**File:** `crates/spectator-server/src/mcp/mod.rs`

The `spatial_inspect` handler in `mod.rs` needs no changes — it already passes through
the full `NodeInspectResponse` via `serde_json::to_value`. The new `resources` field
will be serialized automatically when present.

Update the tool description string to mention resources:

```rust
#[tool(description = "Deep inspection of a single node. Returns transform, physics, \
    state, children, signals, script, spatial context, and resources. Use the 'include' \
    parameter to select specific categories and reduce token usage. Default includes all \
    categories except 'resources' (opt-in).")]
```

**Implementation Notes:**
- Resources is deliberately NOT in the default include list. Resource data can be
  200-500 tokens per node. The agent explicitly opts in when diagnosing visual/animation
  issues.
- The tool description update helps the agent discover the `resources` option.

**Acceptance Criteria:**
- [ ] `"resources"` is accepted in the include list without error
- [ ] `"resources"` is NOT included by default
- [ ] Unknown include values still produce an error
- [ ] Tool description mentions resources

---

### Unit 3: GDExtension — Resource Collection in Collector

**File:** `crates/spectator-godot/src/collector.rs`

Add `collect_inspect_resources` method and supporting helpers:

```rust
use godot::classes::{
    AnimationPlayer, CollisionShape3D, CollisionShape2D, MeshInstance3D,
    NavigationAgent3D, NavigationAgent2D, Sprite2D, Sprite3D,
    GpuParticles3D, GpuParticles2D, ShaderMaterial, Mesh,
    BoxShape3D, CapsuleShape3D, SphereShape3D, CylinderShape3D,
    WorldBoundaryShape3D, ConvexPolygonShape3D, ConcavePolygonShape3D,
    CircleShape2D, RectangleShape2D, CapsuleShape2D, SegmentShape2D,
    GeometryInstance3D, MeshInstance2D, AnimationTree,
};
use spectator_protocol::query::{
    InspectResources, MeshResourceData, MaterialOverrideData,
    CollisionShapeData, AnimationPlayerData, NavigationAgentData,
    SpriteData, ParticleData,
};

impl SpectatorCollector {
    /// Collect resource data from the node and its immediate children.
    fn collect_inspect_resources(&self, node: &Gd<Node>) -> InspectResources {
        let mut resources = InspectResources {
            meshes: Vec::new(),
            collision_shapes: Vec::new(),
            animation_players: Vec::new(),
            navigation_agents: Vec::new(),
            sprites: Vec::new(),
            particles: Vec::new(),
            shader_params: serde_json::Map::new(),
        };

        // Collect from the node itself (shader params if it has a material)
        self.collect_shader_params_from_node(node, &mut resources.shader_params);

        // Walk immediate children
        for i in 0..node.get_child_count() {
            let Some(child) = node.get_child(i) else { continue };
            let child_name = child.get_name().to_string();
            let child_class = child.get_class().to_string();

            match child_class.as_str() {
                "MeshInstance3D" => {
                    if let Ok(mi) = child.clone().try_cast::<MeshInstance3D>() {
                        resources.meshes.push(self.collect_mesh_3d(&mi, &child_name));
                        self.collect_shader_params_from_mesh_3d(
                            &mi, &mut resources.shader_params,
                        );
                    }
                }
                "MeshInstance2D" => {
                    if let Ok(mi) = child.clone().try_cast::<MeshInstance2D>() {
                        resources.meshes.push(self.collect_mesh_2d(&mi, &child_name));
                    }
                }
                "CollisionShape3D" => {
                    if let Ok(cs) = child.clone().try_cast::<CollisionShape3D>() {
                        resources.collision_shapes.push(
                            self.collect_collision_shape_3d(&cs, &child_name),
                        );
                    }
                }
                "CollisionShape2D" => {
                    if let Ok(cs) = child.clone().try_cast::<CollisionShape2D>() {
                        resources.collision_shapes.push(
                            self.collect_collision_shape_2d(&cs, &child_name),
                        );
                    }
                }
                "AnimationPlayer" => {
                    if let Ok(ap) = child.clone().try_cast::<AnimationPlayer>() {
                        resources.animation_players.push(
                            self.collect_animation_player(&ap, &child_name),
                        );
                    }
                }
                "NavigationAgent3D" => {
                    if let Ok(na) = child.clone().try_cast::<NavigationAgent3D>() {
                        resources.navigation_agents.push(
                            self.collect_nav_agent_3d(&na, &child_name),
                        );
                    }
                }
                "NavigationAgent2D" => {
                    if let Ok(na) = child.clone().try_cast::<NavigationAgent2D>() {
                        resources.navigation_agents.push(
                            self.collect_nav_agent_2d(&na, &child_name),
                        );
                    }
                }
                "Sprite2D" => {
                    if let Ok(sp) = child.clone().try_cast::<Sprite2D>() {
                        resources.sprites.push(self.collect_sprite_2d(&sp, &child_name));
                    }
                }
                "Sprite3D" => {
                    if let Ok(sp) = child.clone().try_cast::<Sprite3D>() {
                        resources.sprites.push(self.collect_sprite_3d(&sp, &child_name));
                    }
                }
                "GPUParticles3D" => {
                    if let Ok(p) = child.clone().try_cast::<GpuParticles3D>() {
                        resources.particles.push(
                            self.collect_particles_3d(&p, &child_name),
                        );
                    }
                }
                "GPUParticles2D" => {
                    if let Ok(p) = child.clone().try_cast::<GpuParticles2D>() {
                        resources.particles.push(
                            self.collect_particles_2d(&p, &child_name),
                        );
                    }
                }
                _ => {}
            }
        }

        resources
    }
}
```

**Helper: `resource_path`**

```rust
/// Extract file path from a Godot Resource, or None if inline.
fn resource_path(res: &Gd<Resource>) -> Option<String> {
    let path = res.get_path().to_string();
    if path.is_empty() { None } else { Some(path) }
}
```

**Helper: `collect_mesh_3d`**

```rust
fn collect_mesh_3d(&self, mi: &Gd<MeshInstance3D>, child_name: &str) -> MeshResourceData {
    let mesh_opt = mi.get_mesh();
    let (resource, mesh_type, surface_count) = match &mesh_opt {
        Some(mesh) => {
            let res: Gd<Resource> = mesh.clone().upcast();
            (resource_path(&res), mesh.get_class().to_string(), mesh.get_surface_count() as u32)
        }
        None => (None, "None".into(), 0),
    };

    let mut material_overrides = Vec::new();
    if let Some(mesh) = &mesh_opt {
        for i in 0..mesh.get_surface_count() {
            if let Some(mat) = mi.get_surface_override_material(i) {
                let mat_res: Gd<Resource> = mat.clone().upcast();
                material_overrides.push(MaterialOverrideData {
                    surface: i as u32,
                    resource: resource_path(&mat_res),
                    material_type: mat.get_class().to_string(),
                });
            }
        }
    }

    MeshResourceData {
        child: child_name.into(),
        resource,
        mesh_type,
        surface_count,
        material_overrides,
    }
}
```

**Helper: `collect_collision_shape_3d`**

```rust
fn collect_collision_shape_3d(
    &self,
    cs: &Gd<CollisionShape3D>,
    child_name: &str,
) -> CollisionShapeData {
    let disabled = cs.is_disabled();
    let shape_opt = cs.get_shape();

    let (shape_type, dimensions, inline) = match &shape_opt {
        Some(shape) => {
            let res: Gd<Resource> = shape.clone().upcast();
            let inline = resource_path(&res).is_none();
            let shape_type = shape.get_class().to_string();
            let dims = self.extract_shape_dimensions_3d(shape);
            (shape_type, dims, inline)
        }
        None => ("None".into(), serde_json::Map::new(), true),
    };

    CollisionShapeData {
        child: child_name.into(),
        shape_type,
        dimensions,
        inline,
        disabled,
    }
}

fn extract_shape_dimensions_3d(
    &self,
    shape: &Gd<godot::classes::Shape3D>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut dims = serde_json::Map::new();
    if let Ok(cap) = shape.clone().try_cast::<CapsuleShape3D>() {
        dims.insert("radius".into(), json!(cap.get_radius()));
        dims.insert("height".into(), json!(cap.get_height()));
    } else if let Ok(box_s) = shape.clone().try_cast::<BoxShape3D>() {
        let size = box_s.get_size();
        dims.insert("size".into(), json!([size.x, size.y, size.z]));
    } else if let Ok(sphere) = shape.clone().try_cast::<SphereShape3D>() {
        dims.insert("radius".into(), json!(sphere.get_radius()));
    } else if let Ok(cyl) = shape.clone().try_cast::<CylinderShape3D>() {
        dims.insert("radius".into(), json!(cyl.get_radius()));
        dims.insert("height".into(), json!(cyl.get_height()));
    }
    // ConvexPolygon, ConcavePolygon, WorldBoundary: no simple dimensions
    dims
}
```

**Helper: `collect_collision_shape_2d`**

```rust
fn collect_collision_shape_2d(
    &self,
    cs: &Gd<CollisionShape2D>,
    child_name: &str,
) -> CollisionShapeData {
    let disabled = cs.is_disabled();
    let shape_opt = cs.get_shape();

    let (shape_type, dimensions, inline) = match &shape_opt {
        Some(shape) => {
            let res: Gd<Resource> = shape.clone().upcast();
            let inline = resource_path(&res).is_none();
            let shape_type = shape.get_class().to_string();
            let dims = self.extract_shape_dimensions_2d(shape);
            (shape_type, dims, inline)
        }
        None => ("None".into(), serde_json::Map::new(), true),
    };

    CollisionShapeData {
        child: child_name.into(),
        shape_type,
        dimensions,
        inline,
        disabled,
    }
}

fn extract_shape_dimensions_2d(
    &self,
    shape: &Gd<godot::classes::Shape2D>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut dims = serde_json::Map::new();
    if let Ok(circle) = shape.clone().try_cast::<CircleShape2D>() {
        dims.insert("radius".into(), json!(circle.get_radius()));
    } else if let Ok(rect) = shape.clone().try_cast::<RectangleShape2D>() {
        let size = rect.get_size();
        dims.insert("size".into(), json!([size.x, size.y]));
    } else if let Ok(cap) = shape.clone().try_cast::<CapsuleShape2D>() {
        dims.insert("radius".into(), json!(cap.get_radius()));
        dims.insert("height".into(), json!(cap.get_height()));
    }
    dims
}
```

**Helper: `collect_animation_player`**

```rust
fn collect_animation_player(
    &self,
    ap: &Gd<AnimationPlayer>,
    child_name: &str,
) -> AnimationPlayerData {
    let current = if ap.is_playing() {
        let name = ap.get_current_animation().to_string();
        if name.is_empty() { None } else { Some(name) }
    } else {
        None
    };

    let animations: Vec<String> = ap
        .get_animation_list()
        .as_slice()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let length_sec = current
        .as_ref()
        .and_then(|name| {
            ap.get_animation(&StringName::from(name.as_str()))
                .map(|anim| anim.get_length() as f64)
        })
        .unwrap_or(0.0);

    let looping = current
        .as_ref()
        .and_then(|name| {
            ap.get_animation(&StringName::from(name.as_str()))
                .map(|anim| anim.get_loop_mode() != godot::classes::animation::LoopMode::NONE)
        })
        .unwrap_or(false);

    AnimationPlayerData {
        child: child_name.into(),
        current_animation: current,
        animations,
        position_sec: ap.get_current_animation_position() as f64,
        length_sec,
        looping,
        playing: ap.is_playing(),
    }
}
```

**Helper: `collect_nav_agent_3d`**

```rust
fn collect_nav_agent_3d(
    &self,
    na: &Gd<NavigationAgent3D>,
    child_name: &str,
) -> NavigationAgentData {
    let target = na.get_target_position();
    NavigationAgentData {
        child: child_name.into(),
        target_position: vec![target.x as f64, target.y as f64, target.z as f64],
        target_reached: na.is_target_reached(),
        distance_remaining: na.distance_to_target() as f64,
        path_postprocessing: Some(format!("{:?}", na.get_path_postprocessing())),
        avoidance_enabled: na.get_avoidance_enabled(),
    }
}
```

**Helper: `collect_nav_agent_2d`**

```rust
fn collect_nav_agent_2d(
    &self,
    na: &Gd<NavigationAgent2D>,
    child_name: &str,
) -> NavigationAgentData {
    let target = na.get_target_position();
    NavigationAgentData {
        child: child_name.into(),
        target_position: vec![target.x as f64, target.y as f64],
        target_reached: na.is_target_reached(),
        distance_remaining: na.distance_to_target() as f64,
        path_postprocessing: Some(format!("{:?}", na.get_path_postprocessing())),
        avoidance_enabled: na.get_avoidance_enabled(),
    }
}
```

**Helper: `collect_sprite_2d` / `collect_sprite_3d`**

```rust
fn collect_sprite_2d(&self, sp: &Gd<Sprite2D>, child_name: &str) -> SpriteData {
    let texture = sp.get_texture().map(|t| {
        let res: Gd<Resource> = t.upcast();
        resource_path(&res).unwrap_or_else(|| "inline".into())
    });
    SpriteData {
        child: child_name.into(),
        texture,
        visible: sp.is_visible(),
        flip_h: sp.is_flipped_h(),
        flip_v: sp.is_flipped_v(),
    }
}

fn collect_sprite_3d(&self, sp: &Gd<Sprite3D>, child_name: &str) -> SpriteData {
    let texture = sp.get_texture().map(|t| {
        let res: Gd<Resource> = t.upcast();
        resource_path(&res).unwrap_or_else(|| "inline".into())
    });
    SpriteData {
        child: child_name.into(),
        texture,
        visible: sp.is_visible(),
        flip_h: sp.is_flipped_h(),
        flip_v: sp.is_flipped_v(),
    }
}
```

**Helper: `collect_particles_3d` / `collect_particles_2d`**

```rust
fn collect_particles_3d(&self, p: &Gd<GpuParticles3D>, child_name: &str) -> ParticleData {
    let process_material = p.get_process_material().and_then(|m| {
        let res: Gd<Resource> = m.upcast();
        resource_path(&res)
    });
    ParticleData {
        child: child_name.into(),
        emitting: p.is_emitting(),
        amount: p.get_amount(),
        process_material,
    }
}

fn collect_particles_2d(&self, p: &Gd<GpuParticles2D>, child_name: &str) -> ParticleData {
    let process_material = p.get_process_material().and_then(|m| {
        let res: Gd<Resource> = m.upcast();
        resource_path(&res)
    });
    ParticleData {
        child: child_name.into(),
        emitting: p.is_emitting(),
        amount: p.get_amount(),
        process_material,
    }
}
```

**Helper: `collect_shader_params_from_node` / `collect_shader_params_from_mesh_3d`**

```rust
fn collect_shader_params_from_node(
    &self,
    node: &Gd<Node>,
    params: &mut serde_json::Map<String, serde_json::Value>,
) {
    // Check if the node is a GeometryInstance3D with material override
    if let Ok(gi) = node.clone().try_cast::<GeometryInstance3D>() {
        if let Some(mat) = gi.get_material_override() {
            self.extract_shader_params(&mat.upcast(), params);
        }
    }
    // Check CanvasItem (2D nodes) for material
    if let Ok(ci) = node.clone().try_cast::<godot::classes::CanvasItem>() {
        if let Some(mat) = ci.get_material() {
            self.extract_shader_params(&mat.upcast(), params);
        }
    }
}

fn collect_shader_params_from_mesh_3d(
    &self,
    mi: &Gd<MeshInstance3D>,
    params: &mut serde_json::Map<String, serde_json::Value>,
) {
    // Check material overrides on the MeshInstance3D
    if let Some(mesh) = mi.get_mesh() {
        for i in 0..mesh.get_surface_count() {
            if let Some(mat) = mi.get_surface_override_material(i) {
                let mat_res: Gd<godot::classes::Material> = mat;
                self.extract_shader_params(&mat_res.upcast(), params);
            }
        }
    }
}

fn extract_shader_params(
    &self,
    material: &Gd<Resource>,
    params: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Ok(shader_mat) = material.clone().try_cast::<ShaderMaterial>() {
        if let Some(shader) = shader_mat.get_shader() {
            // Get shader uniform list from the Shader resource
            let uniform_list = shader.get_shader_uniform_list(false);
            for entry in uniform_list.iter_shared() {
                let dict = entry.to::<Dictionary>();
                let Some(name_var) = dict.get("name") else { continue };
                let name = name_var.to::<GString>().to_string();
                let value = shader_mat.get_shader_parameter(&StringName::from(&name));
                if let Some(json_val) = variant_to_json(&value) {
                    params.insert(name, json_val);
                }
            }
        }
    }
}
```

**Wire into `inspect_node`:**

In the existing `inspect_node` method, add a new match arm:

```rust
InspectCategory::Resources => {
    response.resources = Some(self.collect_inspect_resources(&node));
}
```

**Implementation Notes:**
- All `try_cast` calls follow the existing pattern in collector.rs.
- `variant_to_json` is already implemented and handles Color, Vector2/3, arrays, etc.
- The `json!()` macro is from `serde_json` — already imported.
- `resource_path` helper is a free function (not a method) since it's stateless.
- The `get_shader_uniform_list(false)` call passes `false` for `get_groups` — we want
  a flat list of uniforms, not grouped.
- For AnimationPlayer, `get_animation_list()` returns a `PackedStringArray`.

**Acceptance Criteria:**
- [ ] `collect_inspect_resources` returns data for MeshInstance3D children
- [ ] Material overrides are collected per surface
- [ ] Collision shapes report type, dimensions, inline flag, disabled flag
- [ ] AnimationPlayer reports current animation, available list, position, looping
- [ ] NavigationAgent reports target, distance remaining, avoidance
- [ ] Sprite data includes texture path, visibility, flip flags
- [ ] Particle data includes emitting state, amount, process material
- [ ] Shader parameters are collected from ShaderMaterial on node or mesh children
- [ ] Inline vs file resources are correctly distinguished
- [ ] 2D shapes (CircleShape2D, RectangleShape2D, CapsuleShape2D) have correct dimensions
- [ ] 3D shapes (CapsuleShape3D, BoxShape3D, SphereShape3D, CylinderShape3D) have correct dimensions

---

### Unit 4: Query Handler — `get_node_resources` method dispatch

**File:** `crates/spectator-godot/src/query_handler.rs`

No changes needed. The existing `get_node_inspect` method already dispatches to
`collector.inspect_node()`, which now handles `InspectCategory::Resources` via the new
match arm added in Unit 3. The protocol types (Unit 1) serialize the `resources` field
automatically.

**Acceptance Criteria:**
- [ ] `get_node_inspect` with `include: ["resources"]` returns resource data in the response
- [ ] `get_node_inspect` without `"resources"` in include returns no resource data

---

### Unit 5: Integration Test — Resource Inspection via TCP Mock

**File:** `crates/spectator-server/tests/tcp_mock.rs`

Add a test that exercises the resource inspection path through the mock TCP layer.

```rust
#[tokio::test]
async fn test_inspect_resources_passthrough() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let params: GetNodeInspectParams =
                serde_json::from_value(params.clone()).unwrap();
            // Verify "resources" was passed through
            assert!(params.include.contains(&InspectCategory::Resources));

            Ok(json!({
                "path": "enemies/scout_02",
                "class": "CharacterBody3D",
                "instance_id": 12345,
                "resources": {
                    "meshes": [{
                        "child": "MeshInstance3D",
                        "resource": "res://models/scout.tres",
                        "type": "ArrayMesh",
                        "surface_count": 3,
                        "material_overrides": [{
                            "surface": 0,
                            "resource": "res://materials/enemy_skin.tres",
                            "type": "StandardMaterial3D"
                        }]
                    }],
                    "collision_shapes": [{
                        "child": "CollisionShape3D",
                        "type": "CapsuleShape3D",
                        "dimensions": {"radius": 0.5, "height": 1.8},
                        "inline": true,
                        "disabled": false
                    }],
                    "animation_players": [{
                        "child": "AnimationPlayer",
                        "current_animation": "patrol_walk",
                        "animations": ["idle", "patrol_walk", "run", "attack"],
                        "position_sec": 0.8,
                        "length_sec": 1.2,
                        "looping": true,
                        "playing": true
                    }],
                    "navigation_agents": [],
                    "sprites": [],
                    "particles": [],
                    "shader_params": {
                        "outline_color": [1.0, 0.0, 0.0, 1.0],
                        "damage_flash_intensity": 0.0
                    }
                }
            }))
        } else if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("spatial_inspect", json!({
            "node": "enemies/scout_02",
            "include": ["resources"]
        }))
        .await
        .unwrap();

    // Verify resource data is present in the response
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed["resources"]["meshes"].is_array());
    assert_eq!(parsed["resources"]["meshes"][0]["type"], "ArrayMesh");
    assert_eq!(parsed["resources"]["collision_shapes"][0]["dimensions"]["radius"], 0.5);
    assert_eq!(parsed["resources"]["animation_players"][0]["playing"], true);
    assert_eq!(parsed["resources"]["shader_params"]["damage_flash_intensity"], 0.0);
}

#[tokio::test]
async fn test_inspect_default_excludes_resources() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let params: GetNodeInspectParams =
                serde_json::from_value(params.clone()).unwrap();
            // Verify "resources" is NOT in defaults
            assert!(!params.include.contains(&InspectCategory::Resources));

            Ok(json!({
                "path": "enemies/scout_02",
                "class": "CharacterBody3D",
                "instance_id": 12345
            }))
        } else if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("spatial_inspect", json!({
            "node": "enemies/scout_02"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed.get("resources").is_none());
}
```

**Acceptance Criteria:**
- [ ] Test verifies resource data passes through from addon to MCP response
- [ ] Test verifies default include does NOT request resources
- [ ] Test verifies individual resource fields (meshes, collision shapes, animations, shader params)

---

## Implementation Order

1. **Unit 1: Protocol Types** — `InspectResources` and sub-structs in `spectator-protocol`
   - No dependencies. Defines the wire format.
2. **Unit 2: MCP Handler** — parse `"resources"` include in `spectator-server`
   - Depends on Unit 1 (needs `InspectCategory::Resources`).
3. **Unit 3: GDExtension Collector** — `collect_inspect_resources` in `spectator-godot`
   - Depends on Unit 1 (needs `InspectResources` struct).
4. **Unit 4: Query Handler** — no-op, verify existing dispatch works
   - Depends on Unit 3.
5. **Unit 5: Integration Tests** — TCP mock tests
   - Depends on Units 1, 2, 3.

Units 2 and 3 can proceed in parallel after Unit 1.

---

## Testing

### Unit Tests: `crates/spectator-protocol/src/query.rs`

```rust
#[test]
fn inspect_resources_round_trip() {
    let resources = InspectResources {
        meshes: vec![MeshResourceData {
            child: "Mesh".into(),
            resource: Some("res://models/scout.tres".into()),
            mesh_type: "ArrayMesh".into(),
            surface_count: 3,
            material_overrides: vec![MaterialOverrideData {
                surface: 0,
                resource: Some("res://materials/skin.tres".into()),
                material_type: "StandardMaterial3D".into(),
            }],
        }],
        collision_shapes: vec![CollisionShapeData {
            child: "CollisionShape3D".into(),
            shape_type: "CapsuleShape3D".into(),
            dimensions: {
                let mut m = serde_json::Map::new();
                m.insert("radius".into(), json!(0.5));
                m.insert("height".into(), json!(1.8));
                m
            },
            inline: true,
            disabled: false,
        }],
        animation_players: vec![],
        navigation_agents: vec![],
        sprites: vec![],
        particles: vec![],
        shader_params: serde_json::Map::new(),
    };
    let json = serde_json::to_value(&resources).unwrap();
    let back: InspectResources = serde_json::from_value(json).unwrap();
    assert_eq!(back.meshes.len(), 1);
    assert_eq!(back.meshes[0].surface_count, 3);
    assert_eq!(back.collision_shapes[0].dimensions["radius"], 0.5);
}

#[test]
fn inspect_resources_empty_collections_omitted() {
    let resources = InspectResources {
        meshes: vec![],
        collision_shapes: vec![],
        animation_players: vec![],
        navigation_agents: vec![],
        sprites: vec![],
        particles: vec![],
        shader_params: serde_json::Map::new(),
    };
    let json = serde_json::to_string(&resources).unwrap();
    // Empty vecs should be skipped
    assert!(!json.contains("meshes"));
    assert!(!json.contains("collision_shapes"));
}

#[test]
fn inspect_category_resources_deserializes() {
    let json = json!("resources");
    let cat: InspectCategory = serde_json::from_value(json).unwrap();
    assert_eq!(cat, InspectCategory::Resources);
}
```

### Unit Tests: `crates/spectator-server/src/mcp/inspect.rs`

```rust
#[test]
fn parse_include_resources() {
    let include = vec!["resources".into()];
    let result = parse_include(&include).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], InspectCategory::Resources);
}

#[test]
fn default_include_excludes_resources() {
    let defaults = default_include();
    assert!(!defaults.contains(&"resources".to_string()));
}
```

### Integration Tests: `crates/spectator-server/tests/tcp_mock.rs`

See Unit 5 above.

---

## Verification Checklist

```bash
# Build all crates (protocol types must compile first)
cargo build --workspace

# Run unit tests (protocol + server)
cargo test --workspace

# Run integration tests (TCP mock)
cargo test --workspace --features integration-tests

# Lint
cargo clippy --workspace
cargo fmt --check

# Deploy and verify in Godot headless
theatre-deploy
godot --headless --quit --path ~/godot/test-harness 2>&1
# Expected: no SCRIPT ERROR or [panic] lines
```
