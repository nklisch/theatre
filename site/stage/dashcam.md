<script setup>
const messages0 = [
  { role: 'human', text: `I captured the detection miss. The marker is around frame 2712. Can you figure out what happened?` },
  { role: 'agent', text: `Let me pull the clip list and then examine what was happening around that frame.` },
  { role: 'agent', text: `Found clip_stealth_01: 2,890 frames (48 seconds) with marker at frame 2712 labeled enemy_missed_player. Querying the 30-frame window around the marker.` },
]
const messages1 = [
  { role: 'agent', text: `Interesting. At frame 2712: Player is at [4.2, 0.0, -1.3]. Enemy_3 is at [4.5, 0.0, -5.2] facing direction implied by rotation_deg [0, 3.8, 0] — almost dead ahead toward the player. Distance: 3.9 meters. That is within detection range. But Enemy_3/DetectionCone shows monitoring=false at this frame.` },
]
const messages2 = [
  { role: 'agent', text: `Wait — monitoring was false. Let me check when it became false.` },
  { role: 'agent', text: `Found it. monitoring became false at frame 2658, 54 frames (0.9 seconds) before the marker. At frame 2657 it was true; at frame 2658 it flipped to false. That is right when Enemy_3 was in the Patrol state. The patrol script must be turning off monitoring when in patrol state — but that was never meant to disable detection, only animation.` },
]
const messages3 = [
  { role: 'human', text: `So the patrol script is disabling the detection zone?` },
  { role: 'agent', text: `Yes. In enemy_patrol.gd, there is a line in the _on_patrol_state_entered function: detection_cone.monitoring = false. This was probably added to avoid false triggers during patrol transitions, but it permanently disables the cone until the enemy re-enters alert state. The fix: remove that line, or add detection_cone.monitoring = true in _on_patrol_state_exited.` },
]
const messages4 = [
  { role: 'human', text: `Fix the script.` },
  { role: 'agent', text: `The fix needs to be in the GDScript source, not a scene property change. Let me find and edit the file.` },
]
</script>

# The Dashcam Workflow

The dashcam workflow is Theatre's flagship debugging pattern. It mirrors how dashcams work in cars: always recording, you review the footage only when something happens.

## The idea

You play your game normally. When a bug occurs, you press a key to mark the moment. Later, your AI agent scrubs through the spatial recording to find what happened — velocities, positions, property states — at the exact frame the bug occurred.

You do not need to narrate the bug to the agent. You do not need screenshots. You do not need to add print statements and replay. The agent reads the spatial timeline directly.

## The keyboard shortcuts

| Key | Action |
|---|---|
| **F9** | Mark the current frame as a bug moment |
| **F11** | Pause / unpause the game |

These shortcuts are active while the game is running. They are handled by the StageRuntime autoload.

You can also click the **⚑** flag button in the top-left corner of the game viewport. From the agent side, use the `clips` tool's `"add_marker"` action to trigger a clip save, or `"save"` to force-flush the current buffer.

## A complete debugging session

Here is a real debugging story, from start to finish.

### The bug

You have a stealth game. Enemies have a detection cone — an `Area3D` shaped roughly like a forward-facing triangle. When the player enters the cone, the enemy alerts. But sometimes, enemies that should clearly see the player do not alert. You cannot reproduce it reliably.

### Step 1: Play the game

Start the game. The dashcam begins buffering automatically — you will see a small "● Dashcam: buffering" label in the top-left corner of the game viewport. Every physics frame of spatial data is captured into a 60-second rolling buffer.

Play normally. Move around. Do some stealth sections. Wait for the bug to occur.

After about 45 seconds of play, you see it: you walk directly in front of an enemy at close range, the enemy's eyes do not move, no alert. You immediately press **F9** (or click the **⚑** flag button). The dashcam saves the last 60 seconds plus the next ~30 seconds as a clip. A toast confirms: "Dashcam clip saved".

### Step 2: Start the investigation

<AgentConversation :messages="messages0" />

### Step 3: Query the relevant window

<AgentConversation :messages="messages1" />

### Step 4: Find the cause

<AgentConversation :messages="messages2" />

### Step 5: Confirm with source

<AgentConversation :messages="messages3" />

### Step 6: Apply the fix

<AgentConversation :messages="messages4" />

The agent opens `enemy_patrol.gd`, removes the `monitoring = false` line, and saves. On the next test run, the detection works correctly.

## What made this work

1. **Always-on dashcam**: because the buffer was running before the bug, you have data from the moment it began
2. **Frame marker**: pressing F9 gave the agent a precise frame to anchor the search
3. **Condition filtering**: querying only frames where `monitoring == false` found the transition in one call instead of scanning 2,800 frames manually
4. **Temporal reasoning**: the agent found that the flag changed 0.9 seconds *before* the visible failure — the root cause was upstream of the symptom

## Workflow variations

### Quick investigation (no marker)

If the bug is obvious and repeatable:

1. Trigger the bug in the running game
2. Press **F9** to save the clip
3. Ask the agent: "Something went wrong in the last few seconds. Analyze the latest clip."

### Longer buffer windows

The default dashcam buffer holds 60 seconds of pre-trigger history. To extend it, configure the dashcam via the `clips` tool's `status` action or project settings:

- `pre_window_deliberate_sec`: seconds of history before a human/agent marker (default: 60)
- `byte_cap_mb`: memory limit for the ring buffer (default: 1024 MB)

### Post-playtest analysis

After a playtest session:

1. Have the tester play normally (the dashcam is always running)
2. Have them press **F9** whenever something seems wrong
3. Collect the clip files afterward
4. Ask the agent to analyze all markers across clips

### Regression testing

Record a "golden run" of expected behavior. Record a later run where something changed. Ask the agent to compare the two clips for anomalies.
