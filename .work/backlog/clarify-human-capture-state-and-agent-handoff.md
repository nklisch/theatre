---
id: clarify-human-capture-state-and-agent-handoff
tags: [stage, recording, feedback, ux]
created: 2026-09-05
updated: 2026-09-05
---

# Make capture state and agent handoff obvious

A human reported clicking markers and expected the agent to see videos. The
agent had received a feedback note and one screenshot, but had not received a
current marked clip. Stage 0.3.4 `clips status` reported recording disabled with
empty buffers; `clips list` contained only two August 16 captures. The exact
state at the earlier clicks was not observed, so silent marker loss is a
hypothesis, not a reproduced defect.

After recording was explicitly enabled, new marked clips appeared. The human
still had to say “clip saved” in chat, and the agent queried a separate tool to
find them. Feedback notices advertise pending screenshot feedback, not clearly
the relationship between notes, markers, saved clips, and the current run.

Consider clearer distinctions between Share Feedback (note/still image), Mark
(recorded context), and Save Clip. Make disabled/buffering/saving/saved states
and the resulting clip identifier visible. Consider current-run/newest filters
or a concise newly-saved-evidence notice so old clips are not mistaken for the
current reproduction. Do not silently enable expensive capture or imply that a
still image is a video.

## Maintainer direction

The maintainer explicitly requested an easy way to start the dashcam and a
marker hotkey whose existence and operation are obvious. The current experience
does not make it clear what shortcut is available or whether pressing it worked.

Preserve that direction when designing the interaction: a visible start/stop
entry point, discoverable displayed marker binding, and immediate acknowledgement
of a successful marker or an explanation when capture is disabled. A configurable
binding is worth considering to avoid conflicts with consumer game controls;
no particular key has been selected. These are backlog directions, not an
implemented UI or a confirmed defect in any specific existing key binding.

Related: [human-facing timelines](dock-visual-timelines.md). Reads currently
repeat the full pending-feedback notice until globally handled; consider a
less noisy acknowledgement/discovery flow without treating “read” as “fixed.”
