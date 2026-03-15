---
description: "Theatre's Editor Dock provides a visual interface for spatial observations directly inside the Godot editor."
---

# Stage Editor Dock

The Stage editor dock shows connection status and agent activity. It appears
on the right side of the Godot editor when the Stage plugin is enabled.

## Opening the dock

The dock appears automatically when the Stage plugin is enabled
(**Project → Project Settings → Plugins → Stage → Enable**).

If the dock is not visible, go to **Editor → Editor Layout** and check that
"Stage" is enabled.

## Dock sections

### Connection status

At the top of the dock:

| Status | Meaning |
|---|---|
| Green dot + "Connected" | Game running, data flowing |
| Yellow dot + "Waiting..." | Extension loaded, waiting for connection |
| Red dot + "Stopped" | Game not running |

Also shows: port number, tracked node count, group count, frame counter, and FPS.

### Session info

Below the connection status:

- **Tracking**: number of nodes and groups being tracked
- **Watches**: count of active `spatial_watch` registrations
- **Frame**: current physics frame number and FPS

### Activity feed

The activity feed shows recent MCP tool calls made by the AI agent:

```
14:32:01  spatial_snapshot     detail=summary
14:32:05  spatial_inspect      node=EnemyDetectionZone
14:32:08  spatial_action       set collision_mask=1
14:32:12  clips                query_range frames 2700-2730
```

Each entry shows timestamp, tool name, and a brief summary. Entries are
color-coded: yellow for actions, cyan for watches, gray for queries.

The feed holds up to 20 entries and can be collapsed with the **▼** button.

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| **F9** | Save dashcam clip (mark bug moment) |
| **F11** | Pause / unpause the game |

These work while the game is running, whether focus is on the game window
or the editor. They are handled by the StageRuntime autoload, not the dock.

### Configuring shortcuts

Shortcuts are configured in **Project → Project Settings**:

| Setting | Default | Description |
|---|---|---|
| `theatre/stage/shortcuts/marker_key` | `F9` | Key to save dashcam clip |
| `theatre/stage/shortcuts/pause_key` | `F11` | Key to pause/unpause game |

Values are key names: `F1` through `F12`.

## Tips

**Leave the dock visible during debugging sessions.** The activity feed shows
what the agent is doing in real time.

**The frame counter is your reference.** When telling the agent "look at what's
happening now," read the frame number from the dock.

**Watch for large responses.** If the activity feed shows high token counts,
ask the agent to use tighter budgets or filter by node type.
