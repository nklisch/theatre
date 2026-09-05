---
name: patterns
description: >
  Theatre-specific recurring implementation patterns. Use when implementing,
  designing, reviewing, or refactoring Theatre. Read the relevant focused
  references; add new patterns only through evidence-backed extraction work.
---

# Theatre Project Patterns

This is the portable index for recurring implementation shapes in Theatre.
Code is the structural authority; use these references for rationale and
examples, and verify the relevant consumer when applying a pattern.

- [MCP tool handlers](mcp-tool-handler.md) — typed Stage dispatch and responses.
- [Director tool macro](director-tool-macro.md) — common authoring dispatch.
- [Length-prefixed TCP](tcp-length-prefix.md) — shared message framing.
- [Shared session state](arc-mutex-state.md) — state ownership and request matching.
- [GDExtension classes](gdext-class.md) — engine lifecycle and exported methods.
- [Tagged enums](serde-tagged-enum.md) — serialized dispatch choices.
- [Default functions](serde-defaults.md) — omitted parameter behavior.
- [Error layering](error-layering.md) — library and tool-boundary errors.
- [Inline test fixtures](inline-test-fixtures.md) — small pure-logic fixtures.
- [Godot test harnesses](godot-e2e-harness.md) — engine-backed verification.
- [Activity logging](activity-logging.md) — best-effort tool activity feedback.
- [Clip sessions](clip-session.md) — scoped access to saved recordings.

Related authorities: [contracts](../../../docs/CONTRACT.md),
[architecture](../../../docs/ARCHITECTURE.md),
[principles](../../../docs/PRINCIPLES.md),
[Godot naming](../godot-naming/SKILL.md), and
[verification commands](../../../.work/CONVENTIONS.md).

Keep detailed pattern bodies in their focused references, not in this index or
a generated rules digest. New references require recurring consumers and
material maintenance value; do not populate the catalog with speculative advice.
