---
id: feedback-launcher-overlaps-consumer-hud
kind: story
tags: [stage, feedback, ui]
parent: null
release: null
completed: 2026-09-05
---

# Check feedback launcher placement against consumer HUDs

Native capture/feedback controls use configurable corner or hidden placement instead of fixed overlapping positions. Layout follows actual container size and viewport changes; tests cover four corners at 320×240 and 1280×720. Hidden controls retain shortcut access and human confirmations independently of agent-notification preferences.
