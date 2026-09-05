---
id: offer-low-overhead-recording-guidance
kind: feature
tags: [stage, recording, performance, ux]
parent: null
release: null
completed: 2026-09-05
---

# Help users record movement bugs without distorting their feel

Lightweight and Detailed presets expose their effective settings without enabling recording implicitly. A repeatable graphical workload measures capture cost; guidance explains coverage, pacing, readback and screenshot trade-offs rather than promising negligible overhead.

In the 64-moving-Polygon2D debug fixture (Godot 4.7.1, Linux, RTX 4070, compatibility renderer, anomaly detection off), recent physics pacing p95 was 18.4 ms disabled, 37.1 ms lightweight and 43.1 ms detailed. Detailed fell below real-time 60 Hz; zero queue drops did not imply smooth capture.
