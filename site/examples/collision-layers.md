<script setup>
const messages0 = [
  { role: 'human', text: `Bullets are not hitting anything. Check all the collision layers across the relevant nodes.` },
  { role: 'agent', text: `Layer configuration:\\n\\n- Player (CharacterBody3D): layer=1, mask=3\\n- Enemy (CharacterBody3D): layer=2, mask=1\\n- PlayerBullet (Area3D): layer=4, mask=4\\n- EnemyProjectile (Area3D): layer=8, mask=2\\n\\nSpotted two issues immediately: (1) PlayerBullet has mask=4 — it only detects things on layer 4. No gameplay objects are on layer 4. It should have mask=2 to detect enemies. (2) EnemyProjectile has mask=2 — it detects enemies (layer=2), not the player (layer=1). It should have mask=1.` },
]
const messages1 = [
  { role: 'human', text: `Confirm that the bullet actually overlaps the enemy when it should be hitting.` },
  { role: 'agent', text: `Let me check what happens when a bullet is near an enemy.` },
  { role: 'agent', text: `Enemy is at 0.4m from the bullet. Physically overlapping. But since PlayerBullet.collision_mask=4 and Enemy.collision_layer=2, the physics engine does not check this overlap — the mask does not include layer 2. The Area3D body_entered signal will never fire.` },
]
const messages2 = [
  { role: 'human', text: `Fix all four nodes with the correct layer scheme: Player on layer 1, Enemies on layer 2, PlayerBullet on layer 3 detecting layer 2, EnemyProjectile on layer 4 detecting layer 1.` },
  { role: 'agent', text: `All 6 changes applied in one batch:\\n- Player: layer=1, mask=6 (detects enemies + projectiles)\\n- PlayerBullet: layer=3, mask=2 (detects enemies)\\n- EnemyProjectile: layer=4, mask=1 (detects player)` },
]
const messages3 = [
  { role: 'human', text: `Also set the layer names so this makes sense in the editor.` },
  { role: 'agent', text: `Layer names set. The Godot inspector now shows readable layer names instead of numbers — much easier to configure correctly going forward.` },
]
const messages4 = [
  { role: 'human', text: `Running the game now. Bullets hit enemies! Check the collision configuration is clean.` },
  { role: 'agent', text: `PlayerBullet: collision_layer=4 (PlayerBullets), collision_mask=2 (Enemies), monitoring=true. Configuration is correct. Enemy body_entered signal will fire when the bullet overlaps an Enemy.` },
]
</script>

# Collision Layer Confusion

**Scenario**: Player bullets do not hit enemies. Enemy projectiles do not hurt the player. The code handles `body_entered` and `area_entered` correctly, but those signals never fire.

This is the most common "why doesn't this collide?" problem in Godot. The physics layer/mask system is powerful but easy to misconfigure.

## Setup

- `Player`: `CharacterBody3D`, layer=1
- `Enemy`: `CharacterBody3D`, layer=2
- `PlayerBullet`: `Area3D` (the bullet hitbox), meant to detect enemies
- `EnemyProjectile`: `Area3D`, meant to detect the player

## Step 1: Observe the current layer configuration

<AgentConversation :messages="messages0" />

## Step 2: Confirm with relationship query

<AgentConversation :messages="messages1" />

## Step 3: Fix the layer configuration

<AgentConversation :messages="messages2" />

## Step 4: Set meaningful layer names

<AgentConversation :messages="messages3" />

## Step 5: Verify

<AgentConversation :messages="messages4" />

## The collision layer checklist

Use this as a mental model for any collision layer problem:

```
For two bodies to interact:
  A.collision_mask must include any bit from B.collision_layer
  OR
  B.collision_mask must include any bit from A.collision_layer
  (for Area3D + PhysicsBody: both conditions apply)
  (for PhysicsBody + PhysicsBody: either direction triggers)

For Area3D to detect a PhysicsBody:
  Area3D.monitoring must be true
  PhysicsBody.collision_layer AND Area3D.collision_mask ≠ 0

For Area3D to detect another Area3D:
  Both must have monitoring=true AND monitorable=true
  Areas' layer/mask must overlap in both directions
```

## Key takeaways

- **`scene_tree` with `show_properties` is the fastest audit tool.** One call showed all four nodes' layer/mask configuration simultaneously — no need for four separate inspects.
- **Layer 4 / mask 4 self-loop is a common mistake.** Setting both to the same value means the object only detects itself — useless and hard to spot without seeing the actual bitmask.
- **Set layer names early.** Named layers prevent future misconfiguration. "PlayerBullets" is unmistakable; "4" is not.
- **`batch` for multi-node layer fixes.** All six property changes applied in one round-trip.
