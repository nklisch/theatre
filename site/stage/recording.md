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

Manage dashcam clips and analyze recorded gameplay frame by frame.

The `clips` tool interfaces with the dashcam system. The dashcam continuously
buffers spatial data in memory. When you press F9 or the agent calls
`add_marker`, the buffer is flushed to a clip file (SQLite). Clips can then be
queried frame by frame.

## How clips are created

Clips are saved in four ways:

| Trigger | Source | Description |
|---|---|---|
| Press **F9** or click **⚑** in-game button | Human | Saves dashcam buffer as a "human" clip |
| `clips { "action": "add_marker" }` | Agent | Saves dashcam buffer as an "agent" clip |
| `StageRuntime.marker()` in GDScript | Code | Saves dashcam buffer from game code |
| `clips { "action": "save" }` | Agent | Force-flush buffer immediately |

Each clip captures ring buffer contents (up to 60 seconds before the trigger)
plus approximately 30 seconds of post-capture.

## Parameters

<ParamTable :params="params" />

## Actions

### Clip management

#### `add_marker`

Mark the current moment and trigger a clip capture. This is what the **F9** key
triggers from the in-game flag button.

```json
{
  "action": "add_marker",
  "marker_label": "player_clips_wall"
}
```

| Parameter | Type | Description |
|---|---|---|
| `marker_label` | `string` | Optional label for the marker |
| `marker_frame` | `integer` | Frame to mark (defaults to current frame) |

**Response:**
```json
{
  "action": "add_marker",
  "clip_id": "clip_1741987200",
  "marker_id": "m_a1b2c3",
  "marker_label": "player_clips_wall",
  "result": "ok"
}
```

#### `save`

Force-save the dashcam buffer as a clip immediately, without adding a marker.

```json
{
  "action": "save"
}
```

**Response:**
```json
{
  "action": "save",
  "clip_id": "clip_1741987200",
  "result": "ok",
  "frame_count": 512
}
```

#### `status`

Get dashcam buffer state, buffer size, and configuration.

```json
{
  "action": "status"
}
```

**Response:**
```json
{
  "action": "status",
  "state": "buffering",
  "buffer_frames": 1247,
  "pre_window_deliberate_sec": 60,
  "byte_cap_mb": 1024
}
```

States: `buffering` (running normally), `post_capture` (saving post-trigger
window), `disabled` (dashcam off).

#### `list`

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
        { "marker_id": "m_a1b2c3", "frame": 337, "marker_label": "player_clips_wall" }
      ]
    }
  ]
}
```

#### `delete`

Remove a clip by `clip_id`.

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

#### `markers`

List all markers in a saved clip.

```json
{
  "action": "markers",
  "clip_id": "chase_bug_01"
}
```

**Response:**
```json
{
  "clip_id": "chase_bug_01",
  "markers": [
    { "marker_id": "m_a1b2c3", "frame": 337, "marker_label": "player_clips_wall", "source": "human" }
  ]
}
```

### Clip analysis

#### `snapshot_at`

Get the spatial state at a specific frame.

```json
{
  "action": "snapshot_at",
  "clip_id": "chase_bug_01",
  "at_frame": 337,
  "detail": "full"
}
```

| Parameter | Type | Description |
|---|---|---|
| `clip_id` | `string` | Which clip to query |
| `at_frame` | `integer` | Frame number (use this or `at_time_ms`) |
| `at_time_ms` | `integer` | Timestamp in ms (use this or `at_frame`) |
| `detail` | `string` | `"summary"` or `"full"` |

**Response:**
```json
{
  "clip_id": "chase_bug_01",
  "at_frame": 337,
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

#### `trajectory`

Get position/property timeseries across a frame range.

```json
{
  "action": "trajectory",
  "clip_id": "chase_bug_01",
  "node": "Player",
  "from_frame": 325,
  "to_frame": 350,
  "properties": ["position", "velocity"],
  "sample_interval": 1
}
```

| Parameter | Type | Default | Description |
|---|---|---|---|
| `clip_id` | `string` | required | Which clip to query |
| `node` | `string` | required | Node to track |
| `from_frame` | `integer` | required | First frame (inclusive) |
| `to_frame` | `integer` | required | Last frame (inclusive) |
| `properties` | `string[]` | `["position"]` | Properties to track |
| `sample_interval` | `integer` | `1` | Sample every N frames |

#### `query_range`

Search frames for spatial conditions.

```json
{
  "action": "query_range",
  "clip_id": "chase_bug_01",
  "from_frame": 325,
  "to_frame": 350,
  "node": "Player",
  "condition": {
    "type": "velocity_spike"
  }
}
```

| Parameter | Type | Description |
|---|---|---|
| `clip_id` | `string` | Which clip to query |
| `from_frame` | `integer` | First frame (inclusive) |
| `to_frame` | `integer` | Last frame (inclusive) |
| `node` | `string` | Node to filter on (optional) |
| `condition` | `object` | Condition filter |

**Condition types:**

| Type | Description |
|---|---|
| `moved` | Frames where the node moved more than a threshold |
| `proximity` | Frames where two nodes are within a distance |
| `velocity_spike` | Frames with a sudden velocity increase |
| `property_change` | Frames where a property changed value |
| `state_transition` | Frames where an AnimationTree/FSM state changed |
| `signal_emitted` | Frames where a signal was emitted |
| `entered_area` | Frames where a body entered an Area3D |
| `collision` | Frames with a collision event |

#### `diff_frames`

Compare two frames to see what changed.

```json
{
  "action": "diff_frames",
  "clip_id": "chase_bug_01",
  "frame_a": 336,
  "frame_b": 337
}
```

#### `find_event`

Search for a specific event type within a frame range.

```json
{
  "action": "find_event",
  "clip_id": "chase_bug_01",
  "event_type": "signal_emitted",
  "event_filter": { "signal": "body_entered" },
  "from_frame": 300,
  "to_frame": 400
}
```

#### `screenshot_at`

Get the viewport screenshot nearest to a frame or timestamp.

```json
{
  "action": "screenshot_at",
  "clip_id": "chase_bug_01",
  "at_frame": 337
}
```

| Parameter | Type | Description |
|---|---|---|
| `clip_id` | `string` | Which clip to query |
| `at_frame` | `integer` | Frame number (use this or `at_time_ms`) |
| `at_time_ms` | `integer` | Timestamp in ms (use this or `at_frame`) |

#### `screenshots`

List screenshot metadata in a clip (frame numbers, timestamps).

```json
{
  "action": "screenshots",
  "clip_id": "chase_bug_01"
}
```

## Marker sources

| Source | Trigger |
|---|---|
| `human` | F9 key or in-game flag button |
| `agent` | `clips { "action": "add_marker" }` |
| `code` | `StageRuntime.marker()` in GDScript |

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Always specify `node` in `trajectory`.** Even small clips with all nodes can
be enormous. Filter to the 2-3 nodes relevant to the bug.

**Use `sample_interval: 5` for long clips.** Sample every 5th frame for a
quick overview, then use `snapshot_at` to drill into specific moments.

**Use `query_range` with conditions to filter.** The `proximity` condition
finds frames where two nodes are within a distance — exactly when collision
bugs occur.

**Markers are saved automatically with F9.** Pressing F9 (or clicking the
in-game ⚑ flag button) saves a dashcam clip — the full buffer including ~60
seconds of history before the trigger. The clip is immediately available for
analysis via `list` and `snapshot_at`.
