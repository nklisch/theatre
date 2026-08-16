---
id: stabilize-watch-freeze-journey
kind: story
tags: [stage, testing, live-journey, regression]
parent: null
release: null
completed: 2026-08-15
---

# Stabilize the watch freeze journey

The watch gameplay journey now freezes the patrol before establishing its
second delta baseline, then applies the independent health change that the
delta must observe. This excludes legitimate pre-freeze movement without
weakening assertions. Both CLI and MCP variants and the complete Theatre test
suite pass.
