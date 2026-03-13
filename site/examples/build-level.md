<script setup>
const messages0 = [
  { role: 'human', text: `Create a new level scene called level_02 and set up the basic structure.` },
  { role: 'agent', text: `Scene created. Adding the node structure.` },
  { role: 'agent', text: `Scene structure created: World/TileMap, Enemies, Pickups, Lighting groups.` },
]
const messages1 = [
  { role: 'human', text: `Place all the tiles: floor and the three platforms.` },
  { role: 'agent', text: `5 tile operations completed: floor (41 tiles), platform A (5 tiles), platform B (5 tiles), platform C (5 tiles), goal platform (6 tiles, different atlas tile for visual distinction).` },
]
const messages2 = [
  { role: 'human', text: `Instance the enemy scene on platforms A and B, and add a coin pickup on the goal platform.` },
  { role: 'agent', text: `Enemy_A placed on platform A, Enemy_B on platform B. StarCoin_Goal placed on the goal platform.` },
]
const messages3 = [
  { role: 'agent', text: `Lighting setup complete: ambient DirectionalLight3D at 0.4 energy, plus a warm SpotLight3D over the goal platform.` },
]
const messages4 = [
  { role: 'human', text: `Set physics layers: world geometry on layer 5, enemies on layer 2, pickups on layer 6.` },
  { role: 'agent', text: `TileMap set to layer 5 (bitmask 16), mask 0 (does not detect anything — it is static geometry). Layer 5 named World. Enemies and pickups are configured per their own scene files.` },
]
const messages5 = [
  { role: 'human', text: `Level is running. Verify the structure and that the enemies and pickup are in the right positions.` },
  { role: 'agent', text: `Level02 snapshot:\\n- Enemy_A at [7.0, -2.0, 0.0] ✓ (platform A)\\n- Enemy_B at [16.0, -5.0, 0.0] ✓ (platform B)\\n- StarCoin_Goal at [32.0, -11.0, 0.0] ✓ (goal platform)\\n- GoalSpotlight at [32.0, -8.0, 0.0] ✓\\n\\nAll positions correct. TileMap shows as tracked static geometry.` },
  { role: 'human', text: `Can the player actually reach platform A from the floor?` },
  { role: 'agent', text: `Path distance: reachable=true, distance=7.2m. Platform A is navigable from the floor start position. (Note: path_distance uses navmesh — make sure NavigationRegion3D is baked over the level.)` },
]
</script>

# Build a Level

**Scenario**: Build a complete platformer level from scratch using Director. The level needs: floor tiles, floating platforms at three heights, enemy spawn points, a pickup area, collision shapes, and physics layers configured correctly.

This example demonstrates Director's batch operations for efficient level construction.

## The design

```
Height 0: Floor (columns 0-40)
Height -3: Platform A (columns 5-9) — low jump
Height -6: Platform B (columns 14-18) — medium jump
Height -9: Platform C (columns 22-26) — high jump
Height -12: Goal platform (columns 30-35) with pickup trigger
```

Enemies spawn on platforms A and B. A `OmniLight3D` illuminates the goal area.

## Step 1: Create the scene

<AgentConversation :messages="messages0" />

## Step 2: Place the tiles

<AgentConversation :messages="messages1" />

## Step 3: Place enemies and pickups

<AgentConversation :messages="messages2" />

## Step 4: Add lighting

<AgentConversation :messages="messages3" />

## Step 5: Configure physics layers

<AgentConversation :messages="messages4" />

## Step 6: Verify the built level

Press F5 to run the game. Then verify the structure:

<AgentConversation :messages="messages5" />

## Summary

This level was built in 6 steps, all through Director:
- Scene structure (1 batch, 5 nodes)
- Tile placement (1 batch, 5 fill operations, 62 tiles total)
- Enemy and pickup instances (1 batch, 3 instances)
- Lighting (1 batch, 4 operations)
- Physics layers (1 batch, 2 operations)
- Verification via Spectator (2 queries)

Total: 5 batch calls + 2 Spectator calls = 7 round-trips to build a complete level.
