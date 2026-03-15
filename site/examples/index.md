---
description: "Real debugging scenarios with full AI session transcripts — physics tunneling, pathfinding, collision layers, and more."
---

# Examples

Real debugging and development scenarios using [Theatre](/guide/what-is-theatre), an AI agent toolkit for Godot. Each example is a complete story — from problem identification through diagnosis to fix — written as a real AI agent session transcript. You can see exactly which MCP tools the agent calls, in what order, and why.

Every scenario uses a real Godot project as the debugging target. The agent connects via Theatre's Stage tools to observe spatial data from the running game, then uses that data to diagnose and fix the issue — no screenshots, no guessing from code alone.

## Debugging Examples

<div class="scenario-cards">
<ScenarioCard
  title="Physics Tunneling"
  icon="💥"
  problem="Fast-moving objects pass through walls. The code looks right but the collision just doesn't happen."
  link="/examples/physics-tunneling"
/>
<ScenarioCard
  title="Pathfinding Failures"
  icon="🗺️"
  problem="NPCs get stuck, take bizarre routes, or refuse to move at all. The navmesh looks fine in the editor."
  link="/examples/pathfinding"
/>
<ScenarioCard
  title="Animation Sync Issues"
  icon="🎵"
  problem="The attack animation plays but the hitbox doesn't activate at the right frame. Timing is off."
  link="/examples/animation-sync"
/>
<ScenarioCard
  title="Collision Layer Confusion"
  icon="🎭"
  problem="Two objects should collide but don't. Or they collide when they shouldn't. Layer/mask bits are a maze."
  link="/examples/collision-layers"
/>
<ScenarioCard
  title="UI Overlap Issues"
  icon="🖥️"
  problem="UI elements overlap or go off-screen on different resolutions. Anchor logic is hard to reason about."
  link="/examples/ui-overlap"
/>
</div>

## Build Examples

<div class="scenario-cards">
<ScenarioCard
  title="Build a Level"
  icon="🏗️"
  problem="Start from scratch and build a complete platformer level using Director's scene, node, and TileMap tools."
  link="/examples/build-level"
/>
<ScenarioCard
  title="Build & Verify"
  icon="✅"
  problem="The flagship workflow: Director builds a room, Stage verifies a player can navigate it. Full loop."
  link="/examples/build-verify"
/>
</div>

## What these examples teach

Each example follows the same five-step debugging methodology. The tools are always the same — what differs is which tool surfaces the evidence for each category of bug.

| Step | Purpose | Typical Tools |
|------|---------|---------------|
| **Observe** | Understand current game state | `spatial_snapshot`, `scene_tree`, recording playback |
| **Narrow** | Focus on the problem area | `spatial_inspect`, `spatial_query`, `clips query_range` |
| **Diagnose** | Read property values, compare nodes, analyze timeline | `spatial_inspect`, `spatial_delta`, `spatial_watch` |
| **Fix** | Apply the correction | Director operations or source code edits |
| **Verify** | Confirm the fix works | `spatial_snapshot`, `spatial_inspect` |

## Tools used across examples

| Example | Primary Tools | Key Insight |
|---------|---------------|-------------|
| Physics Tunneling | Recording, `spatial_inspect` | Velocity spike exceeds collision shape thickness |
| Pathfinding | `spatial_query`, `scene_tree` | NavMesh region gaps or agent radius mismatches |
| Animation Sync | `spatial_watch`, `spatial_action` | Track keyframe timing against hitbox activation |
| Collision Layers | `spatial_inspect` | Layer and mask bit comparison across node pairs |
| UI Overlap | `spatial_query`, `spatial_inspect` | Control node rect overlaps and z-index ordering |
| Build a Level | Director `scene`, `node`, `tilemap` ops | End-to-end level construction from scratch |
| Build & Verify | Director + Stage loop | Build with Director, verify with Stage snapshots |
