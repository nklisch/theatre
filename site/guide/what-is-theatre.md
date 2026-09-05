---
description: "Theatre is an AI agent toolkit that gives coding assistants spatial awareness of running Godot games via MCP tools."
---

# What is Theatre?

Theatre is an AI agent toolkit for Godot game engine. It bridges the gap between what your AI coding agent can read — source files, logs, error messages — and what it needs to truly understand your game: **spatial data from the running engine**.

## The Problem

Modern AI coding agents are genuinely useful for game development. They can read your GDScript, suggest fixes for logic bugs, refactor node structures, and write boilerplate. But there is a fundamental limitation: **they cannot see your game running**.

When you describe a bug like "the enemy clips through the wall sometimes," your agent has no way to observe it directly. It can only:

- Read collision shape values from source files
- Guess at runtime conditions from code paths
- Make suggestions based on common patterns

It cannot tell you *when* the tunneling happens, *which* wall, *what* the velocity was at the moment of failure, or *whether* its proposed fix actually works. Every iteration is blind.

This is analogous to debugging a web application without being able to open a browser, or debugging a database without being able to run a query. You are reasoning about runtime behavior entirely from static artifacts.

## The Solution

Theatre adds two MCP servers to your Godot workflow, exposing structured runtime data to your AI agent via the **Model Context Protocol**.

Your agent gains the ability to:

- Query spatial positions, velocities, and properties of every tracked node
- Watch specific nodes for changes over time
- Scrub through recorded gameplay to find the exact frame a bug occurs
- Create and modify scenes, tilemaps, animations, and resources

The agent can combine **structured engine data** with a bounded current viewport
image. The structured data explains positions, properties, relationships, and
physics. Pixels show layout, lighting, occlusion, and appearance.

## Two Tools, One Stage

### Stage

Stage is a live observation and interaction tool for running Godot games. It consists of:

- **A Rust GDExtension addon** (`addons/stage/`) that runs inside your game and collects spatial data from the scene tree on every physics tick. It listens for incoming TCP connections on port 9077.
- **A Rust MCP server + CLI** (`stage`) that connects to the addon and exposes live identity, diagnostics, viewport, spatial, action, clip, and feedback tools through MCP or one-shot CLI calls.

Stage answers questions like:

- "Where is the player right now?"
- "How fast is the projectile moving when it hits that wall?"
- "Which nodes are within 5 meters of the enemy?"
- "What changed between frame 300 and frame 340 during the bug recording?"

Stage can also **act on the running game** — setting properties, calling methods, and emitting signals — so the agent can test hypotheses and verify fixes without restarting. These changes are ephemeral (not saved to disk); use Director for permanent modifications.

### Director

Director is a write tool for Godot scenes and resources. It consists of:

- **A GDScript addon** (`addons/director/`) that runs as an editor plugin or headless daemon
- **A Rust MCP binary** (`director`) that routes operations to the appropriate backend

Director answers requests like:

- "Create a CharacterBody3D scene with a CollisionShape3D and a CapsuleShape3D"
- "Set the collision layer on the enemy node to layer 3"
- "Fill these 10 tile coordinates with tile ID 5 in the main TileMap"
- "Create an animation that bounces the node from y=0 to y=2 over 0.5 seconds"

Director authors project content through Godot's own API. Open-scene changes use
native undo and remain unsaved until an explicit selected-scene save. Through a
verified open editor, Director can also start, stop, and restart a selected saved
scene without implicitly saving unrelated work.

Together, Stage and Director support an author, save, run, observe, act, and
verify loop. Stage runtime changes remain temporary. Durable changes belong in
source code or Director-authored resources.

## Who Is Theatre For?

Theatre is designed for developers who:

**Use AI agents (Claude Code, Cursor, etc.) for game development** and want those agents to be genuinely effective at runtime debugging, not just code suggestion.

**Debug spatial/physics issues** where reading code is insufficient — tunneling, navmesh failures, collision layer mismatches, animation timing problems.

**Want to automate scene construction** — building levels, configuring physics layers, wiring signals — and have the agent verify the result by actually running the game.

**Share a hard-to-describe observation** from the running game or editor with
viewport, selection or pointer context, and an optional note.

Theatre does not require one specific AI client. Its primary integration is MCP.
Optional native Claude and Codex packages can announce pending feedback at a
later tool boundary after explicit installation and trust.

## Design Philosophy

**Use state and pixels for different questions.** Engine structures provide exact
positions, velocities, and collision layers. Current viewport images and retained
clip images show visual results that those values cannot express.

**Thin engine boundary, smart server.** The GDExtension gathers engine-owned data
and executes engine operations. Spatial reasoning, diffing, budgeting, and
indexing stay in the Rust server. Capture-local buffering and clip persistence
stay with the recorder because they must continue without an agent session.

**Token budgets first.** Spatial snapshots can be enormous. Every tool that returns scene data accepts a `token_budget` parameter and a `detail` level. Theatre will never blow up your context window with a 500-node scene dump when you only needed the player's position.

**Capture only the evidence you need.** Use a summary snapshot for current state,
`viewport` for the latest completed render, a marked clip for temporal evidence,
or **Share feedback** when a developer needs to attach context and a note. These
surfaces complement one another.
