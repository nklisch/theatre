<script setup>
const messages0 = [
  { role: 'human', text: `I just recorded a clip where the enemy fails to detect the player. Bug was marked around frame 312. Can you investigate?` },
  { role: 'agent', text: `Let me look at the clip list and then examine what was happening around that frame.` },
  { role: 'agent', text: `Found clip_01 (412 frames, 14 seconds). Now checking the spatial data around frame 312.` },
  { role: 'agent', text: `Interesting. I can see the player position and the detection zone. Let me check the distance between them at frame 312.` },
]
const messages1 = [
  { role: 'agent', text: `At frame 312, the player is at global_position [4.1, 0.0, 2.8] and the EnemyDetectionZone center is at [4.0, 1.2, 2.9]. Distance: ~1.2 units — well within the detection radius. But the zone shows body_entered signal was NOT emitted. Let me inspect the detection zone more closely.` },
  { role: 'agent', text: `Found it. The EnemyDetectionZone has collision_layer=2 and collision_mask=2. But the Player CharacterBody3D has collision_layer=1. The Area3D can only detect bodies that are on its mask layers. Since the player is on layer 1 and the zone is masking layer 2, the overlap is never registered by the physics engine.` },
]
const messages2 = [
  { role: 'human', text: `So the zone is set up wrong? Can you check what the player collision layers are?` },
  { role: 'agent', text: `Confirmed. Player.collision_layer = 1 (binary: 00000001). EnemyDetectionZone.collision_mask = 2 (binary: 00000010). The mask does not include layer 1, so the physics engine never reports the overlap. The detection code itself is fine — it is never called because the signal is never emitted.` },
]
const messages3 = [
  { role: 'human', text: `Fix the collision mask on the detection zone. The player should be on layer 1 and the zone should detect layer 1.` },
  { role: 'agent', text: `Done. The EnemyDetectionZone.collision_mask is now 1 (layer 1). The Area3D will now register body_entered for the Player. Save and reload the scene to see the fix.` },
]
const messages4 = [
  { role: 'human', text: `Looks fixed. Can you confirm the layers are set correctly now in the running game?` },
  { role: 'agent', text: `Confirmed. collision_mask is now 1, which matches the player collision_layer of 1. The detection zone is correctly configured.` },
]
</script>

# Your First Debugging Session

This is a worked example of a complete Theatre debugging session. We will find and fix a real bug — an enemy that sometimes fails to detect the player — using Spectator's spatial tools with Claude Code.

## The setup

You have a 3D game with:
- A `CharacterBody3D` named `Player` with a movement script
- An `Area3D` named `EnemyDetectionZone` attached to an enemy, set to trigger enemy alert
- The bug: enemy sometimes does not alert when the player walks directly in front of it

You have already tried reading the detection script and the collision layer settings. The code looks correct. You need to see what is actually happening at runtime.

## Step 1: Reproduce the bug and save a clip

Stage's dashcam is always running — it automatically buffers the last 60 seconds of spatial data in memory. There is nothing to start.

Walk the player in front of the enemy a few times. On the third or fourth pass, the enemy fails to detect you — you see the player enters the zone visually but the alert animation does not play.

Press **F9** (or click the **⚑** flag button in the top-left corner of the game viewport) to save a clip of the bug moment. A toast notification confirms "Dashcam clip saved" in the top-right corner. The clip contains approximately 60 seconds of data before the trigger plus ~30 seconds of post-capture.

## Step 2: Ask the agent to analyze the clip

<AgentConversation :messages="messages0" />

## Step 3: The agent finds the anomaly

<AgentConversation :messages="messages1" />

## Step 4: Verify the diagnosis

<AgentConversation :messages="messages2" />

## Step 5: Fix it with Director

<AgentConversation :messages="messages3" />

## Step 6: Verify the fix

Press F5 to run the game with the fix applied. Walk the player in front of the enemy. The alert triggers every time.

To confirm with the agent:

<AgentConversation :messages="messages4" />

## What made this work

Without Theatre, this bug would have required:

1. Adding `print()` statements to the detection script
2. Re-running the game
3. Staring at console output and manually correlating frame numbers
4. Guessing that the issue was collision layers rather than the detection logic
5. Checking the collision layer settings manually in the inspector

With Theatre:

1. Mark the bug moment with F9 — the dashcam saves the clip automatically
2. Agent queries the spatial recording around the marked frame
3. Agent compares positions and inspects properties — finds the mismatch in < 60 seconds
4. Agent fixes it via Director without hand-editing the scene file

The key insight — that the `collision_mask` was wrong — came from the agent reading the **actual runtime property values**, not from reading the code. The code was correct; the configuration was wrong.

## Patterns to take away

**Mark bug moments with F9.** Pressing F9 (or clicking the in-game ⚑ flag button) saves a dashcam clip — not just a frame marker, but the full buffer including ~60 seconds of history before the trigger. This gives the agent data from before the bug began, not just the moment you noticed it.

**Use `spatial_inspect` after `clips`** to get property values at a specific moment. The clip recording captures position and velocity; inspect gives you the full property set.

**Let the agent compare two nodes.** Most spatial bugs are relational — wrong distance, wrong layer, wrong parent. Give the agent both nodes and ask it to compare.

**Use Director to fix without leaving the agent.** The full loop — observe, diagnose, fix, verify — happens inside your AI agent session without manually touching the editor.
