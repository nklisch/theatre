# Theatre — Engineering Principles

These are the standing engineering values for this project. They guide design
and review trade-offs. Code remains the source of truth for implementation
details.

## Thin engine boundary, smart server

All reasoning — spatial math, budgeting, diffing, indexing, watch evaluation —
lives server-side in pure-logic crates. The code running inside Godot
(GDScript and GDExtension) only reports what the engine says and executes what
it is told.

### Why

Logic inside the engine is hard to test, constrained to the main thread, and
coupled to Godot's release cycle. Logic outside it is plain Rust: testable,
fast to iterate, and reusable across tools.

### Implications

- The shared core has zero Godot and zero MCP dependencies; the dependency
  graph enforces the boundary.
- New capability lands in the server/core first; the in-engine layer grows
  only when the engine is the only place that can answer.
- The Godot-side layer degrades gracefully when its binary is missing rather
  than taking the game down with it.

### Boundaries

Frame-locked data collection, physics queries, and scene-tree access must
happen in-engine by necessity. The principle governs where *thinking* lives,
not where *sensing* happens.

## Token economy as a design constraint

LLM context is the scarcest resource in the system. Every response is
budgeted, tiered, paginated, and reports its own cost.

### Why

An agent that gets dumped thousands of tokens of scene data per call cannot
sustain a debugging session. The tool's usefulness is bounded by how cheaply
an agent can hold the game's state in context.

### Implications

- Detail tiers and soft budgets with a hard cap are part of API design, not
  an afterthought; new tools inherit the same discipline.
- Summary-first, drill-down-later is the expected interaction shape; features
  that force large unconditional payloads need justification.
- Truncation is always explicit and resumable (cursors, counts), never silent.

### Boundaries

Correctness and completeness of a single deep inspection can outrank economy
when the agent explicitly asks for full detail. The budget system caps even
that.

## Agent-driven actions first

The agent is the primary operator of the running game: it observes, acts,
advances time, and verifies outcomes autonomously through the tool surface.
A human driving the game — dropping markers, triggering dashcam capture — is
an optional complement, not the default workflow.

### Why

The tool exists to close the loop between an agent and a live game world.
Workflows that require a human in the driver's seat reintroduce the slow,
lossy narration loop the project was built to eliminate.

### Implications

- Action and verification capabilities are first-class and safe to use, not
  bolted-on exceptions to an observe-only posture.
- Human-in-the-loop features (dock controls, hotkey markers) are designed as
  conveniences layered on the agent workflow, not prerequisites for it.
- Presence of the addon still never changes gameplay on its own; the *agent's*
  explicit actions, not human ritual, are the intended source of intervention.

### Boundaries

The human remains the authority on what the bug is and whether a fix worked.
"Agent-driven" describes operation of the game, not ownership of the goal.

## One source of truth for wire contracts

Shared protocol types are defined once, in one crate consumed by both ends of
the wire. Boundaries deserialize into typed structs so shape mismatches fail
early and loudly.

### Why

The system crosses several process and language boundaries (agent ↔ MCP
server ↔ TCP ↔ GDExtension/GDScript). Hand-mirrored shapes across those
boundaries drift silently; a single typed definition turns drift into a
compile error or an immediate deserialization failure.

### Implications

- Wire types live in the shared protocol crate; neither endpoint defines its
  own private version of a message.
- Responses crossing a boundary are caught by typed deserialization before
  reaching business logic.
- Schemas for tool parameters derive from the same Rust types rather than
  being maintained by hand.

### Boundaries

Godot-side GDScript glue that only passes data through need not be fully
typed; the obligation binds at the Rust boundaries where messages are
produced and consumed.

## Real-loop verification

Tests mirror the real operating loop: pure logic in unit tests, handlers
against a mock transport, and end-to-end journeys against a real headless
Godot that are never skipped.

### Why

The product *is* the loop between an agent and a running engine. Mocks can
prove the pieces; only the real loop proves the product. A regression that
survives to a user is almost always one a skipped journey test would have
caught.

### Implications

- All test layers run unconditionally and must all pass; skipping the E2E
  layer is not an accepted shortcut.
- New user-facing capability arrives with the journey that exercises it, not
  just unit coverage of its parts.
- A component that cannot be exercised in its real environment is treated as
  a design smell, not a testing inconvenience.

### Boundaries

Slow or environment-dependent tests may be marked for explicit invocation,
but the project's definition of "done" includes them passing.

## Change in place — compatibility only where developers call in

Every surface the project controls end-to-end — MCP tools, the TCP wire
protocol, internal APIs, file formats — changes in place. No versioned
schemas, no compatibility shims, no negotiation complexity. The single
exception is the GDScript API that game developers call from their own code:
that surface is external and carries a real compatibility obligation.

### Why

The MCP tools are called by agents that adapt on the next session, and the
wire protocol ships as one matched unit — both ends are always the same
version. Stability work there buys nothing and costs velocity. But a call a
developer wrote into their game code is a promise; breaking it breaks their
project.

### Implications

- Internal surfaces are redesigned freely when a better shape exists;
  migration means editing the code, not adding a shim.
- The developer-facing scripting API evolves deliberately: additive where
  possible, and any breaking change is a conscious, documented decision.
- Version-negotiation machinery is not added for surfaces the project ships
  as a single unit.

### Boundaries

The obligation covers what developers actually invoke from game code, not
the addon's internal structure or editor plumbing, which remain
project-owned.
