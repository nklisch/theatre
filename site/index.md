---
layout: home
hero: false
---

<script setup>
const messages0 = [
  { role: 'human', text: `I just recorded a clip of the bug. The player clips through the east wall around frame 340.` },
  { role: 'agent', text: `Let me analyze the recording around that frame.` },
  { role: 'agent', text: `Found it. At frame 337, the player velocity spikes to 45 units/frame — the CharacterBody3D tunnels through the wall collision shape. The move_and_slide delta is too large for the CollisionShape3D thickness (0.3 units). Two fixes: thicken the collision shape to 1.0, or enable continuous collision detection.` },
]
</script>

<HeroSection />

## The Problem

AI coding agents can read your source files, set breakpoints, inspect variables — but they
**cannot see your game**. When an enemy clips through a wall, when a patrol path overshoots,
when physics bodies tunnel through geometry — your agent has no way to observe these problems.

It's like debugging a web app without being able to open the browser.

## Two Tools, One Stage

<div class="tool-cards">
<ToolCard
  title="Spectator"
  icon="🔭"
  description="Observe the running game. Spatial snapshots, real-time deltas, watches, recordings. Your AI sees what the player sees — as structured data."
  tool="9 MCP tools"
  tokens="200–3000"
  link="/spectator/"
/>
<ToolCard
  title="Director"
  icon="🎬"
  description="Build and modify scenes, resources, tilemaps, and animations through Godot's own API. No hand-editing .tscn files."
  tool="38+ operations"
  tokens="50–500"
  link="/director/"
/>
</div>

## How It Works

<ArchDiagram highlight="both" />

Theatre connects your AI agent to your Godot game through the
**Model Context Protocol (MCP)**. Spectator observes the running game via a
GDExtension addon. Director modifies scenes through the editor or headless Godot.
Both communicate over TCP, exposing structured tools your agent already knows
how to use.

## The Dashcam Moment

The killer workflow: **human plays, AI analyzes**.

<AgentConversation :messages="messages0" />

You press **F9** to mark the bug moment — the dashcam saves the last 60 seconds of spatial data — and the agent
scrubs through the spatial timeline to find exactly what went wrong — no
screenshots, no narration, no guessing from code.

## Real Debugging Scenarios

<div class="scenario-cards">
<ScenarioCard
  title="Physics Tunneling"
  icon="💥"
  problem="Fast-moving objects pass through walls. The code looks right but the collision just... doesn't happen."
  link="/examples/physics-tunneling"
/>
<ScenarioCard
  title="Pathfinding Failures"
  icon="🗺️"
  problem="NPCs get stuck, take bizarre routes, or refuse to move. The navmesh looks fine in the editor."
  link="/examples/pathfinding"
/>
<ScenarioCard
  title="Collision Layer Confusion"
  icon="🎭"
  problem="Two objects should collide but don't. Or they collide when they shouldn't. Layer/mask bits are a maze."
  link="/examples/collision-layers"
/>
<ScenarioCard
  title="Animation Sync Issues"
  icon="🎵"
  problem="The attack animation plays but the hitbox doesn't activate at the right frame. Timing is off."
  link="/examples/animation-sync"
/>
</div>

## Quick Start

### 1. Install Theatre

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo run -p theatre-cli -- install
```

### 2. Set up your Godot project

```bash
theatre init ~/your-godot-project
```

This copies addons, generates `.mcp.json`, and enables plugins — all interactively.

### 3. Install agent skills (optional)

```bash
skilltap install nklisch/theatre
```

Install skilltap from [skilltap.dev](https://skilltap.dev).

Teaches your agent how to use Spectator and Director effectively — tool selection, workflows, and pitfalls.

### 4. Run your game and ask

```
"Take a spatial snapshot of my scene"
```

Your AI agent now sees your game world — via MCP tools or CLI (`spectator spatial_snapshot '{"detail":"summary"}'`).
