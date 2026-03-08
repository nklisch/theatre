# Spectator — Vision

## One-Liner

Spectator gives AI agents spatial awareness of your running Godot game — where DAP tells you what the code is doing, Spectator tells you what the world is doing.

## The Problem

AI coding agents are blind to game state. They can read your source files, set breakpoints, inspect variables — but they cannot *see* your game. When an enemy clips through a wall, when a patrol path overshoots, when a physics body tunnels through geometry — the agent has no way to observe these problems spatially. It's like debugging a web app without being able to open the browser.

Game bugs are fundamentally spatial. They happen in world space: wrong positions, missed collisions, broken pathfinding, incorrect line-of-sight. Diagnosing them requires understanding *where things are and how they relate to each other* — not just what values variables hold.

Today, the debugging loop is:

1. Human sees a bug in the running game
2. Human describes it to the agent in natural language ("the enemy walks through the east wall")
3. Agent guesses at the cause from code alone
4. Agent suggests a fix
5. Human tests it, reports back
6. Repeat

This loop is slow, lossy, and fundamentally limited by the human's ability to translate spatial observations into text.

## The Solution

Spectator makes the agent a **spatial collaborator**. It provides a structured, token-efficient view of the running game's spatial state through MCP (Model Context Protocol), giving any compatible AI agent the ability to:

- **See** the scene — positions, distances, bearings, velocities, spatial relationships
- **Query** specific spatial questions — "what's near the player?", "can this enemy see that door?", "what's the navmesh distance?"
- **Watch** for changes — subscribe to nodes or conditions, get notified when something interesting happens
- **Inspect** deeply — drill into any node's full state, physics, signals, resources, children
- **Act** for debugging — pause, teleport nodes, change properties, advance frames, reproduce conditions
- **Record and analyze** — the human reproduces a bug while Spectator captures a frame-by-frame timeline; the agent scrubs through it to diagnose what went wrong

The agent doesn't need screenshots, doesn't need the human to narrate, doesn't need to guess from code. It has direct, structured access to the game's spatial reality.

## Design Principles

### 1. Spatial-First

Everything is organized around *where things are in space*, not how they're represented in code. Positions, distances, bearings, proximity, line-of-sight, areas — these are the primitives. The agent thinks about the game the way a player does: spatially.

### 2. Token-Efficient

LLM context windows are finite and expensive. Every response is budgeted. Summary views are cheap (~200 tokens). The agent drills deeper only when needed. Pagination prevents blowouts. The system is designed so a typical debugging session costs ~1200 tokens of spatial data across multiple tool calls — not thousands of tokens dumped in one response.

### 3. Human-Agent Collaboration

The human and agent are partners. The human drives the game — they know how to reproduce the bug, when it happens, what looks wrong. The agent analyzes — it can scrub timelines, compute spatial relationships, cross-reference collision layers and navmesh edges. The dashcam captures context automatically around interesting moments; the agent scrubs through saved clips to diagnose what went wrong.

### 4. Observational by Default

Spectator is a *debugger*, not a game controller. It observes without affecting the game unless the agent explicitly uses the action tools. The addon running inside Godot should never alter gameplay behavior just by being present. Actions (teleport, set_property, pause) are opt-in debugging operations, not gameplay automation.

### 5. LLM-Agnostic

Spectator uses MCP, an open protocol. It works with any MCP-compatible client: Claude Code, Cursor, Windsurf, or any future tool. The spatial data is structured for LLM consumption (bearings like "ahead_left" alongside exact degrees) but doesn't assume a specific model or client.

### 6. Godot-Native

The addon should feel like a natural part of the Godot editor. The dock panel uses Godot's UI conventions. Keybindings are familiar. Configuration lives where Godot developers expect it. The addon doesn't fight the engine — it extends it.

## What Spectator Is Not

- **Not a code debugger.** DAP (Debug Adapter Protocol) handles breakpoints, stack frames, variable inspection, stepping through code. Spectator handles the spatial world. They're complementary — use both.
- **Not a game engine.** Spectator doesn't run game logic, simulate physics, or render frames. It observes the engine that does.
- **Not a testing framework.** It doesn't assert, doesn't pass/fail, doesn't run suites. It's an interactive debugging tool for spatial problems.
- **Not a visual editor.** The agent doesn't see rendered pixels. It sees structured spatial data — positions, relationships, properties. This is by design: structured data is what LLMs reason about effectively.
- **Not multiplayer-aware.** Network replication state, authority, client/server scene trees — these are explicitly out of scope. Spectator observes a single local scene tree.

## The North Star

A Godot developer opens their project, enables the Spectator addon, and starts a conversation with their AI agent. The agent can immediately understand the running game's spatial state as naturally as it reads source code. When a bug involves space — collision, pathfinding, positioning, physics, spatial logic — the agent diagnoses it directly from observation, not from guesswork. The human and agent work as a team: human reproduces, agent analyzes, together they fix.

The spatial view becomes as fundamental to AI-assisted game development as the text editor view is to AI-assisted programming.

## Relationship to the Ecosystem

```
┌──────────────────────────────────────────────────────┐
│                    AI Agent                          │
│              (Claude Code, Cursor, etc.)             │
└──────┬───────────────────┬───────────────────┬───────┘
       │                   │                   │
  Read/Write          Spectator (MCP)      DAP / Agent Lens
  Source Code          Spatial State        Code Debugging
       │                   │                   │
  ┌────▼────┐      ┌──────▼──────┐      ┌─────▼─────┐
  │  Files  │      │   Running   │      │ Debugger  │
  │  .gd    │      │   Game      │      │ Breakpts  │
  │  .tscn  │      │   World     │      │ Stack     │
  └─────────┘      └─────────────┘      └───────────┘
```

Three complementary views of a Godot project:
1. **Source code** — what the developer wrote (files)
2. **Spatial state** — what the game world looks like right now (Spectator)
3. **Code execution** — what the code is doing right now (DAP)

Spectator fills the gap between static code and runtime execution — the *spatial runtime* that exists only when the game is running.
