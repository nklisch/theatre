---
id: capture-movement-intent-and-contact-context
kind: feature
tags: [stage, recording, physics]
parent: null
release: null
completed: 2026-09-05
---

# Capture enough context to distinguish standing still from being stuck

Opt-in movement capture records bounded named InputMap strengths and selected CharacterBody3D contact facts alongside existing spatial samples. Saved snapshots and trajectories expose this context with explicit sampling/truncation limits. Older MessagePack recordings remain readable. Engine coverage distinguishes idle, attempted and blocked movement and verifies opt-out absence.
