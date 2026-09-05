---
description: "Mark gameplay, save a clip, and inspect its sampled state and images—even after the game closes."
---

<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['clips'] ?? []
</script>

# clips

Keep evidence of a gameplay problem and inspect it later. While enabled, the
dashcam holds a rolling history of spatial samples and optional screenshots.
A marker preserves a window around an interesting moment; a successful save
writes that window to a clip file.

## Capture a problem

The in-game **Stage capture** panel shows whether recording is active, how much
history is available, and the configured marker shortcut—F9 by default.

| Control | Effect |
| --- | --- |
| **Start / Stop** | Enable or disable rolling capture. Stopping a pending marked window saves the available portion. |
| **Mark** | Mark this moment and collect the configured post-window: the time after the marker. |
| **Save now** | Save the available buffer immediately, including its marker label, without waiting for the remaining post-window. |
| **Copy reference** | Copy the last saved clip's identifier, run, frame range and scene at save for the agent. |
| **Share note + still** | Open the separate feedback composer. This creates a feedback note with a still image, not a dashcam clip. |

Wait for **Clip saved** before asking the agent to inspect the new clip. A marker
acknowledgement means the marker was accepted; it does not mean the post-window
has finished. Marking while recording is stopped explains that recording must
be started first.

Choose a corner or hide the controls with the project setting
`theatre/stage/display/capture_controls`. Hiding the panel does not disable the
marker shortcut or **Ctrl+Shift+F8** for feedback. Human confirmations remain
available when agent notifications are disabled.

An agent can mark the current moment with:

```json
{"action":"add_marker","marker_label":"player stopped against the wall"}
```

The response identifies the current engine frame and trigger tier. It does not
promise an immediately available clip. Historical marker placement is not
supported. To save immediately instead:

```json
{"action":"save","marker_label":"inspect the available window now"}
```

Game code can use `StageRuntime.marker(label, tier)`. Deliberate markers trigger
a clip; system markers are rate-limited; silent markers annotate without
triggering one.

## Configure recording

Use `status` to inspect effective settings, buffered coverage, image availability,
recent capture costs, gaps and the last successful save:

```json
{"action":"status"}
```

The live states are `disabled`, `buffering`, and `post_capture`. Configured window
lengths are limits and intentions, not proof that a full window has accumulated.
Check `coverage` for the actual retained span.

Configuration accepts a partial patch with a flat vocabulary. Unknown fields,
wrong types and invalid values are rejected before any settings change.

```json
{"action":"config","config":{"preset":"lightweight"}}
```

Choosing a preset leaves recording enabled or disabled as it was. Start it
explicitly when wanted:

```json
{"action":"config","config":{"enabled":true}}
```

Explicit values override the preset in the same request:

```json
{
  "action":"config",
  "config":{
    "preset":"lightweight",
    "pre_window_deliberate_sec":15,
    "post_window_deliberate_sec":5,
    "screenshot_max_dimension":320
  }
}
```

The response returns authoritative effective settings. When stopping closes a
pending clip, `stop_save` reports that save separately. Recording can stop
successfully even if the clip could not be saved; inspect that outcome rather
than treating configuration success as a saved acknowledgement. A project's `[dashcam]`
section in `stage.toml` uses the same keys; only its explicit fields are pushed
on connection. Settings chosen at runtime are not automatically written back to
that file.

### Choose cost deliberately

At 60 physics ticks per second, Lightweight samples spatial state about 10 times
per second and images about 5 times per second, at a maximum image dimension of
640 pixels. Detailed samples state about 30 times per second and images about
10 times per second, at up to 960 pixels. Both turn off dense bursts. Their
memory limits bound retained data; they do not guarantee the requested duration.

These names are relative, not a promise of negligible overhead. In a graphical
64-moving-object test using a debug extension, Lightweight maintained roughly
60 physics ticks per second but still showed pacing spikes. Detailed fell below
real-time 60 Hz. The repeatable `capture_controls_engine` measurement journey
prints the workload, effective settings and capture probes; use measurements
from your own project rather than treating those results as a universal budget.

If recording changes the behavior under investigation, reduce image frequency
or dimensions, disable screenshots, or stop recording. For spatial-only capture:

```json
{"action":"config","config":{"screenshot_enabled":false}}
```

Inspect pacing and readback cost as well as queue drops. No queue drops does not
mean capture is smooth. A smaller output image still requires reading the source
viewport, and more frequent spatial sampling also has a cost.

## Capture movement intent and contacts

Standing still does not prove that a character was stuck. For a movement
investigation, opt in to selected CharacterBody3D nodes and existing InputMap
actions. Replace these example paths and action names with those in your game:

```json
{
  "action":"config",
  "config":{
    "movement_nodes":["/root/Main/Player"],
    "input_actions":["move_forward","move_left","move_right"]
  }
}
```

The recorder validates targets before applying the patch and returns their
canonical paths. It supports up to 16 selected bodies and 16 actions. At each
spatial sample it records action strengths, floor/wall/ceiling flags, floor
normal when available, real velocity and up to eight slide-contact normals.
Truncation is explicit. These fields are absent when movement capture is off;
old clips remain readable without them.

Use full `snapshot_at` results or request `"movement"` in a trajectory's
`properties` alongside position or velocity. Use node paths from the saved
spatial data when selecting a trajectory.

Input strengths are global sampled intent, not proof that a particular body
consumed that input. Contacts describe the body's most recent `move_and_slide`
call. Callback order matters, and presses between samples can be missed. Use
these observations to distinguish idle, attempted movement and blocking
contacts—not to claim deterministic replay or an atomic controller trace.

## Select saved evidence

```json
{"action":"list"}
```

Choose an explicit `clip_id`, especially when the storage contains older runs.
New clips retain run identity, scene at save and configuration provenance.
`scene_at_save` identifies the scene when persistence occurred, not every scene
in the buffered history: a clip can span a scene transition. The live
status's `last_saved_clip` belongs to that recorder instance; older clips may
not contain enough metadata to establish their run.

After a successful save, Godot publishes its resolved storage location in the
project's `.stage/clip_storage_path` hint. A fresh CLI or MCP process can use
that location to inspect saved clips after the game closes. Live status,
configuration, markers and saving still need a running game. If the hint is
missing or the storage has moved, reconnect to resolve Godot's storage location
or update the hint to the known directory containing the retained clips.

For the following examples, replace `clip_example`, frames and node paths with
values from your selected clip.

```json
{"action":"markers","clip_id":"clip_example"}
```

Markers have their own engine timestamps. A marker need not coincide with a
sampled spatial frame or image. Missing samples are not evidence that nothing
happened between them.

## Inspect state and images

| Action | Use |
| --- | --- |
| `snapshot_at` | Inspect state at a recorded spatial frame, or nearest to a timestamp. |
| `trajectory` | Read a selected node's sampled position, velocity or other supported properties. |
| `query_range` | Search retained frames for supported spatial/state conditions. |
| `diff_frames` | Compare two recorded frames. |
| `find_event` | Search recorded events; an empty result does not establish that every game event was captured. |
| `screenshots` | List actual image frames and timestamps. |
| `screenshot_at` | Retrieve the nearest retained image. |
| `visual_artifact` | Generate a storyboard, motion history, difference map or node-following filmstrip. |

```json
{"action":"snapshot_at","clip_id":"clip_example","at_frame":336,"detail":"full"}
```

```json
{
  "action":"trajectory","clip_id":"clip_example","node":"Player",
  "from_frame":300,"to_frame":360,
  "properties":["position","velocity"],"sample_interval":1
}
```

```json
{"action":"visual_artifact","clip_id":"clip_example","artifact":"storyboard"}
```

Visual results identify sampled frames and coverage gaps. Markers outside the
saved image timestamps remain available through `markers`; the artifact reports
them as outside visual coverage rather than implying that an image exists at
that moment. Headless runs can retain spatial evidence without rendered images.
These tools analyze recordings; they do not replay or simulate the game.

Delete an unwanted clip explicitly:

```json
{"action":"delete","clip_id":"clip_example"}
```

## Parameters

<ParamTable :params="params" />
