<script setup>
const messages0 = [
  { role: 'human', text: `I recorded several bullets. Some went through the wall. Find the tunneling events.` },
  { role: 'agent', text: `Found clip_bullet_test (3,200 frames, 53 seconds). Querying for frames where bullet velocity was high near the wall.` },
  { role: 'agent', text: `Found 47 frames with bullet velocity > 50 units/sec. Now finding frames where the bullet is near the wall.` },
]
const messages1 = [
  { role: 'agent', text: `Found it. At frame 823: Bullet at [8.1, 1.2, 0.0], velocity=[80.1, 0.0, 0.0]. Wall_East at [9.0, 1.5, 0.0] (thickness 0.3, so it spans x=8.85 to x=9.15). Frame 824: Bullet at [9.43, 1.2, 0.0], velocity=[80.1, 0.0, 0.0] — the bullet jumped from x=8.1 to x=9.43 in one frame (1.33m), completely skipping the wall (x=8.85 to x=9.15). No collision was registered.` },
]
const messages2 = [
  { role: 'agent', text: `The physics math confirms tunneling: at 80 units/sec, 60Hz physics, each tick the bullet moves 80/60 = 1.33 meters. The wall is 0.3m thick. The bullet can cross the wall entirely between two physics frames, starting at 8.1 (outside wall) and ending at 9.43 (outside wall on the other side), never being inside the wall at a sample point. This is a classic thin-wall tunneling problem.` },
  { role: 'human', text: `What are my options to fix this?` },
  { role: 'agent', text: `Three options: (1) Enable motion_mode="float" and increase wall thickness to at least 1.5m — most robust. (2) Use continuous collision detection by switching Bullet from CharacterBody3D to a RigidBody3D with continuous_cd=true — better for physics-based bullets. (3) Implement a manual raycast in the bullet script: each tick, cast a ray from the previous position to the current position and check for hits — most flexible.` },
]
const messages3 = [
  { role: 'human', text: `I applied the raycast fix. Fire more bullets and verify no tunneling.` },
  { role: 'agent', text: `Ready. Fire bullets at the wall and press F9 when done — I will analyze the clip.` },
  { role: 'human', text: `Done. Bullets all hit the wall correctly now.` },
  { role: 'agent', text: `No frames where the bullet was at high velocity and past the wall. All bullets appear to have stopped at the wall face or within 0.1m of it. The fix is working.` },
]
</script>

# Physics Tunneling

**Scenario**: Fast-moving bullets pass through thin walls. Sometimes they hit, sometimes they go straight through. The collision code is correct.

## Setup

- `Bullet`: `CharacterBody3D` with a `SphereShape3D` (radius 0.05) moving at 80 units/second
- `Wall_East`: `StaticBody3D` with a `BoxShape3D` (size [0.3, 3.0, 5.0]) — 30cm thick
- The bullet moves via `move_and_slide()` in `_physics_process`

At 80 units/second and 60Hz physics, the bullet moves **1.33 meters per tick**. The wall is **0.3 meters thick**. The bullet travels 4.4x the wall's thickness in a single physics step.

Godot's `move_and_slide` does not use continuous collision detection by default. If the bullet starts outside the wall and ends outside the wall in one tick, the engine reports no collision — even if the wall was in between.

## Step 1: Reproduce with recording

Fire several bullets at the wall at different angles and distances. When you see a tunneling event, press **F9** to save the clip. The dashcam captures the last 60 seconds plus ~30 seconds of post-capture, so you do not need to press it immediately.

## Step 2: Query the recording

<AgentConversation :messages="messages0" />

## Step 3: Find the tunneling frame

<AgentConversation :messages="messages1" />

## Step 4: Confirm the diagnosis

<AgentConversation :messages="messages2" />

## Step 5: Apply the fix (Option 3 — manual raycast)

The bullet script gets a raycast check:

```gdscript
# In Bullet._physics_process()
var prev_position = global_position
move_and_slide()

# Check if we tunneled through anything
var space = get_world_3d().direct_space_state
var query = PhysicsRayQueryParameters3D.create(prev_position, global_position)
query.collision_mask = collision_mask
var result = space.intersect_ray(query)
if result:
    # Handle the hit at result.position
    _on_hit(result.collider, result.position, result.normal)
    queue_free()
```

## Step 6: Verify the fix

<AgentConversation :messages="messages3" />

## Key takeaways

- **Tunneling is a per-tick displacement problem**, not a code bug. The physics code is correct; the geometry is too thin for the speed.
- **The clip recording made this diagnosable.** Without frame-by-frame position data, you would only know "sometimes it goes through" — not the exact displacement that causes it.
- **`velocity_above` conditions are powerful for physics bugs.** They filter thousands of frames down to the ones where speed is high enough to cause tunneling.
- **Three valid fixes** exist; the right choice depends on your game type. Manual raycasts are the most flexible for custom bullet behavior.
