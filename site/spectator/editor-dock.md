# Spectator Editor Dock

The Spectator editor dock is a panel in the Godot editor that gives you direct control over recording and monitoring without leaving the editor.

## Opening the dock

The dock appears automatically on the right side of the Godot editor when the Spectator plugin is enabled (**Project → Project Settings → Plugins → Spectator → Enable**).

If the dock is not visible, go to **Editor → Editor Layout** and check that "Spectator" is enabled, or drag it from the dock panel list.

## Dock sections

### Status bar

At the top of the dock:

```
● Spectator: Connected (frame 1247)
```

Shows whether the Spectator GDExtension is loaded and the MCP server is connected.

| Status | Meaning |
|---|---|
| `Ready` | Extension loaded, game not running |
| `Connected (frame N)` | Game running, data flowing |
| `Extension not found` | GDExtension binary missing |
| `MCP disconnected` | Server not running or not connected |

### Recording controls

The recording section has three buttons:

**[ ⏺ Record ]** — Start a new recording. Equivalent to pressing F8. The button label changes to "Recording..." while active, and shows the elapsed time.

**[ 🔖 Mark Bug ]** — Mark the current frame. Equivalent to pressing F9. Opens a small dialog to optionally label the mark. Labels appear in the clip timeline.

**[ ⏹ Stop ]** — Stop the current recording. Equivalent to pressing F10. After stopping, the clip appears in the clip list below.

### Clip list

Below the recording controls, the clip list shows all saved clips:

```
clip_stealth_01        48s  ● 2 markers
clip_patrol_test       30s
clip_1741987200        12s  ● 1 marker
```

Clicking a clip shows its details: creation time, frame count, duration, file size, and marker list with frame numbers and labels.

**Delete** button (trash icon): removes the clip file from disk.

**Inspect** button (magnifying glass): opens the clip in the timeline view (if available).

### Active watches

The watches section shows all currently active `spatial_watch` registrations:

```
w_a1b2c3  Player           position, health
w_d4e5f6  Enemy_0          position, velocity
```

Clicking a watch entry shows its details (tracked properties, created at frame). The **×** button deletes the watch.

### Activity feed

The activity feed shows recent MCP tool calls made by the AI agent:

```
14:32:01  spatial_snapshot     detail=summary
14:32:05  spatial_inspect      node=EnemyDetectionZone
14:32:08  spatial_action       set collision_mask=1
14:32:12  recording            query_range frames 2700-2730
```

This is useful for understanding what the agent is doing and verifying that tool calls are reaching the game.

Each entry shows:
- Timestamp
- Tool name
- Brief summary of parameters
- Result (green checkmark = success, red × = error)

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| **F8** | Start / restart recording |
| **F9** | Mark current frame as bug |
| **F10** | Stop recording |

These shortcuts work while the game is running, whether focus is on the game window or the editor. They do not work in the editor without the game running.

### Configuring shortcuts

Shortcuts can be changed in **Editor → Editor Settings → Shortcuts → Spectator**:
- `spectator_record_start`
- `spectator_record_mark`
- `spectator_record_stop`

If F8-F10 conflict with your game's input, reassign them here.

## Reading the activity feed

The activity feed is your window into what the AI agent is doing. Each tool call is logged with a summary:

```
spatial_snapshot  summary, 12 nodes, 847 tokens
spatial_query     radius 5m from Player → 3 results
spatial_inspect   EnemyDetectionZone, properties+signals
spatial_action    Enemy_0.collision_mask = 1  ✓
recording         list → 3 clips
```

If you see a tool call succeed but get unexpected results, click the entry to expand it — you can see the full parameters sent and the full response returned.

### Error entries

Errors appear in red:

```
spatial_inspect   Player/NonexistentNode  ✗  Node not found
```

Errors are usually miscommunication between the agent and the scene structure. Check that the node path is correct (use `scene_tree` to verify).

## Tips

**Leave the dock visible during debugging sessions.** Watching the activity feed in real time tells you what the agent is doing and helps you give better guidance.

**Use the clip list as your recording archive.** After each debugging session, rename your clips to something descriptive (right-click → Rename in the clip list). This makes them findable later.

**The frame counter in the status bar is your reference.** When you want to tell the agent "start analyzing from where I am now," read the current frame from the dock status bar.

**Monitor the activity feed for token spikes.** If you see a tool call returning very high token counts (e.g., `spatial_snapshot: 8,400 tokens`), the response may be too large. Ask the agent to use tighter budgets or filter by type.
