---
description: "Debug pathfinding failures — AI agent inspects navigation meshes and agent properties to fix stuck NPCs."
---

<script setup>
const messages0 = [
  { role: 'human', text: `The enemy near the south doorway is not moving. It just stands there. Game is running.` },
  { role: 'agent', text: `NavigationAgent3D state: target_position=[2.0, 0.0, -8.0] (the player), is_navigation_finished=false, path_desired_distance=0.5, target_desired_distance=0.5, navigation_layers=1. The agent has a valid target and thinks navigation is not finished — but the enemy is not moving. Let me check the enemy itself.` },
]
const messages1 = [
  { role: 'agent', text: `path_distance result: reachable=false. The navigation mesh reports no path from Enemy_3 at [-5.2, 0.0, -12.1] to Player at [2.0, 0.0, -8.0]. The enemy's position is on a disconnected navmesh island — it cannot navigate to the player even though it has a target.` },
]
const messages2 = [
  { role: 'human', text: `Why is it disconnected? The navmesh should cover that area.` },
  { role: 'agent', text: `NavigationRegion3D: agent_radius=0.5, agent_height=1.8, cell_size=0.25. Let me check where Enemy_3 is relative to the doorway.` },
  { role: 'agent', text: `Enemy_3 is 1.8 meters south of SouthDoorway. The doorway gap is 1.2 meters wide. But the NavigationAgent3D has agent_radius=0.5 — the agent clearance diameter is 1.0m. The doorway (1.2m wide) minus agent clearance (1.0m) leaves only 0.2m of navigable space. With cell_size=0.25, that is less than one navmesh cell. The navmesh bake rounded this gap to zero — creating the disconnection.` },
]
const messages3 = [
  { role: 'human', text: `So the doorway is too narrow relative to the agent radius?` },
  { role: 'agent', text: `Exactly. The effective navigable width at a doorway is: doorway_width - (2 × agent_radius). Your doorway is 1.2m and agent_radius is 0.5m: 1.2 - 2×0.5 = 0.2m. The navmesh bake sees 0.2m of clearance, which rounds down to zero given cell_size=0.25. Two fixes: (1) Widen the doorway to at least 1.5m (1.5 - 1.0 = 0.5m clearance, two cells). (2) Reduce agent_radius to 0.3m (1.2 - 0.6 = 0.6m clearance). Widening the doorway is more robust.` },
]
const messages4 = [
  { role: 'human', text: `Widen the south doorway to 1.6m and rebake the navmesh.` },
  { role: 'agent', text: `Doorway widened to 1.6m. Rebake the navmesh in Godot (NavigationRegion3D → Bake Navigation Mesh) and test.` },
  { role: 'human', text: `Rebaked and running. Enemy is moving now.` },
  { role: 'agent', text: `path_distance: reachable=true, distance=7.8 meters, 3 waypoints. Enemy_3 now has a valid path to the player. The disconnected island is resolved.` },
]
</script>

# Pathfinding Failures

**Scenario**: An NPC enemy refuses to move toward the player in a specific area of the map. In the editor the navmesh looks baked and correct. The enemy works fine in most of the level but consistently gets stuck near the south doorway.

## Setup

- `Enemy`: `CharacterBody3D` with a `NavigationAgent3D` child
- `NavigationRegion3D`: covers the main room and corridors
- Enemy script calls `navigation_agent.set_target_position(player.global_position)` every second

## Step 1: Observe the stuck enemy

<AgentConversation :messages="messages0" />

## Step 2: Check the path

<AgentConversation :messages="messages1" />

## Step 3: Investigate the navmesh

<AgentConversation :messages="messages2" />

## Step 4: Verify the diagnosis

<AgentConversation :messages="messages3" />

## Step 5: Apply the fix

<AgentConversation :messages="messages4" />

## Key takeaways

- **`path_distance` is the fastest way to diagnose navmesh disconnection.** `reachable: false` immediately tells you the enemy cannot reach the target — no guessing.
- **Agent radius × 2 must be less than the narrowest passage.** This is the fundamental rule of navmesh baking. Narrow doors, tight corridors, and sharp corners all reduce effective navigable width.
- **The navmesh baking happens at design time**, not at runtime. Structural changes (wider doorway) require rebaking. Property changes to `NavigationRegion3D` (agent_radius) also require rebaking.
- **Verify with `path_distance` after the fix**, not just by eye. The navmesh may still have gaps that are not visually obvious.

## Related patterns

If `path_distance` returns `reachable: true` but the enemy still does not move:

```
spatial_inspect (NavigationAgent3D):
  - is_navigation_finished=true → agent thinks it has arrived (check target_desired_distance)
  - target_position=[0,0,0] → the script is not calling set_target_position()
  - navigation_layers ≠ navmesh layers → layer mismatch (agent and region must match)
```
