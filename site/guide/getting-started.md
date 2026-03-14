# Getting Started

This guide walks you through getting your first `spatial_snapshot` in under 10 minutes. We assume you have completed [Installation](/guide/installation).

## The goal

By the end of this guide, your AI agent will be able to call `spatial_snapshot` and receive real position data from your running Godot game. We will not cover all tools or workflows — just the minimum viable setup.

## Step 1: Run your game

Theatre's Stage tool only works while a Godot game is running. The GDExtension starts its TCP listener when the scene tree initializes, and shuts it down on exit.

Press **F5** (or the play button) in Godot to run your project. Any scene will work — even an empty one with a single Node3D.

In the Godot output panel, you should see something like:

```
[Stage] TCP server started on port 9077
[Stage] Collecting 3 nodes (physics tick 0)
```

If you do not see this, confirm the Stage addon is enabled in **Project → Project Settings → Plugins**.

## Step 2: Ask for a snapshot

In your AI agent (Claude Code, Cursor, etc.), type:

```
Take a spatial snapshot of my scene.
```

The agent will call the `spatial_snapshot` MCP tool. After a moment, you will see a response like:

```json
{
  "frame": 47,
  "timestamp_ms": 1847,
  "node_count": 8,
  "summary": {
    "player": {
      "class": "CharacterBody3D",
      "global_position": [2.3, 0.0, -1.7],
      "velocity": [0.0, 0.0, 0.0]
    },
    "camera": {
      "class": "Camera3D",
      "global_position": [2.3, 1.8, 0.3]
    },
    "ground": {
      "class": "StaticBody3D",
      "global_position": [0.0, -0.5, 0.0]
    }
  }
}
```

The exact content depends on your scene. The agent now has accurate, real-time spatial data from the running game.

## Step 3: Ask a follow-up question

Now that the agent has observed your scene, you can ask questions that depend on spatial context:

```
Where is the player relative to the ground? Is the player grounded?
```

The agent will use the snapshot data (or call `spatial_inspect` on the player node) to answer:

```
The player's global_position.y is 0.0. The ground's global_position.y is -0.5
with a StaticBody3D, so the player is resting on the ground surface. The
CharacterBody3D.is_on_floor() property would confirm this at runtime.
```

## Step 4: Try a spatial query

The `spatial_query` tool lets you ask geometric questions. Try:

```
What nodes are within 5 meters of the player?
```

The agent will call:

```json
{
  "type": "radius",
  "origin": "player",
  "radius": 5.0
}
```

And return a list of nearby nodes with their distances — useful for debugging enemy detection ranges, item pickup areas, or trigger zones.

## What's next?

You now have a working Theatre setup. Here are the natural next steps:

- **[Your first debugging session](/guide/first-session)** — A complete worked example of finding and fixing a real bug.
- **[Recording workflow](/stage/dashcam)** — Record gameplay, mark the bug moment, and have the agent analyze the clip.
- **[Watch & React](/stage/watch-workflow)** — Set up watches on specific nodes to monitor changes over time.
- **[spatial_snapshot reference](/stage/snapshot)** — Full parameter documentation for the snapshot tool.

## Common first-time issues

**Agent says "no tools available"** — The MCP server is not configured or not running. Check your `.mcp.json` and make sure the path to `stage` is correct and the binary exists. Alternatively, use the CLI directly: `stage spatial_snapshot '{}'`.

**Agent calls the tool but gets a connection error** — The game is not running, or port 9077 is blocked. Start the game first (F5), then retry.

**Snapshot returns 0 nodes** — The Stage addon is not collecting any nodes. Check that your nodes are in the scene tree (not orphaned), and that the Stage plugin is enabled.

**Response is very large** — Add `"detail": "summary"` or `"token_budget": 500` to limit response size. See [Token Budgets](/guide/token-budgets).
