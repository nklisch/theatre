<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['spatial_inspect'] ?? []

const messages0 = [
  { role: 'human', text: `The EnemyDetectionZone is not triggering. What is wrong with it?` },
  { role: 'agent', text: `Found the issue. EnemyDetectionZone has monitoring=true and collision_layer=2, but collision_mask=0 — it is not monitoring any layers. It will never detect anything. Also, the body_entered signal has no connections — even if it detected something, nothing would respond. Set collision_mask to include the player layer (layer 1), and connect body_entered to the enemy script.` },
]
const messages1 = [
  { role: 'human', text: `Is the attack animation playing right now?` },
]
const messages2 = [
  { role: 'human', text: `The enemy is not moving toward the player. Check its navigation agent.` },
  { role: 'agent', text: `NavigationAgent3D: is_navigation_finished=true, target_position=[0,0,0]. The target position is [0,0,0] — the agent has not been given a target. The navigation_layers=1 which matches the navmesh, so the navmesh is configured correctly. The enemy script is not calling set_target_position() with the player position.` },
]
</script>

# spatial_inspect

Deep inspection of a single node.

`spatial_inspect` returns everything Spectator knows about one node: all tracked properties, signal connections, children, and spatial context (nearby nodes, relationship to parent). It is the right tool when you need complete information about a specific node.

## When to use it

- **Diagnosing a specific node**: "Why is this Area3D not detecting the player?"
- **Reading full property values**: collision layers, animation state, physics settings
- **Understanding node hierarchy**: what children does this node have?
- **Checking signal connections**: is this signal actually connected?
- **After narrowing down from a snapshot or query**: you found the suspicious node, now inspect it

Do **not** use `spatial_inspect` to scan many nodes — it is designed for targeted investigation. For overview, use `spatial_snapshot`; for searching, use `spatial_query`.

## Parameters

<ParamTable :params="params" />

### `include` options

| Value | Description |
|---|---|
| `"properties"` | All tracked Godot properties of this node |
| `"signals"` | Signal connections: which signals are connected, to which targets |
| `"children"` | Direct children with their classes and positions |
| `"spatial_context"` | Nearby nodes (within 10m), parent info, and relative position |

You can combine any set:
```json
{
  "node": "Player",
  "include": ["properties", "signals", "children", "spatial_context"]
}
```

## Response format

```json
{
  "node": "Player",
  "path": "World/Player",
  "class": "CharacterBody3D",
  "frame": 450,
  "properties": {
    "global_position": [2.3, 0.0, -1.7],
    "rotation_deg": [0.0, 45.0, 0.0],
    "velocity": [2.0, 0.0, 0.0],
    "scale": [1.0, 1.0, 1.0],
    "visible": true,
    "collision_layer": 1,
    "collision_mask": 3,
    "on_floor": true,
    "on_wall": false,
    "floor_snap_length": 0.1,
    "motion_mode": "grounded"
  },
  "signals": [
    {
      "signal": "body_entered",
      "connected_to": "World/Player",
      "method": "_on_body_entered"
    }
  ],
  "children": [
    { "name": "CollisionShape3D", "class": "CollisionShape3D", "relative_position": [0.0, 0.0, 0.0] },
    { "name": "WeaponPivot", "class": "Node3D", "relative_position": [0.3, 1.2, 0.0] },
    { "name": "Camera3D", "class": "Camera3D", "relative_position": [0.0, 1.8, 0.5] }
  ],
  "spatial_context": {
    "parent": {
      "name": "World",
      "class": "Node3D",
      "relative_position": [2.3, 0.0, -1.7]
    },
    "nearby": [
      { "node": "Pickup_0", "class": "Area3D", "distance": 1.3 },
      { "node": "Enemy_0", "class": "CharacterBody3D", "distance": 5.4 }
    ]
  }
}
```

### Property coverage by class

Properties returned by Spectator depend on the node class. Here are the most common:

**`CharacterBody3D`**: `global_position`, `rotation_deg`, `velocity`, `scale`, `visible`, `collision_layer`, `collision_mask`, `on_floor`, `on_wall`, `on_ceiling`, `floor_snap_length`, `motion_mode`, `up_direction`

**`RigidBody3D`**: `global_position`, `rotation_deg`, `linear_velocity`, `angular_velocity`, `scale`, `visible`, `collision_layer`, `collision_mask`, `gravity_scale`, `freeze`, `lock_rotation`

**`Area3D`**: `global_position`, `rotation_deg`, `scale`, `visible`, `collision_layer`, `collision_mask`, `monitoring`, `monitorable`

**`AnimationPlayer`**: `current_animation`, `current_animation_position`, `current_animation_length`, `speed_scale`, `autoplay`, `playback_active`

**`NavigationAgent3D`**: `path_desired_distance`, `target_desired_distance`, `path_max_distance`, `navigation_layers`, `target_position`, `is_navigation_finished`

**`Camera3D`**: `global_position`, `rotation_deg`, `fov`, `near`, `far`, `current`

**`Light3D`**: `global_position`, `visible`, `light_energy`, `light_color`, `shadow_enabled`

## Example conversations

### Debugging a detection zone

<AgentConversation :messages="messages0" />

### Understanding animation state

<AgentConversation :messages="messages1" />

### Checking a navigation agent

<AgentConversation :messages="messages2" />

## Tips

**Start with `include: ["properties"]`** unless you know you need signals or children. It keeps the response focused.

**Add `"spatial_context"` for relational debugging.** Spatial context shows nearby nodes with distances, which often reveals the cause of detection or collision problems without additional queries.

**Use the full path for ambiguous node names.** If you have multiple nodes named `CollisionShape3D`, use `"Player/CollisionShape3D"` or `"Enemy_0/CollisionShape3D"` to be specific.

**Inspect `AnimationPlayer` during the bug moment.** For timing bugs (hitbox not active during attack), inspect the `AnimationPlayer` right when the issue occurs and check `current_animation_position` against your expected keyframe times.
