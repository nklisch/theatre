---
id: make-dashcam-configuration-discoverable-and-truthful
tags: [stage, configuration, recording]
created: 2026-09-05
updated: 2026-09-05
---

# Make dashcam settings discoverable and their application truthful

Stage 0.3.4 exposes `clips config` as an unstructured JSON object. Status reports
nested `pre_window_sec` and `post_window_sec` objects, but configuration accepts
flat names such as `pre_window_deliberate_sec` instead.

Observed on 2026-09-05: submitting `pre_window_sec: {deliberate: 15, system: 10}`
and `post_window_sec: {deliberate: 5, system: 5}` returned `applied.result: ok`.
The returned effective values remained 60/30 and 30/10. Looking at
`crates/stage-godot/src/recorder.rs` revealed the accepted flat keys; submitting
those returned the requested values. Unknown keys were not explicitly rejected.

Consider a discoverable accepted-input schema or examples and explicit reporting
of ignored/invalid keys. A status-shaped object need not be accepted as input,
but a successful result should not suggest unapplied fields took effect.

There is also a coverage discrepancy worth checking independently: with effective
post-window values reported as five seconds, saved `clip_03c5973d`,
`clip_60cb4aba`, and `clip_213a3749` each span about 14.9 seconds and end exactly
on their marker frame (139943, 141059, 141245). This suggests pre-marker-only
coverage, but the intended distinction between marker/save/post-window behavior
has not been established. Clarify or correct it rather than promising an
unverified post-marker interval.
