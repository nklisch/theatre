---
name: godot-naming
description: "Godot 4 property and field naming conventions for Stage API
  output. Use when naming any JSON response field, MCP parameter, or protocol
  struct field that corresponds to a Godot node property or method."
user-invocable: false
allowed-tools: Read
---

# Godot 4 Naming Reference

Full dictionary: [GODOT-NAMING.md](GODOT-NAMING.md)
Contract rules: `.claude/rules/contracts.md`

## Decision Rules (apply in order)

1. **Exact Godot property** → use it unchanged: `velocity`, `scale`, `visible`,
   `collision_layer`, `collision_mask`, `process_mode`, `name`
2. **Godot `get_X()` method, no matching property** → drop `get_`:
   `get_class()` → `class`, `get_path()` → `path`, `get_groups()` → `groups`
3. **Godot `is_X()` method** → drop `is_`:
   `is_on_floor()` → `on_floor`, `is_on_wall()` → `on_wall`
4. **`rotation_degrees`** → `rotation_deg` (only allowed abbreviation)
5. **Stage-computed, no Godot equivalent** → full descriptive snake_case,
   no abbreviations: `relative`, `bearing`, `bearing_deg`, `distance`,
   `occluded`, `timestamp_ms`, `global_position` (from `global_transform.origin`)

## Quick Lookup

| Concept | Godot | API field |
|---------|-------|-----------|
| World position (3D/2D) | `global_position` | `global_position` |
| Local position (3D/2D) | `position` | `position` |
| Rotation in degrees (3D full) | `rotation_degrees` | `rotation_deg` |
| Rotation Y only (3D yaw) | `rotation_degrees.y` | `rotation_y_deg` |
| Rotation in degrees (2D) | `rotation_degrees` | `rotation_deg` |
| Scale | `scale` | `scale` |
| Transform basis matrix | `transform.basis` | `basis` |
| Transform local origin | `transform.origin` | `origin` |
| Linear velocity (CharacterBody) | `velocity` | `velocity` |
| Linear velocity (RigidBody) | `linear_velocity` | `linear_velocity` |
| Angular velocity | `angular_velocity` | `angular_velocity` |
| On floor | `is_on_floor()` | `on_floor` |
| On wall | `is_on_wall()` | `on_wall` |
| On ceiling | `is_on_ceiling()` | `on_ceiling` |
| Floor normal | `get_floor_normal()` | `floor_normal` |
| Collision layer | `collision_layer` | `collision_layer` |
| Collision mask | `collision_mask` | `collision_mask` |
| Node name | `name` | `name` |
| Node path | `get_path()` | `path` |
| Class name | `get_class()` | `class` |
| Instance ID | `get_instance_id()` | `instance_id` |
| Groups | `get_groups()` | `groups` |
| Visible in tree | `is_visible_in_tree()` | `visible` |
| Script path | `get_script().get_path()` | `script` |
| Process mode | `process_mode` | `process_mode` |

## Stage-Specific (no Godot equivalent)

| Field | Description |
|-------|-------------|
| `relative` | Distance + bearing + elevation + occlusion from current perspective |
| `bearing` | Cardinal direction string (`"ahead_left"` etc.) |
| `bearing_deg` | Exact bearing in degrees (0 = ahead, clockwise) |
| `distance` | Straight-line Euclidean distance in world units |
| `occluded` | Whether line-of-sight is blocked |
| `forward` | Unit forward vector (−Z local = `-(global_transform.basis.col_c())`) |
| `timestamp_ms` | Milliseconds since game start (`Time.get_ticks_msec()`) |

## Common Mistakes

```
abs            ✗  →  global_position  ✓
rot            ✗  →  rotation_deg     ✓
rot_y          ✗  →  rotation_y_deg   ✓
rel            ✗  →  relative         ✓
global_origin  ✗  →  global_position  ✓
local_origin   ✗  →  position         ✓
dist           ✗  →  distance         ✓
```
