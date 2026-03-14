<script setup>
const messages0 = [
  { role: 'human', text: `Build the corridor room connecting Chamber_A to Chamber_B.` },
  { role: 'agent', text: `Current scene has Chamber_A at x=0 and Chamber_B at x=18. I need to build the corridor from x=6 to x=12 (6m length, centered, with 2m side chambers). Building now.` },
  { role: 'agent', text: `Corridor floor created (8m × 2m, centered at x=9). Adding walls and ceiling.` },
]
const messages1 = [
  { role: 'agent', text: `North wall, south wall, and locked door placed. Corridor geometry complete. Adding enemy guard positions.` },
]
const messages2 = [
  { role: 'agent', text: `Both guards placed with patrol routes. Phase 1 complete — corridor is built. Time to verify.` },
]
const messages3 = [
  { role: 'human', text: `Game is running. Verify the corridor is navigable.` },
  { role: 'agent', text: `Path from Chamber_A entrance to corridor center: reachable=true, distance=7.2m, 2 waypoints. Good — the navmesh covers the connection.` },
  { role: 'agent', text: `Path from Chamber_A to Chamber_B: reachable=false. Expected — the locked door blocks the path. Let me verify the guards are reachable.` },
]
const messages4 = [
  { role: 'agent', text: `Guard_Corridor_A at [8.0, 0.0, 0.0]: on_floor=true. Nearby: WallNorth at 1.1m (north), WallSouth at 1.1m (south). The guard has 2.2m of clearance side-to-side. With player radius 0.4m × 2 = 0.8m, there is 1.4m of space to pass by the guard. Sufficient for the player to slip past.` },
]
const messages5 = [
  { role: 'agent', text: `Static bodies within 3m of corridor center:\\n- WallNorth at 1.1m\\n- WallSouth at 1.1m\\n- Floor at 0.1m below (expected)\\n\\nCorridor clear width is 2.2m between walls. Player capsule diameter is 0.8m. Navmesh agent radius is 0.5m (total bake clearance 1.0m). Available navmesh width = 2.2 - 1.0 = 1.2m. The navmesh should bake correctly with a comfortable margin.` },
]
const messages6 = [
  { role: 'agent', text: `Guard_A patrol route: reachable=true, distance=2.5m. 2 waypoints, direct path along the corridor. Guard patrol works.` },
  { role: 'agent', text: `Guard_B patrol route: reachable=true, distance=2.5m. Both guards have valid patrol routes. Corridor is complete and verified.` },
]
const messages7 = [
  { role: 'human', text: `I walked the player through but they are clipping into WallNorth slightly. Check the wall position.` },
  { role: 'agent', text: `WallNorth position=[9.0, 1.25, -1.1], CollisionShape3D BoxShape3D size=[8.0, 2.5, 0.2]. The wall extends from z=-1.0 to z=-1.2. But the corridor floor ends at z=-1.0 — there is no overlap/gap. The 0.2m gap between the floor edge and the wall face is exactly player radius. At player radius=0.4m, the player can approach within 0.4m of z=-1.0, meaning the wall face at z=-1.0 should allow 0.0m margin. The collision shape needs to move 0.1m north.` },
  { role: 'agent', text: `WallNorth moved 0.1m north. The wall face now aligns correctly with the floor edge. Reload and verify.` },
]
</script>

# Build & Verify

The flagship cross-tool workflow: Director builds a room, Stage verifies a player can navigate it.

This example demonstrates the **build → run → verify → adjust** loop — the most powerful pattern in Theatre. An AI agent constructs a section of geometry, then uses spatial queries to verify that:
- The player can navigate through the area
- All passages are wide enough
- No geometry blocks intended pathways
- Enemy spawn points have valid paths to player-reachable areas

## The task

Build a corridor room connecting two larger chambers. The corridor must be:
- 2m wide (player radius 0.4m × 2 + 1.2m clearance = 2m minimum)
- 2.5m tall
- 8m long
- Have two guard positions (enemies that should be able to patrol the corridor)
- Have a locked door at the far end (StaticBody3D, removable at runtime)

## Phase 1: Build with Director

### Create the room

<AgentConversation :messages="messages0" />

### Add walls

<AgentConversation :messages="messages1" />

### Place guards

<AgentConversation :messages="messages2" />

## Phase 2: Verify with Stage

Press F5 to run the game.

### Verify basic navigation

<AgentConversation :messages="messages3" />

### Verify guard positions

<AgentConversation :messages="messages4" />

### Check corridor width against agent radius

<AgentConversation :messages="messages5" />

### Verify guard patrol paths

<AgentConversation :messages="messages6" />

## Phase 3: Detect and fix an issue

<AgentConversation :messages="messages7" />

## The complete loop in summary

| Phase | Action | Tool |
|---|---|---|
| Build | Create scene structure | Director batch |
| Build | Place geometry | Director batch |
| Build | Instance enemies | Director batch |
| Build | Configure exports | Director batch |
| Verify | Check navmesh connectivity | Stage `path_distance` |
| Verify | Check corridor width | Stage `spatial_query radius` |
| Verify | Check patrol routes | Stage `path_distance` |
| Fix | Adjust wall position | Director `node_set_properties` |
| Verify | Confirm fix | Stage `spatial_inspect` |

This loop — build, run, verify, adjust — is what Theatre is designed for. Neither tool alone is sufficient: Director can build the geometry but cannot verify navigability; Stage can verify navigability but cannot build geometry. Together, they close the loop.
