<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['clips'] ?? []

const messages0 = [
  { role: 'human', text: `I recorded the tunneling bug. The marker was at frame 337. What was happening?` },
  { role: 'agent', text: `Found clip chase_bug_01 with 512 frames and 1 marker at frame 337 labeled player_clips_wall. Querying the frames around the marker.` },
  { role: 'agent', text: `Clear velocity spike at frame 337: Player velocity jumps from 14.2 m/s at frame 330 to 45.3 m/s at frame 337 — a 3x increase in one frame (16ms). At that velocity, the CharacterBody3D travels 0.75 meters per tick, which is wider than the Wall_East CollisionShape3D. The move_and_slide continuous collision detection did not catch it because the shape is thinner than the per-tick displacement. Fix: enable CCD in the CharacterBody3D or thicken the wall collision shape to at least 1.0 unit.` },
]
</script>

# clips

Record gameplay clips and query them frame by frame.

The `clips` tool is the backbone of the dashcam workflow. It writes every physics frame of spatial data to disk, so you can scrub through a timeline of exactly what happened — positions, velocities, and properties — at any frame in the recording.

## When to use it

- **Capturing a bug**: record while playing, mark the bug moment, analyze the clip
- **Post-mortem analysis**: something went wrong in a playtest — query what happened
- **Regression testing**: record expected behavior, compare against future runs
- **Long sessions**: in-memory ring buffer holds ~10 seconds; recordings hold hours

## Parameters

<ParamTable :params="params" />

## Actions

### `start`

Begin recording. A new clip is created and data is written to disk on every physics tick.

```json
{
  "action": "start",
  "clip_id": "chase_bug_01"
}
```

If `clip_id` is omitted, a unique ID is generated automatically (e.g., `clip_1741987200`).

**Response:**
```json
{
  "action": "start",
  "clip_id": "chase_bug_01",
  "result": "ok",
  "record_path": "/tmp/theatre-clips/chase_bug_01.clip"
}
```

### `stop`

Stop the current recording.

```json
{
  "action": "stop"
}
```

**Response:**
```json
{
  "action": "stop",
  "clip_id": "chase_bug_01",
  "result": "ok",
  "frame_count": 512,
  "duration_ms": 8533,
  "file_size_bytes": 204800
}
```

### `mark`

Mark the current frame as a point of interest (e.g., "bug happened here"). This is what the **F9** key triggers from the editor dock.

```json
{
  "action": "mark",
  "label": "player_clips_wall"
}
```

**Response:**
```json
{
  "action": "mark",
  "clip_id": "chase_bug_01",
  "frame": 337,
  "label": "player_clips_wall",
  "result": "ok"
}
```

Markers in clips carry a `source` field identifying who placed them:

| Source | Origin |
|--------|--------|
| `"human"` | F9 key or editor dock button |
| `"agent"` | MCP `add_marker` action |
| `"system"` | Automatic dashcam trigger |
| `"code"` | `SpectatorRuntime.marker()` in game script |

### `list`

List all available clips.

```json
{
  "action": "list"
}
```

**Response:**
```json
{
  "clips": [
    {
      "clip_id": "chase_bug_01",
      "frame_count": 512,
      "duration_ms": 8533,
      "created_at": "2026-03-12T14:30:00Z",
      "markers": [
        { "frame": 337, "label": "player_clips_wall" }
      ]
    }
  ]
}
```

### `query_frame`

Get the complete spatial state at a specific frame.

```json
{
  "action": "query_frame",
  "clip_id": "chase_bug_01",
  "frame": 337,
  "nodes": ["Player", "Wall_East"],
  "detail": "full"
}
```

| Parameter | Type | Description |
|---|---|---|
| `clip_id` | `string` | Which clip to query |
| `frame` | `integer` | Frame number (0-based) |
| `nodes` | `string[]` | Limit to these nodes (optional) |
| `detail` | `string` | `"summary"` or `"full"` |

**Response:**
```json
{
  "clip_id": "chase_bug_01",
  "frame": 337,
  "timestamp_ms": 5617,
  "nodes": {
    "Player": {
      "class": "CharacterBody3D",
      "global_position": [8.92, 0.0, -3.14],
      "velocity": [45.3, 0.0, 0.0]
    },
    "Wall_East": {
      "class": "StaticBody3D",
      "global_position": [9.0, 0.0, -3.0]
    }
  }
}
```

### `query_range`

Query multiple consecutive frames at once. The primary tool for analyzing a bug across time.

```json
{
  "action": "query_range",
  "clip_id": "chase_bug_01",
  "start_frame": 325,
  "end_frame": 350,
  "nodes": ["Player"],
  "detail": "summary",
  "stride": 1
}
```

| Parameter | Type | Default | Description |
|---|---|---|---|
| `clip_id` | `string` | required | Which clip to query |
| `start_frame` | `integer` | required | First frame (inclusive) |
| `end_frame` | `integer` | required | Last frame (inclusive) |
| `nodes` | `string[]` | all | Nodes to include |
| `detail` | `string` | `"summary"` | Data level per node |
| `stride` | `integer` | `1` | Sample every N frames |
| `condition` | `object` | null | Filter frames by condition |

**Condition filtering:**

Use `condition` to include only frames where something specific is true:

```json
{
  "condition": {
    "type": "proximity",
    "nodes": ["Player", "Wall_East"],
    "max_distance": 1.0
  }
}
```

Other condition types:
- `{ "type": "velocity_above", "node": "Player", "threshold": 20.0 }` — frames where the node's speed exceeds threshold
- `{ "type": "property_equals", "node": "Player", "property": "on_floor", "value": false }` — frames where a property matches a value

### `delete`

Delete a clip file from disk.

```json
{
  "action": "delete",
  "clip_id": "chase_bug_01"
}
```

**Response:**
```json
{
  "action": "delete",
  "clip_id": "chase_bug_01",
  "result": "ok"
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Always specify `nodes` in `query_range`.** Even small clips with all nodes and `detail: full` can be enormous. Filter to the 2-3 nodes relevant to the bug.

**Use `stride: 5` for long recordings.** Instead of every frame, sample every 5th frame for a quick scan. Then use `query_frame` to drill into specific moments.

**Use conditions to filter.** The `proximity` condition is especially powerful — it finds frames where two nodes are closer than a threshold, which is exactly when collision bugs occur.

**Markers are set automatically with F9.** In the editor dock, pressing F9 calls `clips { "action": "mark" }` with a default label. You can also add more labels by calling the tool directly during a session.
