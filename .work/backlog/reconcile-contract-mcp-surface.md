---
id: reconcile-contract-mcp-surface
tags: [docs, contract]
created: 2026-07-25
updated: 2026-07-25
---

# Reconcile docs/CONTRACT.md with the actual MCP surface

CONTRACT.md documents a 9-tool `recording`-centric surface; the implemented
`clips` tool has diverged (action names, params, response shapes), and the
visual-storyboards feature added `visual_artifact` and `config` actions plus
`capture_probe`/`screenshot_gaps` status blocks that no foundation doc
mentions. Per the change-in-place principle the tool surface may evolve, but
the doc should describe current truth or be explicitly marked aspirational.
Reconcile next time CONTRACT.md is touched (see also docs/DOCS-AUDIT-REPORT.md).
