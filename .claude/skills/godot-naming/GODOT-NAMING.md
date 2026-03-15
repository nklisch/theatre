# Godot 4 Naming Dictionary

Reference for Stage API field naming. When adding fields to MCP responses,
use the "API field name" column. Prefer Godot's exact property name when it
exists; use descriptive snake_case when Godot uses a method (`get_X`) with no
corresponding property name.

Sourced from Godot 4.3 class reference and the godot-rust/gdext bindings.

---

## Position & Transform

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| World-space position (3D) | `global_position` | `global_position` | `Vector3`; from `Node3D.get_global_position()` |
| World-space position (2D) | `global_position` | `global_position` | `Vector2`; from `Node2D.get_global_position()` |
| Local position (3D) | `position` | `position` | relative to parent; from `Node3D.get_position()` |
| Local position (2D) | `position` | `position` | relative to parent; from `Node2D.get_position()` |
| Global transform origin | `Transform3D.origin` | `origin` | `Vector3` field on `Transform3D` struct |
| Global transform (3D) | `global_transform` | `global_transform` | `Transform3D`; from `Node3D.get_global_transform()` |
| Global transform (2D) | `global_transform` | `global_transform` | `Transform2D`; from `Node2D.get_global_transform()` |
| Local transform (3D) | `transform` | `transform` | `Transform3D`; from `Node3D.get_transform()` |
| Local transform (2D) | `transform` | `transform` | `Transform2D`; from `Node2D.get_transform()` |

**Godot 3 note:** Position was accessed as `translation` on `Spatial` (the Godot 3
name for `Node3D`). In Godot 4, `Spatial` was renamed to `Node3D`, and `translation`
was renamed to `position`. Use `position`/`global_position`.

---

## Rotation

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Local rotation in radians (3D) | `rotation` | — | `Vector3` (Euler XYZ in radians); rarely needed in API |
| Local rotation in degrees (3D) | `rotation_degrees` | `rotation_deg` | `Vector3`; from `Node3D.get_rotation_degrees()` |
| World rotation in degrees (3D) | `global_rotation_degrees` | `rotation_deg` (in `PerspectiveData`) | `Vector3`; from `Node3D.get_global_rotation_degrees()` |
| Local rotation in radians (2D) | `rotation` | — | `f32` scalar in radians |
| Local rotation in degrees (2D) | `rotation_degrees` | `rotation_deg` | `f32` scalar; from `Node2D.get_rotation_degrees()` |
| World rotation in degrees (2D) | `global_rotation_degrees` | `rotation_deg` | `f32` scalar; from `Node2D.get_global_rotation_degrees()` |
| Rotation as quaternion | `quaternion` | `quaternion` | `Quaternion` (x, y, z, w); from `Node3D.get_quaternion()` |

**Key facts:**
- `rotation_degrees` is the correct Godot 4 name (not `rotation_deg`). Stage
  uses `rotation_deg` in its API (both as a field name and in struct names) because
  it is shorter and unambiguous. When reading from Godot, call `get_rotation_degrees()`.
- `rotation_degrees` still exists in Godot 4. It was not removed from Godot 3 to 4.
- Godot's Euler order for 3D rotation is YXZ by default (via `Basis.get_euler()`).

---

## Scale

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Local scale (3D) | `scale` | `scale` | `Vector3`; from `Node3D.get_scale()` |
| Local scale (2D) | `scale` | `scale` | `Vector2`; from `Node2D.get_scale()` |
| Scale derived from basis | `Basis.get_scale()` | `scale` | returns `Vector3`; used when only transform is available |

---

## Basis / Transform Internals

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Rotation/scale matrix (3D) | `basis` | `basis` | `Basis` struct; field on `Transform3D`; from `Node3D.get_basis()` |
| Basis column (local X axis) | `Basis.col_a()` / rows[0] | — | gdext exposes as `col_a()` (Godot: column 0 = local X) |
| Basis column (local Y axis) | `Basis.col_b()` / rows[1] | — | gdext exposes as `col_b()` (Godot: column 1 = local Y) |
| Basis column (local Z axis / forward) | `Basis.col_c()` / rows[2] | `forward` | Godot forward is **-Z**; negate `col_c()` for forward vector |
| Basis internal storage | `Basis.rows: [Vector3; 3]` | — | gdext internal; basis columns ≠ basis rows |
| 2D transform matrix | `Transform2D` | — | 3-column matrix (x, y, origin); no `Basis` equivalent |
| 2D transform origin | `Transform2D.origin` | `origin` | `Vector2` field |

**Note:** In gdext, `Basis` stores its data as `rows` (row-major), but Godot's
mathematical convention treats the columns as the local axes. The gdext helpers
`col_a()`, `col_b()`, `col_c()` extract the columns. To get the node's local
forward direction, use `-col_c()` (Godot forward is -Z in local space).

---

## Physics — Velocity

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| CharacterBody3D velocity | `velocity` (property) | `velocity` | `Vector3`; `get_velocity()` / `set_velocity()` |
| CharacterBody2D velocity | `velocity` (property) | `velocity` | `Vector2`; `get_velocity()` / `set_velocity()` |
| RigidBody3D linear velocity | `linear_velocity` (property) | `linear_velocity` | `Vector3`; `get_linear_velocity()` / `set_linear_velocity()` |
| RigidBody2D linear velocity | `linear_velocity` (property) | `linear_velocity` | `Vector2`; same accessor pattern |
| RigidBody3D angular velocity | `angular_velocity` (property) | `angular_velocity` | `Vector3`; `get_angular_velocity()` / `set_angular_velocity()` |
| RigidBody2D angular velocity | `angular_velocity` (property) | `angular_velocity` | `f32` scalar for 2D |

**Naming decision:** Stage uses `velocity` as the unified field name in entity
snapshots. For `CharacterBody*` nodes this maps directly. For `RigidBody*` nodes,
`linear_velocity` is aliased to `velocity` in the snapshot output for consistency.
When more granularity is needed (e.g., in `spatial_inspect`), use `linear_velocity`
and `angular_velocity` to match Godot.

---

## Physics — Contact States (CharacterBody)

| Concept | Godot method | API field name | Notes |
|---|---|---|---|
| Touching floor | `is_on_floor()` | `on_floor` | Method (not property); returns `bool` |
| Touching wall | `is_on_wall()` | `on_wall` | Method; returns `bool` |
| Touching ceiling | `is_on_ceiling()` | `on_ceiling` | Method; returns `bool` |
| Floor surface normal | `get_floor_normal()` | `floor_normal` | Method; `Vector3` (3D) or `Vector2` (2D) |
| Wall surface normal | `get_wall_normal()` | `wall_normal` | Method; `Vector3` only |
| Apply velocity + collide | `move_and_slide()` | — | Mutating method; not in output |

**Note:** These are all **methods**, not properties. They return results computed
during the last `move_and_slide()` call in the same physics frame. Stage strips
`is_` prefix for API fields: `is_on_floor()` → `on_floor`.

---

## Physics — Collision Layers

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Collision layer bitmask | `collision_layer` | `collision_layer` | `u32`; on `CollisionObject3D`/`CollisionObject2D` |
| Collision mask bitmask | `collision_mask` | `collision_mask` | `u32`; which layers this body collides with |
| Individual bit check | `get_collision_layer_value(n)` | — | 1-indexed; not in bulk output |

**Inheritance:** `collision_layer` and `collision_mask` are defined on
`CollisionObject3D` / `CollisionObject2D` and inherited by all physics bodies
(`CharacterBody*`, `RigidBody*`, `StaticBody*`, `Area*`).

---

## RigidBody Properties

| Concept | Godot property | API field name | Notes |
|---|---|---|---|
| Mass | `mass` | `mass` | `f32`; `get_mass()` |
| Gravity scale | `gravity_scale` | `gravity_scale` | `f32`; multiplier on world gravity |
| Linear damping | `linear_damp` | `linear_damp` | `f32` |
| Angular damping | `angular_damp` | `angular_damp` | `f32` |
| Frozen | `freeze` (Godot 4) | `frozen` | Bool; replaces Godot 3 `mode` enum; property name is `freeze`, getter is `is_freeze_enabled()` |
| Freeze mode | `freeze_mode` | `freeze_mode` | Enum: `FREEZE_MODE_STATIC` or `FREEZE_MODE_KINEMATIC` |
| Sleeping | `sleeping` | `sleeping` | Bool; `is_sleeping()` |
| Can sleep | `can_sleep` | `can_sleep` | Bool; `is_able_to_sleep()` |

**Godot 3 note:** RigidBody3D `mode` enum (RIGID, STATIC, CHARACTER, KINEMATIC)
was replaced in Godot 4 with separate `freeze` / `freeze_mode` properties.

---

## Node Identity

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Node name | `name` (property) | `name` | `StringName`; from `get_name()` |
| Scene-tree path | `get_path()` | `path` | Returns `NodePath`; e.g. `"enemies/scout_02"` |
| Class name | `get_class()` | `class` | Returns `GString`; e.g. `"CharacterBody3D"` |
| Instance ID | `get_instance_id()` | `instance_id` | `u64`; unique per node lifetime |
| Owner node | `owner` (property) | — | Rarely needed in output |
| Scene file | `scene_file_path` (property) | — | For instanced scenes |
| Unique name | `unique_name_in_owner` (property) | — | For `%UniqueNode` syntax |
| Process mode | `process_mode` (property) | `process_mode` | Enum; see variants below |

**`process_mode` values** (Godot enumerator names):
- `PROCESS_MODE_INHERIT` — inherit from parent (default)
- `PROCESS_MODE_PAUSABLE` — process normally; pause when game is paused
- `PROCESS_MODE_WHEN_PAUSED` — only process while game is paused
- `PROCESS_MODE_ALWAYS` — always process regardless of pause state
- `PROCESS_MODE_DISABLED` — never process

---

## Node Groups & Visibility

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Group membership | `get_groups()` | `groups` | Returns `Array[StringName]`; strip `_` prefixed internal groups |
| In-group check | `is_in_group(name)` | — | Method for filtering |
| Add to group | `add_to_group(name)` | — | Mutating; not in output |
| Visible (local) | `visible` (property on `CanvasItem`) | — | For 2D nodes; `is_visible()` getter |
| Visible in tree | `is_visible_in_tree()` | `visible` | Accounts for parent visibility; use this not `visible` |
| Visible (3D) | `visible` (property on `Node3D`) | — | `is_visible()` getter; also has `is_visible_in_tree()` |

**Note:** `visible` is a property on both `Node3D` and `CanvasItem` (the 2D base).
Stage uses `is_visible_in_tree()` for both 2D and 3D snapshot output because it
correctly reflects inherited visibility from parent nodes.

---

## Script & Signals

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Attached script | `get_script()` | `script` | Returns `Variant` (cast to `Resource` / `GDScript`); `get_path()` for the file path |
| Script resource path | `script.get_path()` | `script` (as path string) | e.g. `"res://scripts/enemy.gd"` |
| Script base class | `script.get_instance_base_type()` | `base_class` | GDScript introspection |
| All signals on node | `get_signal_list()` | — | Returns `Array[Dictionary]`; each dict has `"name"` key |
| Signal connection list | `get_signal_connection_list(name)` | — | Returns connections for a named signal |
| Connected signals | (iterate `get_signal_list()`) | `signals_connected` | List of signal names that have at least one connection |
| Script methods | (iterate `get_method_list()`) | `methods` | Available script-defined methods |

---

## Camera3D

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Field of view | `fov` (property) | `fov` | `f32` degrees; only for perspective projection |
| Near clip | `near` (property) | `near` | `f32` world units |
| Far clip | `far` (property) | `far` | `f32` world units |
| Projection type | `projection` (property) | `projection` | Enum: `PERSPECTIVE`, `ORTHOGONAL`, `FRUSTUM` |
| Orthographic size | `size` (property) | `size` | `f32`; height of viewport in world units (orthographic only) |
| Current camera | `current` (property) | `current` | Bool; `is_current()` getter |
| Frustum planes | `get_frustum()` | `frustum` | Returns `Array[Plane]`; computed, not a property |
| Camera world transform | `get_camera_transform()` | — | Returns `Transform3D`; use `global_transform` instead |

---

## Camera2D

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Zoom level | `zoom` (property) | `zoom` | `Vector2`; `get_zoom()` |
| Camera offset | `offset` (property) | `offset` | `Vector2`; additional offset from followed target |
| Anchor mode | `anchor_mode` (property) | `anchor_mode` | Enum: `ANCHOR_MODE_FIXED_TOP_LEFT`, `ANCHOR_MODE_DRAG_CENTER` |
| Ignore rotation | `ignore_rotation` (property) | — | Bool; `is_ignoring_rotation()` |
| Current camera | `current` (property) | `current` | Bool; `is_current()` |
| Screen center | `get_screen_center_position()` | `screen_center` | `Vector2`; computed method, not a property |
| Canvas transform | `Viewport.get_canvas_transform()` | — | `Transform2D`; apply to world pos to get screen pos |

---

## Viewport

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Viewport size | `get_visible_rect().size` | — | `Vector2`; pixel dimensions of the viewport |
| Active 3D camera | `get_camera_3d()` | — | Returns `Camera3D` or null |
| Active 2D camera | `get_camera_2d()` | — | Returns `Camera2D` or null |
| Canvas transform | `get_canvas_transform()` | — | `Transform2D`; world→screen conversion for 2D |
| World3D | `world_3d` (property) | — | The physics/rendering world for 3D |

---

## AnimationPlayer

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Playing animation name | `current_animation` (property) | `current_animation` | `String`; empty if stopped |
| Is playing | `is_playing()` | `playing` | Method; `bool` |
| Playback position | `get_current_animation_position()` | `position_sec` | `f64` seconds |
| Animation length | `get_current_animation_length()` | `length_sec` | `f64` seconds |
| Available animations | `get_animation_list()` | `animations` | `PackedStringArray` |
| Playback speed | `get_playing_speed()` | — | Read-only; `f32` |
| Autoplay | `autoplay` (property) | — | `String`; name of animation to autoplay |

---

## NavigationAgent

| Concept | Godot property/method | API field name | Notes |
|---|---|---|---|
| Target position | `target_position` (property) | `target_position` | `Vector3`/`Vector2` |
| Target reached | `is_target_reached()` | `target_reached` | Method; `bool` |
| Distance remaining | `distance_to_target()` | `distance_remaining` | Method; `f64` |
| Avoidance enabled | `avoidance_enabled` (property) | `avoidance_enabled` | `bool` |
| Path postprocessing | `path_postprocessing` (property) | `path_postprocessing` | Enum as string |

---

## Built-in Types (Vector/Transform fields)

| Type | Field names | Notes |
|---|---|---|
| `Vector2` | `.x`, `.y` | `f32` components; zero constant: `Vector2::ZERO` |
| `Vector3` | `.x`, `.y`, `.z` | `f32` components; zero constant: `Vector3::ZERO` |
| `Transform3D` | `.basis` (`Basis`), `.origin` (`Vector3`) | Both are public struct fields |
| `Transform2D` | `.a` (x column), `.b` (y column), `.origin` (`Vector2`) | gdext field names |
| `Basis` | `.rows: [Vector3; 3]` (internal) | Use `col_a()`/`col_b()`/`col_c()` for columns/axes |
| `Quaternion` | `.x`, `.y`, `.z`, `.w` | `f32`; identity: `Quaternion::IDENTITY` |

---

## Stage-Specific Fields (no direct Godot equivalent)

These fields appear in Stage's MCP API output but have no exact Godot
property they map to — they are computed or aggregated by Stage.

| API field name | Description | Derived from |
|---|---|---|
| `path` | Node path relative to scene root | `root.get_path_to(node)` |
| `rotation_deg` (struct field) | Abbreviated name for `rotation_degrees` | `get_rotation_degrees()` / `get_global_rotation_degrees()` |
| `on_floor` | Abbreviated from `is_on_floor()` | `CharacterBody*.is_on_floor()` |
| `on_wall` | Abbreviated from `is_on_wall()` | `CharacterBody*.is_on_wall()` |
| `on_ceiling` | Abbreviated from `is_on_ceiling()` | `CharacterBody*.is_on_ceiling()` |
| `forward` | Unit forward vector (−Z in local space) | `-(global_transform.basis.col_c())` |
| `bearing` | Cardinal direction to target | Computed server-side from positions |
| `bearing_deg` | Exact bearing in degrees (0=ahead, CW) | Computed server-side |
| `distance` | Straight-line Euclidean distance | Computed server-side from positions |
| `occluded` | Whether line-of-sight is blocked | Physics raycast result |
| `origin` | Transform3D origin vector in transform data | `global_transform.origin` |
| `is_static` | Whether node is a `StaticBody*` | class check |
| `timestamp_ms` | Milliseconds since game start | `Time.get_ticks_msec()` |
| `camera_visible` | Whether node is in camera frustum | Godot frustum culling / raycast |
| `camera_distance` | Distance from active camera | Computed from camera position |
| `in_areas` | List of `Area*` nodes containing this node | `get_overlapping_bodies()` |
| `signals_recent` | Signal emissions in last N frames | Recorded by signal watcher |

---

## Naming Conventions Summary

| Convention | Rule |
|---|---|
| Godot property → API field | Use exact Godot property name unchanged |
| Godot `get_X()` method → API field | Drop `get_` prefix: `get_class()` → `class` |
| Godot `is_X()` method → API field | Drop `is_` prefix: `is_on_floor()` → `on_floor` |
| Godot abbreviation for API | `rotation_degrees` → `rotation_deg` (Stage convention) |
| Computed fields | Use descriptive snake_case: `bearing_deg`, `distance`, `occluded` |
| Boolean state | No `is_` prefix in JSON output: `on_floor: true`, not `is_on_floor: true` |
| Enum values in JSON | Use `snake_case` (serde `rename_all = "snake_case"`) |
