---
description: "Use opt-in rolling capture to retain the moments around a gameplay issue."
---

# Dashcam workflow

Dashcam is Stage's rolling retention option, not a separate tool or an
always-active default. Choose its channels and saving policy explicitly.

## Human review

Launch a saved scene through its open editor:

```sh
theatre run scenes/review.tscn --play light --play-scope group:review --play-retention rolling --operator human
```

Integrate the project's [ready notification](/stage/recording#integrate-project-readiness).
After readiness, the panel waits without capturing. When ready to test:

1. Click **Start new**, or **Continue** to add a separate segment to saved loading
   evidence.
2. Reproduce the issue. **Mark** annotates a moment; it does not automatically
   save under the default manual policy.
3. **Stop**, then **Keep** or **Discard** the draft. Failed saves leave it
   available for retry.
4. Copy the saved reference and ask the agent to inspect the relevant segment.

The recording retains only bounded recent history while active. It cannot
recover activity from before Start or from a waiting interval. Spatial scope
and image availability limit what the agent can establish.

## Explicit save-on-trigger

For an unattended investigation or a deliberate local preference:

```sh
theatre run scenes/review.tscn --observe on --operator agent --play heavy --play-retention rolling --play-save on_trigger
```

A deliberate or system marker opens one five-second post-window; silent markers
only annotate. Repeated markers cannot postpone completion indefinitely.
Successful saving starts a new rolling segment in the same recording.
Stop retains only the available post-window and reports it as shortened.
A failed save retains the draft and prevents automatic resumption.

Keep on-trigger persistence separate from selecting a preset or an operator.
No automatic cleanup deletes previously saved clips. At the storage admission
budget, remove selected evidence explicitly before retrying.

## Analyze without overclaiming

Start with `clips(list)` and `clips(markers)`. Use `metrics` for timing/counters,
spatial analysis for recorded state, and images/artifacts only if captured.
A current `spatial_inspect` result describes the running game, not a historical
frame. Reproduce and inspect live when saved data does not contain the required
property.

See [Recording](/stage/recording) for presets, bounds, readiness, local overrides,
provider APIs, shortcuts and renderer limitations.
