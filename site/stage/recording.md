---
description: "Choose explicit capture channels, separate loading from play, and keep bounded gameplay evidence."
---

# Recording

Stage capture is opt-in. Installing the addon does not start a listener, logger,
sampler, recorder or capture controls on ordinary launches. Godot still loads
the installed scripts and extension. Live observation and retained capture are
independent choices.

## Choose a launch

Use an already open, verified Godot editor with Director enabled:

```sh
theatre run scenes/review.tscn
theatre run scenes/review.tscn --observe on
theatre run scenes/review.tscn --play minimal --operator human
theatre run scenes/review.tscn --startup minimal --play standard --play-scope group:review --operator human
theatre run scenes/review.tscn --startup heavy --startup-images off --play off
```

Paths are relative to the project selected by `--project` (default: current
directory). Commands resolve through PATH on Windows, Linux and macOS.
`theatre run` uses Director's existing `editor_run` operation; it does not open
an editor, install missing components, change the main scene or launch a fallback
game process. Ask the user before project installation.

Director accepts the same typed options in `editor_run.launch`:

```json
{
  "observe": false,
  "operator": "human",
  "readiness": "project",
  "startup": {"preset": "minimal"},
  "play": {"preset": "standard", "scope": ["group:review"], "images": false}
}
```

The CLI's `--options` accepts this JSON; named flags override its fields.
Director's response describes configured intent, not proof that runtime loading
succeeded. Its `target_scene` distinguishes the selected scene from the native
bootstrap `playing_scene`. With observation enabled, inspect `runtime_status`
for the actual run identity and scene. Inspect `clips(status).ready` for the
capture readiness boundary; a node's ordinary Godot-ready state is not proof
that asynchronous project loading has finished.

## Presets are option bundles

| Preset | Metrics | Spatial state | Images | Default cadence |
| --- | --- | --- | --- | --- |
| Minimal | Yes | No | No | Metrics each rendered process callback |
| Light | Yes | Explicit scope | No | Spatial every 6 physics ticks |
| Standard | Yes | Explicit scope | Yes | Spatial every 2 ticks; images every 12 |
| Heavy | Yes | Whole scene unless scoped | Yes | Spatial every tick; images every 6 |

Light and Standard require scene-relative node paths or `group:name` selectors.
Scope selects those nodes, not an implicit recursive scan of their descendants.
Heavy deliberately allows broad scene coverage. Images are 640 pixels maximum
dimension under Standard and 960 under Heavy.

Each phase defaults to a 120-second, 12,000-record bound. Encoded payload budgets
are 8 MiB for Minimal, 128 MiB for Light/Standard and 256 MiB for Heavy. These
are limits on retained encoded data, not total process-memory or frame-time
guarantees. Effective settings and image capability appear in capture status.

Overrides include `metrics`, `spatial`, `images`, `scope`, `spatial_interval`,
`image_interval`, `image_size`, `duration_secs`, `max_records`, `payload_mib`,
`retention` and `save`. Use `preset: "off"` to disable a phase.
Optional `movement_nodes` and `input_actions` select bounded CharacterBody3D
contact evidence and named InputMap strengths, never raw keyboard capture.
Movement requires spatial capture; missing evidence is not zero input.

`image_readback: "auto"` uses an available native asynchronous OpenGL path.
Unavailable images do not stop metrics or spatial capture.
`image_readback: "synchronous"` is explicit recovery and can stall gameplay.
No preset selects it or changes the project's renderer.
`anomaly_enabled: true` separately opts into visual anomaly measurements and
system markers; it requires images. Automatic persistence still requires an
explicit saving policy.

## Integrate project readiness

The default `readiness: "project"` requires a deliberate notification. Add it
at the project's existing transition from initial loading to the intended
reviewable activity:

```gdscript
# Call after all required asynchronous work succeeds.
# Do not notify after failure/cancellation or for later background tasks.
var stage := get_node_or_null("/root/StageRuntime")
if stage != null:
    stage.notify_ready()
```

The project owns what "ready" means. Find the actual completion path, not merely
the scene's `_ready()` or the first background task. Test successful, failed,
canceled and repeated notification paths. Repeated calls are idempotent.
For a purely synchronous scene, explicitly choose `readiness: "scene"` to use
Godot's scene-ready boundary instead.

Startup uses a small bootstrap without preloading the target. Capture starts
before the selected scene's resource load, stops at readiness, and saves once
by default (`startup.save: "on_stop"`). It does not profile process creation,
engine initialization or earlier autoloads. Blocking loading work can prevent
periodic samples; elapsed intervals are not an internal loading trace.

Missing readiness leaves an actionable waiting state. Startup bounds still
stop capture and retain available evidence; they do not invent a ready event.
Orderly startup exit attempts a save. A crash can lose the in-memory tail.

## Human and agent operation

Human play defaults to manual start. After startup ends, no provider sampling,
spatial collection or image requests continue while waiting. The game itself
is not automatically paused; restart explicitly if review needs a fresh scene.

1. **Continue** starts a new play segment in the same recording.
2. **Start new** starts an independent recording.
3. **Mark** annotates a moment during capture.
4. **Stop** freezes the draft. **Keep** persists it; **Discard** removes only
   that unkept draft. Failed Keep retains the draft for retry.
5. **Copy reference** identifies saved evidence. **Share note + still** is a
   separate, deliberate feedback operation.

A segment is one contiguous interval with fixed settings. Changing the preset
or `clips(config)` stages settings for the next segment, never starts recording,
and never retroactively enriches earlier evidence. Unkept drafts are lost if
the process exits.

Agent play defaults to starting at readiness. Explicit `play_start` settings
override this operator-derived default, including a local user's automatic
preference on human launches. Review the resolved launch settings.

The live `clips` actions are `start`, `continue`, `stop`, `keep`, `discard`,
`save` (stop and keep immediately), `add_marker`, `status` and `config`.
They require observation to be enabled for agent access; native human controls
and project APIs do not require an agent connection.

The marker and pause shortcuts remain configurable through
`theatre/stage/shortcuts/marker_key` and `pause_key` (F9/F11 defaults).
Use buttons or project-appropriate alternative bindings when those keys conflict
with editor or gameplay controls. Repeated key events are ignored.

Drag the **Stage capture** header to move the panel away from game UI. **−**
minimizes it to a single **Stage +** restore button; this does not stop or pause
recording. Hover the restore button for capture status. Position and minimized
state last for this run only, and viewport resizing keeps the panel reachable.
The configured corner remains the initial placement; `hidden` still hides all
controls rather than leaving a restore button. Set the corner or `hidden` with
`theatre/stage/display/capture_controls`. Hiding controls does not disable
shortcuts. An ordinary off launch installs neither.

Release the pointer using the game's own binding before clicking the controls.
The panel uses a non-modal Godot child window so its clicks cannot fall through
to gameplay handlers that recapture the pointer. It follows the project's
embedded/native subwindow setting without changing that setting. Captured mouse
input passes through to gameplay; Stage never changes the game's mouse mode.
Gameplay that polls global `Input` state must still respect its own UI policy;
handling an event does not clear global button or action state.

## Rolling capture (dashcam)

`play.retention: "rolling"` retains bounded recent history instead of stopping
when a session fills. Start must still be explicit or selected through
`play_start: "ready"`.

Markers annotate by default. With `play.save: "on_trigger"`, deliberate/system
markers start one bounded five-second post-window. Repeated markers do not
extend it indefinitely. At completion the segment is saved and rolling capture
continues in a new segment. Stop shortens a pending post-window, saves what is
available and reports shortened coverage. A failed save keeps the draft and
blocks resumption.

`StageRuntime.marker(label, tier)` remains the developer API. Tiers are
`deliberate`, `system` and `silent`; silent only annotates. System markers
are rate-limited. Automatic saving is separately opt-in.
For local always-on metrics, prefer rolling retention to repeated persistent
captures. Saved storage has a visible 1024 MiB admission budget; previously
kept evidence is never deleted automatically.

## Add lightweight metrics

Register explicit synchronous providers, returning small dictionaries:

```gdscript
func _ready() -> void:
    var stage := get_node_or_null("/root/StageRuntime")
    if stage != null:
        stage.register_metric_provider("simulation", capture_metrics)

func capture_metrics() -> Dictionary:
    return {"pending_jobs": pending_jobs, "update_usec": last_update_usec}

func _exit_tree() -> void:
    var stage := get_node_or_null("/root/StageRuntime")
    if stage != null:
        stage.unregister_metric_provider("simulation")
```

Use unit-bearing names and cheap values already computed by the project.
Do not traverse the world or trigger expensive calculations in a provider.
There are at most 32 named providers. Callbacks run only while the metrics
channel is active, on Godot's main thread. Built-in `engine` values remain
distinct from project values.

For context that should not be repeated in every sample, use
`register_metadata_provider(name, Callable)` and `unregister_metadata_provider(name)`.
These providers return dictionaries and run once at an explicit phase/Start
boundary, not during the human wait. Register before that boundary; providers
registered by scene code are available for play, not the earlier startup boundary.
Use an existing early integration point if startup metadata is needed. Automatic rolling resumes
reuse the captured context. At most 32 providers and 64 KiB of combined encoded
metadata are accepted; invalid or oversized context leaves capture stopped.
Metadata is retained under `capture.context.project_metadata`. Keep it small,
portable and free of secrets.

Records include wall time, monotonic elapsed and interval microseconds, render
and physics frame identifiers. Callback/serialization timing is diagnostic
observer cost, not a controlled benchmark or total recorder cost. Provider
callbacks themselves can be expensive.

## Preferences and consent

Resolution order, lowest to highest:

1. Built-in safe defaults.
2. Shared project `stage.toml`.
3. Private `theatre/settings.toml` beneath Godot's `OS.get_config_dir()`.
4. Private project `stage.local.toml`.
5. Explicit launch options.

All files use a `[launch]` table, for example a deliberate private preference:

```toml
[launch]
play_start = "ready"

[launch.play]
preset = "minimal"
retention = "rolling"
save = "manual"
```

Ask before creating private project preferences; ensure the narrow
`stage.local.toml` ignore entry is approved and present first.
Tracked defaults must use portable project-relative paths.
An explicit off launch overrides always-on preferences:

```sh
theatre run scenes/review.tscn --observe off --startup off --play off
```

Legacy `[dashcam]` handshake settings and Godot auto-start settings do not
override launch-managed capture. Migrate intentional always-on behavior into
`[launch]`; do not infer that consent from old settings.

For deliberate direct Godot launches, place Theatre arguments after `--`:

```sh
godot --path . scenes/review.tscn -- --theatre-play=minimal --theatre-readiness=scene
godot --path . addons/stage/capture_bootstrap.tscn -- --theatre-target=res://scenes/review.tscn --theatre-startup=minimal
```

Use `--theatre-options=JSON` for the full option model. Startup capture requires
the bootstrap and an explicit target. Do not change the project's main scene
to the bootstrap.

## Inspect retained evidence

Each kept segment is a new SQLite clip in Godot's `user://stage_recordings`.
Segments share a recording ID when continued, but keep independent settings,
time ranges, stop reasons and actual channel counts. Never average timings
across an uncaptured gap or interpolate missing frames.

Use `clips(list)`, then `clips(metrics)` for metric samples and numeric summaries.
Physics-frame filters preserve separate render-frame IDs; summaries exclude
missing values rather than treating them as zero.
Responses return at most 200 samples and report truncation; numeric summaries
cover the selected range (up to 256 series and eight object levels). Use narrower
frame ranges for detailed inspection; the SQLite file retains the full data.
Spatial and image analysis report unavailable channels when they were not
captured. Use `snapshot_at`, `trajectory`, `query_range`, `diff_frames`,
`screenshot_at` and `visual_artifact` only with appropriate retained evidence.
Older clips remain readable; missing new metadata is not guessed.

Completed images retain their request frame/time. Unfinished work is reported
as gaps when a segment stops, never assigned to the next segment. Image and
physics samples are not an atomic snapshot.
After Keep, the ignored `.stage/clip_storage_path` hint enables offline analysis
without a running game. See [Dashcam workflow](/stage/dashcam).
