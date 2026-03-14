# MCP & Your AI Agent

This page explains what MCP is, how Theatre uses it, and what tool call patterns your agent will use when working with Theatre.

## What is MCP?

**Model Context Protocol (MCP)** is an open standard for connecting AI agents to external tools and data sources. Think of it like a plugin system for AI agents — instead of the agent knowing how to do everything itself, it can call out to specialized servers that do specific things.

An MCP server exposes a set of **tools** — named functions with typed parameters — that the agent can call during a conversation. The agent decides when to call a tool, what parameters to pass, and how to interpret the result. The human does not need to trigger tool calls manually.

MCP uses JSON-RPC over a transport (Theatre uses stdio — the server reads from stdin and writes to stdout).

For the full specification, see [modelcontextprotocol.io](https://modelcontextprotocol.io/).

## How Theatre's tools work

When you ask "where is the player?", the agent does not guess from source code. It calls `spatial_snapshot` with appropriate parameters and reads the result. Here is what that looks like under the hood:

**Agent → MCP server (via stdin):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "spatial_snapshot",
    "arguments": {
      "detail": "summary",
      "focal_node": "Player"
    }
  }
}
```

**MCP server → Agent (via stdout):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{ \"frame\": 412, \"node_count\": 12, ... }"
      }
    ]
  }
}
```

The agent receives the response as a text blob and reasons over it. If it needs more detail on a specific node, it calls `spatial_inspect`. If it wants to check for changes, it calls `spatial_delta`. If it wants to fix something, it calls a Director operation.

## Tool discovery

When your agent starts, it calls `tools/list` to discover what tools are available. Theatre's servers return their full tool list with JSON Schema descriptions for each parameter. The agent uses these schemas to know what it can call and what parameters each tool accepts.

You can see Theatre's tool list by running the server manually:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
  ./target/release/stage serve
```

## Parallel tool calls

Agents can call multiple tools in parallel when the calls are independent. Theatre servers handle concurrent requests correctly. For example, an agent investigating a bug might call `spatial_snapshot` and `clips { "action": "list" }` simultaneously to gather context before deciding which to drill into.

You cannot depend on ordering between parallel calls — if you need the result of one call to decide what parameters to pass to the next, the agent will sequence them. Modern agents (Claude 3.5+, GPT-4o) handle this reasoning correctly.

## Tool call patterns

These are the most common patterns agents use with Theatre tools:

### Pattern 1: Observe → Diagnose

```
spatial_snapshot (broad overview)
  → spatial_inspect (drill into a specific node)
    → [diagnose from properties]
```

Used when there is a reported problem and the agent wants to understand the current state before suggesting anything.

### Pattern 2: Record → Query → Diagnose → Fix

```
[human clicks Start Recording in the Stage dock, plays, F9 to mark, clicks Stop Recording]
clips { "action": "list" }
  → clips { "action": "query_range", ... }
    → spatial_inspect (on a specific node at a specific frame)
      → director operation (fix the problem)
```

Used for the dashcam workflow — the gold standard debugging loop.

### Pattern 3: Watch → React

```
spatial_watch { "node": "player", "track": ["position", "velocity"] }
  → [agent polls spatial_delta periodically]
    → [detect anomaly]
      → spatial_inspect (get full context at anomaly frame)
        → diagnose
```

Used for ongoing monitoring during a test run.

### Pattern 4: Build → Verify

```
director { "op": "scene_create", ... }
  → director { "op": "node_add", ... } (repeated for each node)
    → [run game]
      → spatial_snapshot (verify structure)
        → spatial_query { "type": "radius", ... } (verify reachability)
```

Used for AI-driven level construction with Stage verifying the result.

## Tool result caching

The agent's context window retains tool results. If the agent calls `spatial_snapshot` at the start of a session, it can reference that data later without calling the tool again — unless the state has changed.

When things change (the player moves, you make a scene edit, you restart the game), remind the agent explicitly: "The game is running now" or "I moved the player to the respawn point." This lets the agent know it should re-observe rather than use stale data.

## What agents do not do automatically

**Agents do not poll.** Your agent will not keep calling `spatial_delta` every second unless you explicitly set up a watch workflow. Tool calls cost tokens and latency — agents call tools when they need data, not on a schedule.

**Agents do not modify files without being asked.** If the agent diagnoses a collision layer problem, it will tell you the diagnosis. It will call Director to fix it only if you ask it to.

**Agents do not know your scene structure without asking.** The first thing to do in a new session is `spatial_snapshot` or `scene_tree`. Otherwise the agent is reasoning from source files, not from the running game.

## Tips for working with Theatre

**Be explicit about the game state.** "The game is running," "I just pressed F5," "I'm at the part where the enemy spawns" — these help the agent call the right tools.

**Ask for a snapshot when context is stale.** If you've made changes and re-run the game, say "I restarted the game, take a fresh snapshot." The agent's previous tool results are now stale.

**Name your nodes clearly in Godot.** Theatre refers to nodes by their scene tree path. `Player/WeaponPivot/Gun` is much easier for an agent to reason about than `CharacterBody3D/Node3D/MeshInstance3D`.

**Tell the agent which node is relevant.** "Check the EnemyDetectionZone" is faster than "check all nodes." The agent can narrow scope immediately rather than scanning a 200-node snapshot.
