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
| **F8** | Start recording (or start a new recording if one is active) |
| **F9** | Mark the current frame as a bug moment |
| **F10** | Stop recording |

These shortcuts are active while the game is running. They are handled by the Spectator editor dock — they work whether you are focused on the game window or the Godot editor.

You can also trigger these via the Spectator dock UI: **Start**, **Mark**, **Stop** buttons.

## A complete debugging session

Here is a real debugging story, from start to finish.

### The bug

You have a stealth game. Enemies have a detection cone — an `Area3D` shaped roughly like a forward-facing triangle. When the player enters the cone, the enemy alerts. But sometimes, enemies that should clearly see the player do not alert. You cannot reproduce it reliably.

### Step 1: Enable continuous recording

Start the game, then press **F8** immediately. The dock shows "Recording: clip_stealth_01". Now every physics frame is captured.

Play normally. Move around. Do some stealth sections. Wait for the bug to occur.

After about 45 seconds of play, you see it: you walk directly in front of an enemy at close range, the enemy's eyes do not move, no alert. You immediately press **F9**. The dock shows "Marker: frame 2712 — enemy_missed_player".

Continue playing for a few more seconds, then press **F10** to stop.

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

1. **Always-on recording**: because you started recording before the bug, you have data from the moment it began, not just after you noticed it
2. **Frame marker**: pressing F9 gave the agent a precise frame to anchor the search
3. **Condition filtering**: querying only frames where `monitoring == false` found the transition in one call instead of scanning 2,800 frames manually
4. **Temporal reasoning**: the agent found that the flag changed 0.9 seconds *before* the visible failure — the root cause was upstream of the symptom

## Workflow variations

### Quick investigation (no marker)

If the bug is obvious and repeatable, you do not need a marker:

1. F8 to start
2. Trigger the bug
3. F10 to stop
4. Ask the agent: "The bug happens around the end of the recording. Find the anomaly."

### Continuous background recording

Enable continuous recording in the Spectator config so recording always starts with the game:

```json
{
  "auto_record": true,
  "max_clip_duration_s": 120
}
```

Now every session is automatically captured. You only need F9 to mark bug moments.

### Post-playtest analysis

After a playtest session where you were not at the keyboard:

1. Have the tester start recording when they start playing
2. Have them press F9 whenever something seems wrong
3. Collect the clip files afterward
4. Ask the agent to analyze all markers in the clip

### Regression testing

Record a "golden run" of expected behavior. Record a later run where something changed. Ask the agent to compare the two clips for anomalies.
