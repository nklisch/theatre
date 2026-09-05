---
id: return-reusable-node-paths-from-discovery
tags: [stage, tools, ergonomics]
created: 2026-09-05
updated: 2026-09-05
---

# Return directly reusable node paths when discovering the scene

Stage 0.3.4 `scene_tree {action: roots}` returned a root's class and bare name,
`UserPlayerSandboxStarter`. Passing that name into `scene_tree children` failed
with “Node ... not found”; the absolute `/root/UserPlayerSandboxStarter` worked.

The discovery result should be easy to use in the next inspection/action without
requiring the agent to infer Godot's path prefix. Consider including the canonical
path alongside the friendly name in roots and child listings. Do not replace
stable node identity with display labels or add a new identity protocol.

Observed during the Voxlar consumer session on 2026-09-05. This is an ergonomic
improvement, not a claim that the documented absolute-path lookup is incorrect.
