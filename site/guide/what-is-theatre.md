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

The agent does not see a screenshot. It sees **structured data** — the same data you would read from Godot's debugger, but accessible programmatically, queryable, and integrated into the agent's reasoning loop.

## Two Tools, One Stage

### Spectator

Spectator is a read-only observation tool for running Godot games. It consists of:

- **A Rust GDExtension addon** (`addons/spectator/`) that runs inside your game and collects spatial data from the scene tree on every physics tick. It listens for incoming TCP connections on port 9077.
- **A Rust MCP server + CLI** (`spectator`) that connects to the addon and exposes 9 tools to your AI agent — via MCP (`spectator serve`) or CLI (`spectator <tool> '<json>'`).

Spectator answers questions like:

- "Where is the player right now?"
- "How fast is the projectile moving when it hits that wall?"
- "Which nodes are within 5 meters of the enemy?"
- "What changed between frame 300 and frame 340 during the bug recording?"

Spectator never modifies the game state. It is purely observational.

### Director

Director is a write tool for Godot scenes and resources. It consists of:

- **A GDScript addon** (`addons/director/`) that runs as an editor plugin or headless daemon
- **A Rust MCP binary** (`director`) that routes operations to the appropriate backend

Director answers requests like:

- "Create a CharacterBody3D scene with a CollisionShape3D and a CapsuleShape3D"
- "Set the collision layer on the enemy node to layer 3"
- "Fill these 10 tile coordinates with tile ID 5 in the main TileMap"
- "Create an animation that bounces the node from y=0 to y=2 over 0.5 seconds"

Director never runs the game. It modifies the project files on disk through Godot's own API, so the changes are valid and immediately visible in the editor.

## Who Is Theatre For?

Theatre is designed for developers who:

**Use AI agents (Claude Code, Cursor, etc.) for game development** and want those agents to be genuinely effective at runtime debugging, not just code suggestion.

**Debug spatial/physics issues** where reading code is insufficient — tunneling, navmesh failures, collision layer mismatches, animation timing problems.

**Want to automate scene construction** — building levels, configuring physics layers, wiring signals — and have the agent verify the result by actually running the game.

**Build AI-driven gameplay features** where the agent needs to observe game state to make decisions — procedural level adjustment, automated QA, dynamic balancing.

Theatre does not require any specific AI agent. It uses MCP (Model Context Protocol), which is supported by Claude Code, Cursor, Windsurf, and any other agent that supports MCP tool servers.

## Design Philosophy

**Agents see data, not pixels.** Screenshots require vision models and lose precision. Theatre exposes the engine's own data structures — positions as `Vector3`, velocities as `Vector3`, collision layers as bitmasks. Agents reason over numbers, not images.

**Thin addon, smart server.** The GDExtension addon does as little as possible — it collects raw data and sends it over TCP. All spatial reasoning, diffing, budgeting, and indexing happens in the Rust server. This keeps the addon stable across Godot versions and keeps game performance impact minimal.

**Token budgets first.** Spatial snapshots can be enormous. Every tool that returns scene data accepts a `token_budget` parameter and a `detail` level. Theatre will never blow up your context window with a 500-node scene dump when you only needed the player's position.

**No screenshots required.** The workflow is: click **Record** in the dock, mark bugs with **F9**, then click **Stop** and ask your agent to analyze the clip. The agent scrubs the spatial timeline, finds the exact frame, diagnoses the cause, and suggests a fix — all from structured data.
