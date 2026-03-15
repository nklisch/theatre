---
description: "Debug UI overlap and layout issues — AI agent uses spatial query to find overlapping Control nodes and z-index problems."
---

<script setup>
const messages0 = [
  { role: 'human', text: `The HUD overlaps on non-standard resolutions. I am running the game at 2560x1440. Show me the UI node positions and sizes.` },
  { role: 'agent', text: `At 2560×1440: HealthBar position=[10, 1330], size=[200, 40]. Minimap position=[10, 1290], size=[200, 200]. The Minimap y position (1290) + size (200) = 1490, which exceeds the screen height at the bottom of the layout. More critically, HealthBar y=1330 and Minimap starts at y=1290 — the HealthBar is inside the Minimap rect.` },
]
const messages1 = [
  { role: 'agent', text: `HealthBar anchors: anchor_left=0, anchor_top=0, anchor_right=0, anchor_bottom=0 — pinned to the TOP-LEFT corner, not bottom-left. offset_top=1300 (a hardcoded pixel offset). This is why it breaks on different resolutions: it is placed 1300px from the top, which looks correct on 1080p but not on 1440p.` },
  { role: 'agent', text: `Minimap anchors: anchor_left=0, anchor_top=0, anchor_right=0, anchor_bottom=0 — also top-left pinned. offset_top=1260. Same problem. Both should use anchor_bottom=1 (bottom edge) with small negative offsets from the bottom.` },
]
const messages2 = [
  { role: 'human', text: `Fix the anchors so HealthBar is bottom-left and Minimap is bottom-right, 10px from the edges.` },
  { role: 'agent', text: `Anchors updated for all three nodes:\\n- HealthBar: bottom-left, 10px from bottom-left corner\\n- Minimap: bottom-right, 10px from bottom-right corner\\n- AmmoLabel: bottom-center, 10px from bottom` },
]
const messages3 = [
  { role: 'human', text: `Running at 2560x1440 now — it looks correct. Check the positions to confirm.` },
  { role: 'agent', text: `At 2560×1440:\\n- HealthBar: position=[10, 1380], size=[200, 40] — 10px from bottom-left ✓\\n- Minimap: position=[2350, 1230], size=[200, 200] — 10px from bottom-right ✓\\n- AmmoLabel: position=[1230, 1395], size=[100, 25] — centered horizontally ✓\\n\\nNo overlaps. All three nodes are within screen bounds.` },
]
</script>

# UI Overlap Issues

**Scenario**: The HUD looks fine on a 1920×1080 display, but on 2560×1440 and mobile (portrait), the health bar overlaps the minimap and parts of the HUD go off-screen. Godot's anchor/margin system should handle this, but the configuration is wrong.

## Setup

- `UI/HUD`: `CanvasLayer` → `Control` (fills screen)
- `UI/HUD/HealthBar`: `TextureProgressBar` — should be bottom-left, 10px from edges
- `UI/HUD/Minimap`: `SubViewportContainer` — should be bottom-right, 10px from edges
- `UI/HUD/AmmoLabel`: `Label` — should be bottom-center

## Step 1: Configure Stage to track UI nodes

By default, Stage excludes UI nodes. Add them to the tracked types:

```json
{
  "op": "spatial_config",
  "extra_tracked_types": ["Control", "TextureProgressBar", "SubViewportContainer", "Label"]
}
```

## Step 2: Get a snapshot of the UI layout

<AgentConversation :messages="messages0" />

## Step 3: Check the anchor configuration

<AgentConversation :messages="messages1" />

## Step 4: Fix the anchors

<AgentConversation :messages="messages2" />

## Step 5: Verify at multiple resolutions

<AgentConversation :messages="messages3" />

## Godot anchor system reference

```
anchor_left=0, anchor_right=0 → pinned to left edge (x = offset_left)
anchor_left=1, anchor_right=1 → pinned to right edge (x = screen_width + offset_left)
anchor_left=0, anchor_right=1 → stretches full width (width = screen_width + offset_right - offset_left)
anchor_left=0.5, anchor_right=0.5 → centered (x = screen_width/2 + offset_left)

anchor_top=0, anchor_bottom=0 → pinned to top (y = offset_top)
anchor_top=1, anchor_bottom=1 → pinned to bottom (y = screen_height + offset_top)
anchor_top=0, anchor_bottom=1 → stretches full height
anchor_top=0.5, anchor_bottom=0.5 → centered vertically
```

For bottom-edge anchoring with 10px margin from the bottom:
- `anchor_top = 1.0, anchor_bottom = 1.0`
- `offset_bottom = -10` (10px from bottom)
- `offset_top = -50` (element is 40px tall + 10px from bottom)

## Key takeaways

- **Configure Stage to track UI types** — they are excluded by default. Add them to `extra_tracked_types` for UI debugging sessions.
- **UI positions from `spatial_snapshot` are pixel coordinates in screen space.** Compare them against screen resolution to spot off-screen elements.
- **Hardcoded pixel offsets from top-left are the root cause of most HUD resolution bugs.** The fix is always anchors + margin offsets, not hardcoded positions.
- **Verify at multiple resolutions** by changing the game window size and taking fresh snapshots.
