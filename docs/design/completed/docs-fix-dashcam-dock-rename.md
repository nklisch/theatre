# Design: Documentation Fixes — Dashcam Reality, Dock Accuracy, Stage Rename

## Overview

The public documentation site describes a manual start/stop recording workflow
with dock buttons that don't exist. The actual system is a dashcam that
auto-buffers and flushes clips on F9. Additionally, the `clips` MCP tool
documents actions (`start`, `stop`, `mark`) that don't exist — the real actions
are `add_marker`, `save`, `status`, `list`, `delete`, `markers`, `snapshot_at`,
`trajectory`, `query_range`, `diff_frames`, `find_event`, `screenshot_at`,
`screenshots`.

This design covers fixing all affected pages to match the real implementation,
and catches remaining Stage→Stage naming artifacts.

### Guiding Principles

- **Human-first**: Lead with what the user sees and does (F9, overlay button,
  toast notifications). Show MCP tool details second.
- **Strip to reality**: Only document features that exist in code. No aspirational
  sections.
- **Dashcam mental model**: The system is always recording. You mark moments,
  not start/stop sessions.

---

## Implementation Units

### Unit 1: `site/guide/first-session.md` — Rewrite Step 1

**File**: `site/guide/first-session.md`

**Current (wrong)**:
```markdown
## Step 1: Start a recording session

Click the **Start Recording** button in the Stage dock to begin recording.

Walk the player in front of the enemy a few times...

Press **F9** to mark this as a bug moment (the dock shows "Bug marker set at frame 312").
Click the **Stop Recording** button in the Stage dock.
```

**New content**:
```markdown
## Step 1: Capture the bug

Stage's dashcam is always running — it continuously buffers the last 60
seconds of spatial data in memory. You don't need to start anything.

Play the game normally. Walk the player in front of the enemy a few times. On
the third or fourth pass, the enemy fails to detect you — you see the player
enters the zone visually but the alert animation does not play.

Press **F9** to save a clip of the bug moment. You'll see a toast notification
in the top-right corner: "Dashcam clip saved". The clip contains the last 60
seconds of spatial data plus ~30 seconds of post-capture.

You can also click the **⚑** flag button in the top-left corner of the game
viewport — it does the same thing as F9.
```

**Implementation Notes**:
- Remove all references to "Start Recording" / "Stop Recording" buttons
- Remove "(the dock shows ...)" — the dock doesn't show marker feedback, the
  in-game overlay shows a toast
- Keep the rest of the page (Steps 2-6) mostly intact — the agent conversation
  examples are fine since they just reference "a clip"
- Update "Patterns to take away" section: change "Mark bug moments with F9"
  explanation to mention dashcam context

**Acceptance Criteria**:
- [ ] No references to dock Record/Stop buttons
- [ ] Step 1 explains the dashcam auto-buffering model
- [ ] F9 and flag button are both mentioned as ways to save a clip
- [ ] Toast notification described (not dock feedback)

---

### Unit 2: `site/stage/editor-dock.md` — Strip to reality

**File**: `site/stage/editor-dock.md`

Replace the entire "Recording controls", "Clip list", and "Active watches"
sections. Keep "Status bar", "Activity feed", and "Keyboard shortcuts" sections.

**New structure**:
```markdown
# Stage Editor Dock

The Stage editor dock shows connection status and agent activity. It appears
on the right side of the Godot editor when the Stage plugin is enabled.

## Opening the dock

The dock appears automatically when the Stage plugin is enabled
(**Project → Project Settings → Plugins → Stage → Enable**).

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

The feed can be collapsed with the **▼** button.

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
```

**Implementation Notes**:
- Remove: "Recording controls" section (Record/Stop/Mark Bug buttons)
- Remove: "Clip list" section (doesn't exist in dock)
- Remove: "Active watches" section (dock shows count only, not a list)
- Remove: "Configuring shortcuts" reference to `Editor → Editor Settings →
  Shortcuts → Stage` and `stage_record_mark` / `stage_record_pause` (these
  don't exist — shortcuts are in Project Settings)
- Remove: "Reading the activity feed" expandable detail (can't click to expand
  in current dock implementation)
- Remove: Tips about clip list renaming
- Fix F11 description: "Pause / unpause the game" not "Pause / unpause
  recording"

**Acceptance Criteria**:
- [ ] No references to Record, Stop, or Mark Bug buttons
- [ ] No references to clip list UI
- [ ] No references to interactive watches list
- [ ] Shortcut config points to Project Settings, not Editor Settings
- [ ] F11 described as pausing the game, not recording
- [ ] Activity feed section is accurate to dock.gd implementation

---

### Unit 3: `site/stage/dashcam.md` — Rewrite around always-on model

**File**: `site/stage/dashcam.md`

The page already hints at the dashcam concept but contradicts itself by
describing dock Record/Stop buttons. Rewrite to be consistent.

**Key changes**:

1. **Keyboard shortcuts table**: Change F11 from "Pause / unpause recording"
   to "Pause / unpause the game"

2. **Remove paragraph about dock buttons** (line 43):
   ```
   Use the editor dock's **Record** button to start and stop recording. You can
   also trigger marking via the dock UI's **Mark Bug** button. From the agent
   side, use the `clips` tool's `"start"` and `"stop"` actions.
   ```
   Replace with:
   ```
   From the agent side, use the `clips` tool's `"add_marker"` action to trigger
   a clip save, or `"save"` to force-flush the current buffer.
   ```

3. **Rewrite "Step 1: Enable continuous recording"** (lines 78-82):
   ```
   ### Step 1: Play the game

   Start the game. The dashcam begins buffering automatically — you'll see
   "● Dashcam: buffering" in the top-left corner of the game viewport. Every
   physics frame of spatial data is captured into a 60-second rolling buffer.

   Play normally. Move around. Do some stealth sections. Wait for the bug to occur.
   ```

4. **Rewrite the marker action** (lines 84-86): Remove "click **Stop** in the
   dock (or call `clips { "action": "stop" }`)" and replace with:
   ```
   You immediately press **F9** (or click the **⚑** flag button). The dashcam
   saves the last 60 seconds plus the next ~30 seconds as a clip. A toast
   confirms: "Dashcam clip saved". Continue playing or stop — the clip is
   already saved.
   ```

5. **"What made this work" section** (line 112): Change "Always-on recording:
   because you started recording before the bug" to "Always-on dashcam: because
   the buffer was running before the bug"

6. **"Quick investigation" variation** (lines 121-126): Remove dock button
   references:
   ```
   ### Quick investigation (no marker)

   If the bug is obvious and repeatable:

   1. Trigger the bug in the running game
   2. Press **F9** to capture the clip
   3. Ask the agent: "Something went wrong in the last few seconds. Analyze the
      latest clip."
   ```

7. **"Continuous background recording" variation** (lines 128-139):
   The dashcam IS continuous by default. Reframe:
   ```
   ### Longer buffer windows

   The default dashcam buffer holds 60 seconds. To extend it, configure the
   dashcam via `spatial_config` or project settings:

   - `pre_window_deliberate_sec`: seconds of history to keep before a marker
     (default: 60)
   - `byte_cap_mb`: memory limit for the ring buffer (default: 1024)
   ```

8. **"Post-playtest analysis" variation** (lines 141-149): Remove dock button
   references:
   ```
   ### Post-playtest analysis

   1. Have the tester play normally (dashcam is always running)
   2. Have them press **F9** whenever something seems wrong
   3. Collect the clip files from `user://stage_recordings/` afterward
   4. Ask the agent to analyze all markers across clips
   ```

**Acceptance Criteria**:
- [ ] No references to dock Record/Stop/Mark Bug buttons
- [ ] No references to `clips { "action": "start" }` or `clips { "action": "stop" }`
- [ ] F11 described as game pause, not recording pause
- [ ] Dashcam described as always-on, not something you enable
- [ ] F9 described as saving a clip (flush), not just marking a frame
- [ ] In-game overlay elements mentioned (flag button, dashcam label, toasts)

---

### Unit 4: `site/stage/recording.md` — Human workflow + API reference split

**File**: `site/stage/recording.md`

Restructure the page: lead with the human dashcam workflow, then document the
actual `clips` MCP tool actions.

**New structure**:

```markdown
# clips

Manage dashcam clips and analyze recorded gameplay frame by frame.

The `clips` tool interfaces with the dashcam system. The dashcam continuously
buffers spatial data in memory. When you press F9 or the agent calls
`add_marker`, the buffer is flushed to a clip file. Clip files can then be
queried frame by frame.

## How clips are created

Clips are saved automatically when:

- **You press F9** or click the in-game flag button — saves a "human" clip
- **The agent calls `add_marker`** — saves an "agent" clip
- **Game code calls `StageRuntime.marker()`** — saves a "code" clip
- **The agent calls `save`** — force-flushes the current buffer

Each clip captures the ring buffer contents (up to 60 seconds before the
trigger) plus a post-capture window (~30 seconds after).

## Parameters

<ParamTable :params="params" />

## Actions

### Clip management

#### `add_marker`

Mark the current moment, triggers clip capture.

[JSON example with marker_label, marker_frame params]

#### `save`

Force-save the dashcam buffer as a clip immediately.

#### `status`

Get dashcam buffer state: buffering/post_capture/disabled, buffer size, config.

#### `list`

List all saved clips with frame counts, durations, and markers.

#### `delete`

Remove a clip by clip_id.

#### `markers`

List all markers in a saved clip.

### Clip analysis

#### `snapshot_at`

Spatial state at a specific frame in a clip. Use `at_frame` or `at_time_ms`.

#### `trajectory`

Position/property timeseries across a frame range. Use `properties` to select
which fields, `sample_interval` to control density.

#### `query_range`

Search frames for spatial conditions. Supports `condition` filtering:
proximity, velocity_spike, property_change, state_transition, signal_emitted,
entered_area, collision, moved.

#### `diff_frames`

Compare two frames in a clip. Shows property changes between frame_a and
frame_b.

#### `find_event`

Search for events matching a type and filter string.

#### `screenshot_at`

Get the viewport screenshot nearest to a frame or timestamp.

#### `screenshots`

List screenshot metadata in a clip.

## Marker sources

| Source | Origin |
|--------|--------|
| `"human"` | F9 key or in-game flag button |
| `"agent"` | MCP `add_marker` action |
| `"system"` | Automatic dashcam trigger |
| `"code"` | `StageRuntime.marker()` in game script |
```

**Implementation Notes**:
- Remove fabricated `start`, `stop`, `mark`, `query_frame`, `query_range` actions
- Document all 13 actual ClipAction variants from the enum
- Use correct serde snake_case names: `add_marker`, `save`, `status`, `list`,
  `delete`, `markers`, `snapshot_at`, `trajectory`, `query_range`,
  `diff_frames`, `find_event`, `screenshot_at`, `screenshots`
- The ClipsParams struct field names give the actual parameter names: `clip_id`,
  `marker_label`, `marker_frame`, `token_budget`, `at_frame`, `at_time_ms`,
  `detail`, `from_frame`, `to_frame`, `node`, `condition`, `properties`,
  `sample_interval`, `event_type`, `event_filter`, `frame_a`, `frame_b`
- Remove the `condition` examples that use `proximity`, `velocity_above`,
  `property_equals` (old names) — the actual condition types from the schemars
  description are: `moved`, `proximity`, `velocity_spike`, `property_change`,
  `state_transition`, `signal_emitted`, `entered_area`, `collision`
- Remove Tips section references to non-existent actions

**Acceptance Criteria**:
- [ ] All 13 ClipAction variants documented
- [ ] No `start`, `stop`, `mark`, `query_frame` actions
- [ ] Parameter names match ClipsParams struct fields
- [ ] Condition types match actual schemars description
- [ ] Marker source table uses "in-game flag button" not "editor dock button"
- [ ] Tips section references correct action names

---

### Unit 5: `site/guide/what-is-theatre.md` — Fix design philosophy

**File**: `site/guide/what-is-theatre.md`

**Current (wrong)** (line 88):
```markdown
**No screenshots required.** The workflow is: click **Record** in the dock,
mark bugs with **F9**, then click **Stop** and ask your agent to analyze the
clip.
```

**New content**:
```markdown
**No screenshots required.** The workflow is: play your game, press **F9** to
mark the bug moment, and ask your agent to analyze the clip. The agent scrubs
the spatial timeline, finds the exact frame, diagnoses the cause, and suggests
a fix — all from structured data.
```

**Acceptance Criteria**:
- [ ] No reference to dock Record/Stop buttons
- [ ] F9 is the primary action mentioned
- [ ] Dashcam model implied (mark + analyze, no start/stop)

---

### Unit 6: `site/index.md` — Fix homepage dashcam paragraph

**File**: `site/index.md`

**Current (wrong)** (line 61):
```markdown
You press **F9** to mark the bug moment (use the dock's **Record** button to start), and the agent
```

**New content**:
```markdown
You press **F9** to mark the bug moment — the dashcam saves the last 60 seconds of spatial data — and the agent
```

**Acceptance Criteria**:
- [ ] No reference to dock Record button
- [ ] Dashcam described as always-on

---

### Unit 7: `site/guide/mcp-basics.md` — Fix Pattern 2

**File**: `site/guide/mcp-basics.md`

**Current (wrong)** (lines 84-92):
```markdown
### Pattern 2: Record → Query → Diagnose → Fix

```
[human clicks Start Recording in the Stage dock, plays, F9 to mark, clicks Stop Recording]
clips { "action": "list" }
```

**New content**:
```markdown
### Pattern 2: Mark → Query → Diagnose → Fix

```
[human plays the game, presses F9 when a bug occurs, dashcam clip is saved]
clips { "action": "list" }
  → clips { "action": "snapshot_at", "clip_id": "...", "at_frame": 337 }
    → clips { "action": "trajectory", "clip_id": "...", "node": "Player" }
      → director operation (fix the problem)
```

**Acceptance Criteria**:
- [ ] No Start/Stop Recording references
- [ ] Uses actual action names (snapshot_at, trajectory, not query_range)
- [ ] Pattern name changed from "Record" to "Mark"

---

### Unit 8: `site/guide/how-it-works.md` — Fix ClassDB.instantiate claim

**File**: `site/guide/how-it-works.md`

**Current (wrong)** (lines 52-55):
```markdown
`addons/stage/runtime.gd` is a thin GDScript file that:
- Checks if the GDExtension loaded via `ClassDB.class_exists`
- Instantiates extension classes using `ClassDB.instantiate`
- Provides graceful degradation if the extension is missing
```

**New content**:
```markdown
`addons/stage/runtime.gd` is a thin GDScript file that:
- Checks if the GDExtension loaded via `ClassDB.class_exists`
- Instantiates extension classes using direct constructors (e.g. `StageTCPServer.new()`)
- Provides graceful degradation if the extension is missing (logs a warning, no crash)
```

Also fix line 44:
```markdown
- **`StageRecorder`** — manages the dashcam ring buffer and writes clip files to disk
```
(was: "writes frame buffers to clip files on disk when recording is active" —
implies manual recording)

**Acceptance Criteria**:
- [ ] `ClassDB.instantiate` replaced with `.new()` constructors
- [ ] StageRecorder description mentions dashcam, not "when recording is active"

---

### Unit 9: `site/examples/physics-tunneling.md` — Fix Step 1

**File**: `site/examples/physics-tunneling.md`

**Current (wrong)** (line 39):
```markdown
Click **Record** in the Stage dock, fire several bullets at the wall at
different angles and distances, then click **Stop** when you have captured
a few tunneling events.
```

**New content**:
```markdown
Fire several bullets at the wall at different angles and distances. When you
see a tunneling event, press **F9** to save the clip. The dashcam captures
the last 60 seconds plus 30 seconds of post-capture, so you don't need to
press it immediately.
```

Also fix conversation script lines 16-18 where human/agent reference
"Record another test" / "Stop recording" — these should reference the dashcam
model instead.

**Acceptance Criteria**:
- [ ] No dock Record/Stop button references
- [ ] Uses F9 to trigger clip save

---

### Unit 10: `site/examples/animation-sync.md` — Fix Step 1

**File**: `site/examples/animation-sync.md`

**Current (wrong)** (line 38):
```markdown
Click Start Recording in the Stage dock, perform 5-6 attacks against an
enemy, including some that visually connect, then click Stop Recording.
```

**New content**:
```markdown
Perform 5-6 attacks against an enemy, including some that visually connect.
When you see a hit that should register but doesn't, press **F9** to save
the clip.
```

**Acceptance Criteria**:
- [ ] No dock Record/Stop button references
- [ ] Uses F9 to save clip

---

### Unit 11: `site/api/wire-format.md` — Fix recording wire types

**File**: `site/api/wire-format.md`

**Current (wrong)** (lines 73-79):
```json
{"type": "record_start", "clip_id": "clip_01"}
{"type": "record_stop"}
{"type": "record_mark", "label": "bug_moment"}
{"type": "record_query_frame", "clip_id": "clip_01", "frame": 337}
{"type": "record_query_range", "clip_id": "clip_01", "start_frame": 300, "end_frame": 350}
{"type": "record_list"}
{"type": "record_delete", "clip_id": "clip_01"}
```

**New content** (from actual QueryMethod enum):
```json
{"type": "recording_marker", "source": "agent", "label": "bug_moment"}
{"type": "recording_markers", "clip_id": "clip_01"}
{"type": "recording_list"}
{"type": "recording_delete", "clip_id": "clip_01"}
{"type": "recording_resolve_path"}
{"type": "dashcam_status"}
{"type": "dashcam_flush"}
{"type": "dashcam_config"}
```

**Implementation Notes**:
- These are the actual TCP wire format message types from
  `crates/stage-protocol/src/query_dispatch.rs`
- There is an explicit test (`recording_start_is_unknown_method`) that confirms
  `record_start` and `record_stop` don't exist
- Note: clip analysis actions (snapshot_at, trajectory, etc.) are handled by
  the server reading SQLite directly, not via TCP wire messages

**Acceptance Criteria**:
- [ ] Wire format examples match actual QueryMethod variants
- [ ] No `record_start` or `record_stop` wire types
- [ ] Note that clip analysis is server-side, not wire protocol

---

### Unit 12: `site/api/errors.md` — Fix recording errors

**File**: `site/api/errors.md`

**Current (wrong)** (lines 49-56):
```markdown
### Recording errors

| Error | Cause | Resolution |
|---|---|---|
| `No active recording` | `stop` or `mark` called when not recording | Start a recording first with `action: "start"` |
```

**New content**:
```markdown
### Clip errors

| Error | Cause | Resolution |
|---|---|---|
| `Clip not found: "X"` | clip_id does not exist | Use `clips { "action": "list" }` to see available clips |
| `Frame out of clip range` | `at_frame` is beyond the clip's frame count | Check clip details from `list` before querying |
| `Dashcam disabled` | Dashcam not running | Check `status` action for dashcam state |
| `Write error: disk full` | No space for clip file | Free disk space |
```

**Acceptance Criteria**:
- [ ] No reference to `start` or `stop` actions
- [ ] Error table references actual action names
- [ ] No `record_path` via `spatial_config` reference (not how config works)

---

### Unit 13: `site/api/index.md` — Fix clips TypeScript signatures

**File**: `site/api/index.md`

Lines around 424-433 show:
```typescript
// Start recording
clips({ action: "start", clip_id: "chase_01" })

// Stop recording
clips({ action: "stop" })
```

Replace with actual actions:
```typescript
// Save dashcam buffer as clip
clips({ action: "save" })

// Add marker (triggers clip capture)
clips({ action: "add_marker", marker_label: "bug_here" })

// Get dashcam status
clips({ action: "status" })

// Analyze a frame in a clip
clips({ action: "snapshot_at", clip_id: "clip_01", at_frame: 337 })
```

**Acceptance Criteria**:
- [ ] No start/stop action examples
- [ ] Shows actual action names

---

### Unit 14: `site/changelog.md` — Fix editor dock claims

**File**: `site/changelog.md`

**Current (wrong)** (lines 67-72):
```markdown
**Editor dock:**
- Recording controls (Start / Mark Bug / Stop)
- Keyboard shortcuts: F9 (marker), F11 (pause)
- Clip list with duration, frame count, and marker display
- Active watches display with delete controls
- Activity feed showing recent agent tool calls
```

**New content**:
```markdown
**Editor dock:**
- Connection status, tracked nodes, active watches count
- Keyboard shortcuts: F9 (save dashcam clip), F11 (pause game)
- Activity feed showing recent agent tool calls

**In-game overlay (runtime):**
- Dashcam status label (top-left)
- Marker flag button (top-left)
- Toast notifications for markers and clip saves (top-right)
```

Also fix line 16: "Editor dock: clip list now shows duration and marker count
inline" — this feature doesn't exist. Either remove or move to a "Planned"
note.

**Acceptance Criteria**:
- [ ] Dock features match actual dock.gd implementation
- [ ] In-game overlay features documented separately
- [ ] No fabricated clip list / watches list features

---

### Unit 15: `CLAUDE.md` — Fix ClassDB.instantiate claim

**File**: `CLAUDE.md`

**Current (wrong)** (GDScript Adapter Notes section):
```markdown
`addons/stage/runtime.gd` avoids static type annotations for GDExtension
types (`StageTCPServer`, `StageCollector`, `StageRecorder`) and
uses `ClassDB.instantiate(&"ClassName")` instead of `ClassName.new()`.
```

**New content**:
```markdown
`addons/stage/runtime.gd` avoids static type annotations for GDExtension
types (`StageTCPServer`, `StageCollector`, `StageRecorder`) and uses direct
constructors (`StageTCPServer.new()`, etc.). The `ClassDB.class_exists` guard
checks whether the extension loaded before attempting instantiation.
```

**Implementation Notes**:
- The actual runtime.gd code uses `StageTCPServer.new()`, `StageCollector.new()`,
  `StageRecorder.new()` — standard Godot constructors
- The `ClassDB.class_exists(&"StageTCPServer")` check is what provides safety
- The "avoids static type annotations" part is accurate (uses `var tcp_server:
  StageTCPServer` untyped in the var declaration context)

**Acceptance Criteria**:
- [ ] No reference to `ClassDB.instantiate`
- [ ] Mentions `.new()` constructors
- [ ] Keeps the `ClassDB.class_exists` guard mention

---

## Implementation Order

The units are independent — they can be implemented in any order or in parallel.
However, the recommended order groups by criticality:

1. **Unit 3** (`dashcam.md`) — most content to rewrite, establishes the new
   narrative that other pages reference
2. **Unit 4** (`recording.md`) — second-largest rewrite, documents the actual API
3. **Unit 2** (`editor-dock.md`) — full page rewrite
4. **Unit 1** (`first-session.md`) — Step 1 rewrite
5. **Unit 5** (`what-is-theatre.md`) — one paragraph fix
6. **Unit 6** (`index.md`) — one line fix
7. **Unit 7** (`mcp-basics.md`) — one section fix
8. **Unit 8** (`how-it-works.md`) — two small fixes
9. **Units 9-10** (`physics-tunneling.md`, `animation-sync.md`) — example fixes
10. **Units 11-13** (`wire-format.md`, `errors.md`, `api/index.md`) — API ref fixes
11. **Unit 14** (`changelog.md`) — changelog accuracy
12. **Unit 15** (`CLAUDE.md`) — internal doc fix

## Testing

### Manual verification

After all changes, run the VitePress dev server and visually verify each page:

```bash
cd site && npm run dev
```

Check each modified page renders correctly and all `AgentConversation`,
`ParamTable`, and `ArchDiagram` components still function.

### Grep verification

Confirm no remaining fabricated UI references:

```bash
# Should return zero matches in site/ after all fixes:
grep -r "Start Recording\|Stop Recording\|Mark Bug\|Click.*Record\|click.*Stop.*dock\|dock.*Record" site/ --include="*.md"

# Should return zero matches for non-existent actions:
grep -r '"action": "start"\|"action": "stop"\|"action": "mark"' site/ --include="*.md"

# Should return zero matches for old wire types:
grep -r "record_start\|record_stop\|record_mark" site/ --include="*.md"

# Should return zero matches for ClassDB.instantiate in docs:
grep -r "ClassDB.instantiate" site/ --include="*.md"
grep "ClassDB.instantiate" CLAUDE.md
```

### Cross-reference check

Verify that all `clips` action names in docs match the actual `ClipAction` enum:

```bash
# Extract action names from docs:
grep -oP '"action":\s*"[^"]+' site/stage/recording.md | sort -u

# Compare against actual enum (serde snake_case):
# add_marker, save, status, list, delete, markers, snapshot_at,
# trajectory, query_range, diff_frames, find_event, screenshot_at, screenshots
```

## Verification Checklist

```bash
# Build the site to check for broken links / component errors
cd site && npm run build

# Grep for remaining fabricated references (should be empty)
grep -rn "Start Recording\|Stop Recording\|Mark Bug" site/ --include="*.md"
grep -rn '"action": "start"\|"action": "stop"\|"action": "mark"' site/ --include="*.md"
grep -rn "record_start\|record_stop" site/ --include="*.md"
grep -rn "ClassDB.instantiate" site/ CLAUDE.md --include="*.md"
grep -rn "Editor Settings.*Shortcuts.*Stage\|stage_record_mark\|stage_record_pause" site/ --include="*.md"

# Verify no broken internal links
# (VitePress build will catch these)
```
