# Examples

Real debugging and development scenarios using Theatre. Each example is a complete story — from problem identification through diagnosis to fix.

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
  problem="The flagship workflow: Director builds a room, Spectator verifies a player can navigate it. Full loop."
  link="/examples/build-verify"
/>
</div>

## What these examples teach

Each example is written as a real session transcript — you can see exactly what tool calls the AI agent makes, in what order, and why. The pattern across all examples is the same:

1. **Observe** — spatial_snapshot, scene_tree, or recording to understand current state
2. **Narrow** — spatial_inspect, spatial_query, or recording query_range to focus on the problem
3. **Diagnose** — read property values, compare nodes, analyze the spatial timeline
4. **Fix** — Director operation or source code edit
5. **Verify** — spatial_inspect or snapshot to confirm the fix

The tools are always the same. What differs is which tool surfaces the evidence for each category of bug.
