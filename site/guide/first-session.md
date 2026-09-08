---
description: "A deliberate observe, capture, diagnose and verify workflow for a Godot gameplay issue."
---

# Your First Debugging Session

Suppose an enemy sometimes fails to notice a nearby player. You need to distinguish
a positioning problem from detection logic or collision configuration.

## Choose the evidence

If code inspection is insufficient, enable only the runtime tools needed.
With the project's editor open:

```sh
theatre run scenes/review.tscn --observe on --play light --play-scope group:review --operator human
```

Use a real review group containing the relevant player, enemy and detection
nodes. Integrate [project readiness](/guide/getting-started#integrate-readiness-before-phased-capture)
before using this default phased workflow. Ordinary launches do not record.

## Reproduce and keep a segment

1. After readiness, click **Start new** when ready to test.
2. Reproduce the failure and click **Mark**, or use the configured marker key.
3. Click **Stop**, then **Keep**. A failed save preserves the draft for retry.
4. Copy the saved reference and give the agent the symptom and relevant marker.

Manual Mark annotates; it does not silently save. If recent-history rolling
capture and automatic marker clips are preferred, opt into those policies
separately as described in [Dashcam](/stage/dashcam).

## Inspect what was actually retained

Ask the agent to inspect the clip's metadata and markers, then spatial snapshots
or trajectories near the marked time. Compare the player's recorded position
with the enemy's detection region. Use metrics for timing or explicitly supplied
counters. Light does not include images.

Do not conclude that a signal never fired merely because a channel lacks that
event. Do not treat a live `spatial_inspect` as historical evidence. If collision
layers or masks were not retained, reproduce the problem and inspect them live
or inspect the saved scene configuration. State which evidence supports each
part of the diagnosis.

For example, if a live reproduction confirms that the player is on collision
layer 1 while the detection area's mask only includes layer 2, the mismatch
explains why that area cannot detect the player.

## Fix and verify

After the user authorizes the fix, use Director for serialized scene/resource
changes. Verify its persistence result, then explicitly save only the intended
scene; preserve unrelated unsaved editor work.

Restart the selected saved scene with observation enabled. Verify the new run
identity and scene before checking the corrected mask and reproducing the same
interaction. Capture another segment only if comparison evidence is needed.

The useful pattern is: select evidence, capture deliberately, distinguish
missing data from negative evidence, make an authorized change, and verify a
fresh run. No continuous recorder is needed for every ordinary launch.
