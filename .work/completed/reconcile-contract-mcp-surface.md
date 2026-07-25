---
id: reconcile-contract-mcp-surface
kind: story
tags: [docs, contract]
parent: null
release: null
completed: 2026-07-25
---

# Reconcile docs/CONTRACT.md with the actual MCP surface

CONTRACT.md Tool 9 rewritten as the implemented `clips` tool (replacing the
aspirational `recording` tool): all 15 actions with params, status response
including capture_probe and anomaly blocks, the four visual_artifact kinds
with cache and degradation semantics, and config forwarding; summary table,
error codes, and workflow patterns updated. The clips tool description in
mcp/mod.rs now documents all actions including screenshot_at, screenshots,
visual_artifact, and config.
