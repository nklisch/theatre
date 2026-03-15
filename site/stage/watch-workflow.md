---
description: "Watch and React workflow — set up continuous monitoring with automatic alerts when spatial conditions are met."
---

<script setup>
const messages0 = [
  { role: 'human', text: `I want to monitor the player health, shield, and position during this combat encounter.` },
  { role: 'agent', text: `Let me set up watches on the relevant properties. What is the node path to the player?` },
  { role: 'human', text: `Just Player. It has health and shield as exported vars.` },
  { role: 'agent', text: `Baseline at frame 500. Now setting up the watches.` },
  { role: 'agent', text: `Watch created (watch_id: w_c8d9e0). Monitoring Player.position, .velocity, .health, and .shield. Start the combat encounter whenever you are ready.` },
]
const messages1 = [
  { role: 'human', text: `I just took some hits and health does not seem to be regenerating. Check it.` },
  { role: 'agent', text: `Delta from frame 500 to frame 847 (5.8 seconds):\\n\\n- health: 100 → 35 (3 damage events)\\n- shield: 50 → 0 (fully depleted at frame 612)\\n- position moved about 8 meters total\\n\\nNo regeneration events. Health only went down. Shield depleted at frame 612 and has not recovered. Expected regen at 5 hp/sec for 5 seconds would have added 25 hp — that should have brought health from 35 to 60 but health is still 35.` },
  { role: 'human', text: `Keep watching. I am out of combat now — wait 10 seconds and check again.` },
  { role: 'agent', text: `Delta from frame 847 to frame 1447 (10 seconds): health stayed at 35, no regen. Shield also still at 0. Position barely moved (standing still). The regen is definitely not triggering despite being out of combat for 10 seconds.` },
]
const messages2 = [
  { role: 'human', text: `Check the health regen controller node.` },
  { role: 'agent', text: `HealthRegenController state: regen_active=false, in_combat=true, regen_timer=0.0. Even though you are standing still with no enemies nearby, in_combat is still true. The combat exit timer is not running — regen_timer is stuck at 0. The HealthRegenController.in_combat flag is not being cleared when combat ends.` },
]
const messages3 = [
  { role: 'human', text: `Watch Enemy_0 position and velocity with a high threshold so I only see big changes.` },
  { role: 'agent', text: `Watch created (watch_id: w_f1a2b3). I will poll with min_distance_change: 0.5 to filter small movements.` },
  { role: 'human', text: `It has been patrolling for 20 seconds. Check.` },
  { role: 'agent', text: `Found a suspicious event at frame 1102: position jumped from [4.2, 0.0, 8.1] to [4.2, 0.0, 8.7] in one frame — a 0.6m teleport with velocity=0. That is not smooth movement. At frame 1101 velocity was [0,0,3.2] and at frame 1103 velocity is [0,0,2.8]. The waypoint transition at frame 1102 snapped the position instead of continuing movement smoothly.` },
]
</script>

# Watch & React Workflow

The Watch & React workflow is for situations where you want to **monitor nodes over time** rather than analyze a specific recorded moment. Instead of recording and reviewing, you set up watches and the agent observes the game as it runs.

## When to use watches instead of recordings

| Use recordings when... | Use watches when... |
|---|---|
| The bug already happened | You want to catch the bug as it happens |
| You need frame-precise timing | Coarse timing is sufficient |
| You want to scrub backward in time | You want real-time monitoring |
| The bug is intermittent and hard to trigger | The bug is happening continuously |
| Post-mortem analysis | Live investigation |

## The basic loop

```
1. spatial_snapshot          — get current frame number
2. spatial_watch (add)       — register nodes to monitor
3. [game runs, things happen]
4. spatial_delta             — check what changed
5. [repeat 3-4 as needed]
6. spatial_watch (remove)    — clean up
```

## Example: Monitoring player health during combat

You are debugging a health regen system that seems to not work correctly during certain combat conditions.

### Step 1: Set up the watches

<AgentConversation :messages="messages0" />

### Step 2: Check changes during combat

<AgentConversation :messages="messages1" />

### Step 3: Narrow down the cause

<AgentConversation :messages="messages2" />

The agent found that the `in_combat` flag never resets — likely a missing call to set it to `false` when the last enemy dies. The fix is in the combat state manager.

## Example: Watching enemy patrol behavior

You suspect enemies are teleporting slightly during patrol waypoint transitions.

<AgentConversation :messages="messages3" />

## Watching multiple nodes simultaneously

Set up watches on related nodes to catch interactions:

```json
{ "action": "add", "watch": { "node": "Player", "track": ["position", "health"] } }
{ "action": "add", "watch": { "node": "Boss", "track": ["position", "velocity", "phase", "health"] } }
{ "action": "add", "watch": { "node": "Boss/AttackHitbox", "track": ["monitoring"] } }
{ "action": "add", "watch": { "node": "BossArena/HazardTiles", "track": ["monitoring"] } }
```

Now a single `spatial_delta` call shows changes across all four nodes — you can see the Boss switch `phase` at a certain health threshold, the `AttackHitbox` become active, and the `HazardTiles` respond, all in one response.

## Delta poll frequency

You control how often to poll. The agent does not poll automatically — you ask for a delta when you want it.

Typical patterns:
- **Quick check**: poll once after a specific event ("I just died, check the delta")
- **Periodic**: poll every 10-30 seconds during a long session
- **Event-triggered**: poll after specific in-game events ("I just finished wave 3, check it")

There is no overhead from not polling — the watch just accumulates data. Poll whenever you want; all changes since the last poll are included.

## Cleaning up

```json
{ "action": "clear" }
```

Always clear watches when moving to a new investigation. Stale watches from a previous investigation add noise to subsequent deltas.

Or delete a specific watch:

```json
{ "action": "remove", "watch_id": "w_c8d9e0" }
```
