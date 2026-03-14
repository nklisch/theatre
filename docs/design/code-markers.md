# Design: GDScript Code Markers API

## Overview

Add a developer-facing `SpectatorRuntime.marker("label")` API so game scripts
can place markers that fire when that line of code executes. This gives
developers instrumentation points for dashcam clip capture without needing
human input (F9) or an AI agent (MCP `add_marker`).

Code markers use `source: "code"` in the marker data and default to **system
tier** (rate-limited, shorter clips) to prevent a marker in a hot loop from
generating thousands of clips. An optional `tier` parameter lets developers
escalate to `"deliberate"` (always triggers) or downgrade to `"silent"`
(annotate-only, no trigger).

### Tier behavior summary

| Tier | Triggers clip? | Rate-limited? | Use case |
|------|---------------|---------------|----------|
| `"system"` (default) | Yes | Yes (2s min interval) | General instrumentation — safe in loops |
| `"deliberate"` | Yes | No | Rare, high-value events (e.g., boss defeated) |
| `"silent"` | No | N/A | Annotations for clips triggered by other means |

Silent markers are stored in a pending list on the recorder. When any clip is
saved (for any reason), pending silent markers whose frames fall within the
clip's frame range are included in the clip's `markers` table.

## Implementation Units

### Unit 1: Silent marker storage on SpectatorRecorder

**File**: `crates/spectator-godot/src/recorder.rs`

Add a `Vec<DashcamTrigger>` field to `SpectatorRecorder` that accumulates
silent markers. Drain matching entries into clips at save time.

```rust
// New field on SpectatorRecorder:
/// Silent markers waiting to be attached to the next saved clip.
pending_silent_markers: Vec<DashcamTrigger>,
```

**In `init()`**: initialize to `Vec::new()`.

**New method — `add_silent_marker`**:

```rust
/// Record a silent marker. Does not trigger dashcam capture.
/// The marker is attached to the next clip whose frame range includes it.
fn add_silent_marker(&mut self, source: &str, label: &str, frame: u64, timestamp_ms: u64) {
    self.pending_silent_markers.push(DashcamTrigger {
        frame,
        timestamp_ms,
        source: source.to_string(),
        label: label.to_string(),
    });

    self.base_mut().emit_signal(
        "marker_added",
        &[frame.to_variant(), GString::from(source).to_variant(), GString::from(label).to_variant()],
    );
}
```

**In `flush_dashcam_clip_internal()`** — after computing
`frame_range_start`/`frame_range_end`, drain matching silent markers into the
clip's `markers` vec:

```rust
// After computing frame_start and frame_end from pre_buffer/post_buffer:
let mut all_markers = markers; // from PostCapture state
let drained: Vec<DashcamTrigger> = self.pending_silent_markers
    .drain(..)
    .filter(|m| m.frame >= frame_start && m.frame <= frame_end)
    .collect();
// Retain markers outside the range for future clips.
// (drain already removed them, but we need to put back out-of-range ones)
// Actually: use retain + extend pattern instead:
let mut silent_in_range = Vec::new();
self.pending_silent_markers.retain(|m| {
    if m.frame >= frame_start && m.frame <= frame_end {
        silent_in_range.push(DashcamTrigger {
            frame: m.frame,
            timestamp_ms: m.timestamp_ms,
            source: m.source.clone(),
            label: m.label.clone(),
        });
        false // remove from pending
    } else {
        true // keep for future clips
    }
});
all_markers.extend(silent_in_range);
all_markers.sort_by_key(|m| m.frame);
```

Also add a cap to prevent unbounded growth of the pending list. If no clip
is ever triggered, old silent markers should be evicted:

```rust
/// Max pending silent markers. Oldest are evicted when exceeded.
const MAX_PENDING_SILENT_MARKERS: usize = 1000;

// In add_silent_marker, after push:
while self.pending_silent_markers.len() > MAX_PENDING_SILENT_MARKERS {
    self.pending_silent_markers.remove(0);
}
```

**Implementation Notes**:
- `pending_silent_markers` lives on the main thread with everything else — no
  sync concerns.
- The retain-and-extend approach keeps markers that predate the clip's frame
  range (they might match a future clip) and discards those that postdate it
  (impossible — silent markers are always at or before current frame).
- Sort by frame after merging so the markers table is ordered.

**Acceptance Criteria**:
- [ ] Silent markers are stored in `pending_silent_markers` without triggering
      dashcam state transitions
- [ ] When a clip is saved, pending silent markers within the clip's frame
      range are included in the clip's `markers` table
- [ ] Pending silent markers outside the frame range are retained
- [ ] The pending list is capped at 1000 entries with FIFO eviction
- [ ] The `marker_added` signal is still emitted for silent markers

---

### Unit 2: `add_code_marker` exported method on SpectatorRecorder

**File**: `crates/spectator-godot/src/recorder.rs`

New `#[func]` method that dispatches based on tier string.

```rust
#[godot_api]
impl SpectatorRecorder {
    /// Add a code marker. Tier: "system" (default, rate-limited), "deliberate"
    /// (always triggers), "silent" (annotate-only).
    #[func]
    pub fn add_code_marker(&mut self, label: GString, tier: GString) {
        let frame = current_physics_frame();
        let timestamp_ms = current_time_ms();
        let tier_str = tier.to_string();

        match tier_str.as_str() {
            "silent" => {
                self.add_silent_marker("code", &label.to_string(), frame, timestamp_ms);
            }
            "deliberate" => {
                self.on_dashcam_marker("code", &label.to_string(), frame, timestamp_ms);
                self.base_mut().emit_signal(
                    "marker_added",
                    &[frame.to_variant(), GString::from("code").to_variant(), label.to_variant()],
                );
            }
            _ => {
                // Default: system tier (includes "system" and any unrecognized string)
                self.on_dashcam_marker("code", &label.to_string(), frame, timestamp_ms);
                self.base_mut().emit_signal(
                    "marker_added",
                    &[frame.to_variant(), GString::from("code").to_variant(), label.to_variant()],
                );
            }
        }
    }
}
```

**Implementation Notes**:
- The existing `on_dashcam_marker` maps source to tier: `"system"` →
  `DashcamTier::System`, anything else → `DashcamTier::Deliberate`. For code
  markers, the tier mapping needs adjustment. Since `"code"` source would
  currently map to `Deliberate`, we need to change `on_dashcam_marker`'s tier
  resolution or pass the tier explicitly.
- **Preferred approach**: Add an internal `on_dashcam_marker_with_tier` method
  that accepts an explicit `DashcamTier`, and have the existing
  `on_dashcam_marker` call it after resolving tier from source. The new
  `add_code_marker` calls `on_dashcam_marker_with_tier` directly.

```rust
fn on_dashcam_marker_with_tier(
    &mut self,
    source: &str,
    label: &str,
    frame: u64,
    timestamp_ms: u64,
    tier: DashcamTier,
) {
    // ... existing on_dashcam_marker body but using `tier` param directly
}

fn on_dashcam_marker(&mut self, source: &str, label: &str, frame: u64, timestamp_ms: u64) {
    let tier = if source == "system" {
        DashcamTier::System
    } else {
        DashcamTier::Deliberate
    };
    self.on_dashcam_marker_with_tier(source, label, frame, timestamp_ms, tier);
}
```

Then `add_code_marker` calls:
```rust
"deliberate" => {
    self.on_dashcam_marker_with_tier("code", &label_str, frame, timestamp_ms, DashcamTier::Deliberate);
    // emit signal...
}
_ => {
    self.on_dashcam_marker_with_tier("code", &label_str, frame, timestamp_ms, DashcamTier::System);
    // emit signal...
}
```

Note: `on_dashcam_marker` already emits `marker_added` signal via
`add_marker`. Since `add_code_marker` is a separate entry point, it must emit
the signal itself for the non-silent tiers. Wait — actually, `on_dashcam_marker`
does NOT emit the signal; that's done by the public `add_marker` method.
So `add_code_marker` must emit `marker_added` for all tiers (including silent,
which is handled in `add_silent_marker`).

**Acceptance Criteria**:
- [ ] `add_code_marker("label", "system")` triggers dashcam at system tier
      (rate-limited)
- [ ] `add_code_marker("label", "deliberate")` triggers dashcam at deliberate
      tier (always)
- [ ] `add_code_marker("label", "silent")` stores a pending marker without
      triggering dashcam
- [ ] `add_code_marker("label", "")` defaults to system tier
- [ ] All tiers emit the `marker_added` signal
- [ ] Source is always `"code"` in the marker data

---

### Unit 3: `marker()` method on SpectatorRuntime autoload

**File**: `addons/spectator/runtime.gd`

```gdscript
## Place a code marker at the current frame.
## Tier controls dashcam behavior:
##   "system"     — rate-limited clip trigger (default, safe in loops)
##   "deliberate" — always triggers a clip (use for rare, important events)
##   "silent"     — annotates only, no clip trigger
func marker(label: String, tier: String = "system") -> void:
    if not recorder:
        return
    recorder.add_code_marker(label, tier)
```

**Implementation Notes**:
- This is a thin wrapper. All logic lives in the Rust `add_code_marker`.
- The method is callable from any GDScript in the project as
  `SpectatorRuntime.marker("label")`.
- When Spectator is not loaded (GDExtension missing), `recorder` is null and
  the call is a no-op. This is important: game code with markers won't crash
  if the addon is removed.
- The default `tier = "system"` means the one-arg form
  `SpectatorRuntime.marker("hit detected")` is rate-limited and safe.

**Acceptance Criteria**:
- [ ] `SpectatorRuntime.marker("label")` is callable from any game script
- [ ] Default tier is `"system"` when omitted
- [ ] No-op when Spectator is not loaded (no crash, no error)
- [ ] Two-arg form `SpectatorRuntime.marker("label", "deliberate")` works

---

### Unit 4: TCP handler for `recording_marker` — support `source: "code"`

**File**: `crates/spectator-godot/src/recording_handler.rs`

The existing `handle_marker` function in the TCP handler hardcodes
`source: "agent"` as default and routes through `trigger_dashcam_clip`. No
changes needed here — code markers go through the GDScript → Rust path
(`add_code_marker`), not through TCP. The existing `recording_marker` TCP
method continues to serve the MCP `add_marker` action.

However, verify that `source: "code"` in clip marker data doesn't break any
downstream consumers (clip listing, marker queries). Since source is a free
string field, this should be fine.

**Acceptance Criteria**:
- [ ] Clips with `source: "code"` markers are listed correctly by
      `recording_list` and `recording_markers`
- [ ] No hardcoded source validation that would reject "code"

---

### Unit 5: Unit tests for code markers

**File**: `crates/spectator-godot/src/recorder.rs` (in `#[cfg(test)] mod tests`)

```rust
#[test]
fn silent_marker_stored_in_pending() {
    // Create recorder internals (use existing test helpers)
    // Call add_silent_marker
    // Assert pending_silent_markers contains the marker
    // Assert dashcam_state is unchanged (still Buffering)
}

#[test]
fn silent_markers_merged_into_clip() {
    // Setup: recorder in PostCapture state with known frame range
    // Add silent markers: one in range, one out of range
    // Flush clip
    // Assert: in-range marker appears in clip, out-of-range stays pending
}

#[test]
fn silent_markers_capped_at_max() {
    // Add MAX_PENDING_SILENT_MARKERS + 10 markers
    // Assert: only MAX_PENDING_SILENT_MARKERS remain, oldest evicted
}

#[test]
fn code_marker_system_tier_is_rate_limited() {
    // Reuse existing rate-limiting test pattern
    // Fire two code markers within system_min_interval_sec
    // Assert: second one annotates but doesn't extend post-window
}

#[test]
fn code_marker_deliberate_tier_always_triggers() {
    // Fire code marker with tier "deliberate" while buffering
    // Assert: transitions to PostCapture with Deliberate tier
}
```

**Implementation Notes**:
- These tests operate on the recorder's internal state, not on Godot. They
  follow the existing `#[cfg(test)] mod tests` pattern in recorder.rs which
  tests the dashcam state machine directly using a mock SQLite DB.
- The `add_code_marker` method itself calls into `on_dashcam_marker_with_tier`
  and `add_silent_marker` which can be tested individually.
- Testing the actual signal emission requires Godot (`#[itest]`), which is
  out of scope for unit tests.

**Acceptance Criteria**:
- [ ] All 5 tests pass
- [ ] Tests cover the three tiers: system, deliberate, silent
- [ ] Tests verify rate-limiting applies to code/system markers
- [ ] Tests verify silent marker eviction cap

---

### Unit 6: Documentation updates

The following files need updates to document the new code marker API:

#### 6a: User-facing dashcam guide

**File**: `site/spectator/dashcam.md`

Add a new section **"Code Markers"** after the existing marker documentation:

```markdown
### Code Markers

Place markers directly in your GDScript to trigger dashcam clips when
specific code paths execute:

```gdscript
# Basic usage — system tier (rate-limited, safe in loops)
SpectatorRuntime.marker("player_hit")

# Rare, important events — always triggers a clip
SpectatorRuntime.marker("boss_defeated", "deliberate")

# Silent — annotates the next clip without triggering one
SpectatorRuntime.marker("entered_zone_b", "silent")
```

**Tiers:**

| Tier | Triggers clip? | Rate-limited? | Best for |
|------|---------------|---------------|----------|
| `"system"` (default) | Yes | Yes (2 s) | General instrumentation |
| `"deliberate"` | Yes | No | Rare, high-value events |
| `"silent"` | No | — | Annotations for other clips |

Code markers appear in clip data with `source: "code"`.
```

#### 6b: Recording/clips reference

**File**: `site/spectator/recording.md`

Add `"code"` to the list of marker sources (alongside `"human"`, `"agent"`,
`"system"`).

#### 6c: Spectator agent skill

**File**: `.agents/skills/spectator/SKILL.md`

Add a note in the clips section that clips may contain `source: "code"`
markers placed by game scripts, and what the tier implications are.

#### 6d: Godot addon skill

**File**: `.agents/skills/godot-addon/SKILL.md`

Document the `SpectatorRuntime.marker(label, tier)` method and the
`SpectatorRecorder.add_code_marker(label, tier)` GDExtension export.

**Acceptance Criteria**:
- [ ] `site/spectator/dashcam.md` documents the `SpectatorRuntime.marker()`
      API with all three tiers and usage examples
- [ ] `site/spectator/recording.md` lists `"code"` as a marker source
- [ ] `.agents/skills/spectator/SKILL.md` mentions `source: "code"` markers
- [ ] `.agents/skills/godot-addon/SKILL.md` documents the new methods

---

### Unit 7: MCP tool schema — no changes needed

The existing `clips` MCP tool with `add_marker` action already covers the
agent use case. Code markers are a game-side API, not an MCP tool. No new
MCP tool or parameter is needed.

The `clips` tool's `markers` action will naturally return code markers in its
response since they're stored in the same `markers` table. The `source` field
distinguishes `"code"` from `"agent"` / `"human"` / `"system"`.

**Acceptance Criteria**:
- [ ] `clips` action `markers` returns entries with `source: "code"` when
      present in a clip

---

## Implementation Order

1. **Unit 2** — `on_dashcam_marker_with_tier` refactor + `add_code_marker`
   export (depends on nothing)
2. **Unit 1** — Silent marker storage + merge into clip flush (builds on
   Unit 2's tier plumbing)
3. **Unit 3** — `runtime.gd` `marker()` method (depends on Unit 2)
4. **Unit 5** — Unit tests (depends on Units 1 & 2)
5. **Unit 4** — Verification pass (depends on Units 1-3)
6. **Unit 6** — Documentation (depends on all above being final)

## Testing

### Unit Tests: `crates/spectator-godot/src/recorder.rs`

See Unit 5 above. Tests operate on recorder internals without requiring Godot.

### Integration Test (manual)

After `theatre deploy`, run a test Godot project with a script containing:

```gdscript
func _physics_process(delta):
    if health < 50:
        SpectatorRuntime.marker("low_health")
    if health <= 0:
        SpectatorRuntime.marker("player_died", "deliberate")
    SpectatorRuntime.marker("frame_tick", "silent")
```

Verify:
- `low_health` markers trigger rate-limited system clips
- `player_died` marker triggers a deliberate clip immediately
- `frame_tick` silent markers appear in clips triggered by other markers
- No clip flood from the per-frame `frame_tick` call

### E2E Journey Test

The existing E2E test infrastructure (`GodotFixture`, `E2EHarness`) can be
extended with a test scene that calls `SpectatorRuntime.marker()` from
GDScript. This is optional for the initial implementation — the unit tests
on recorder internals provide sufficient coverage of the core logic.

## Verification Checklist

```bash
# Build
cargo build -p spectator-godot

# Unit tests
cargo test -p spectator-godot

# Lint
cargo clippy --workspace
cargo fmt --check

# Deploy and verify with Godot
theatre deploy ~/godot/test-harness
godot --headless --quit --path ~/godot/test-harness 2>&1
# Expected: no SCRIPT ERROR, no [panic]

# Full test suite
cargo test --workspace
```
