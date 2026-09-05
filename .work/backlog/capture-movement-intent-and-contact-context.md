---
id: capture-movement-intent-and-contact-context
tags: [stage, recording, physics]
created: 2026-09-05
updated: 2026-09-05
---

# Capture enough context to distinguish standing still from being stuck

The human reported “I get stuck on this” and saved marked clips in the Voxlar
combined starter. Recorded frames contain player position, rotation, and velocity,
but these clips advertise `include_input: false`, have no events, and the player
state includes exported settings rather than the contact facts needed here.

Read-only inspection of `clip_c2bae9b7` found less than 2 mm movement in 146 of
149 sampled intervals. Other clips also contain long stationary stretches.
Without movement intent this cannot prove the player was trying to move, and
without contact normals/floor classification it cannot explain a collision snag.
The screenshot feedback separately supplies run/frame identifiers but not an
atomic player-state snapshot; a later live inspection is not the captured pose.

Consider a bounded, opt-in movement-debug capture that makes existing input and
relevant CharacterBody3D contact information accessible alongside screenshots.
First establish which existing capture settings can already supply this, rather
than adding another recorder or a game-specific telemetry framework. Keep the
sampling boundary and gaps explicit: this is diagnostic evidence, not a promise
of deterministic gameplay replay.
