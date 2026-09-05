---
id: offer-low-overhead-recording-guidance
tags: [stage, recording, performance, ux]
created: 2026-09-05
updated: 2026-09-05
---

# Help users record movement bugs without distorting their feel

Recording can affect the very hitch or movement problem under investigation.
Expose that trade-off in the ordinary recording workflow, not only raw settings.

Advisory observations from Stage 0.3.4 in the Voxlar combined starter on
2026-09-05: before capture, the reported physics interval average was about
16.7 ms. With spatial capture every two physics frames and 960-pixel screenshots
every six frames, the reported average later reached about 29 ms, the windowed
95th percentile about 62 ms, and readback average about 9 ms. This was not a
controlled before/after benchmark; movement, scene work, and other load may
contribute. It does not establish a general performance regression.

The agent lowered spatial cadence to every six frames and screenshots to every
12 frames at 640 pixels, with explicit 128 MiB state and 16 MiB screenshot caps.
That produced usable saved still sequences and player trajectories; a matched
performance comparison of the lighter setup was not performed.

Consider discoverable lightweight movement-debug versus detailed-capture presets
and a visible indication of capture cost/quality. Existing capture probes may
already provide the needed signals; this is not a request for a new profiler or
an automatic performance-control system. Keep actual effective settings with
the evidence so investigators can account for recording overhead.
