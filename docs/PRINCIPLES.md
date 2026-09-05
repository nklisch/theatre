# Theatre — Engineering Principles

These are the standing engineering values for this project. They guide design
and review trade-offs. Code remains the source of truth for implementation
details.

## Thin engine boundary, smart server

Spatial reasoning, response budgeting, query diffing, indexing, and watch
evaluation live server-side, with pure logic kept independent of Godot. The
engine boundary gathers state and executes engine operations. Capture-local
buffering, clip persistence, and trigger detection stay with the recorder so
recording can continue without an agent session.

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
happen in-engine. Capture-local work has a different lifecycle from agent-side
analysis; see [architecture](ARCHITECTURE.md) for that ownership boundary.
Do not move work across processes solely to satisfy a slogan.

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
- Human-in-the-loop features such as markers and shared feedback are
  conveniences layered on the agent workflow, not prerequisites for it.
- Feedback reads do not consume evidence. Handling and deletion remain explicit,
  so one client cannot silently discard another client's context.
- Presence of the addon still never changes gameplay on its own. The *agent's*
  explicit actions, not human ritual, are the intended source of intervention.

### Boundaries

The human remains the authority on what the bug is and whether a fix worked.
"Agent-driven" describes operation of the game, not ownership of the goal.

## One source of truth for contracts

Code owns structural contracts. Stage's shared wire types live in its protocol
crate; tool schemas derive from their Rust parameter types. Director's Rust
boundary types and GDScript operations must agree through boundary validation
and tests. Documents own semantics, invariants, and rationale, not a second
hand-maintained set of structures. The shared feedback crate likewise owns its
public evidence types for both servers and the CLI.

### Why

The system crosses several process and language boundaries: agent, MCP
server, TCP transport, and Godot. Duplicate structural definitions drift.
Shared types and generated references reduce that drift; cross-language
boundaries still need real tests rather than a claim of compile-time coverage.

### Implications

- Shared Stage wire types live in the protocol crate rather than private
  endpoint copies.
- Validate boundary responses before business logic relies on their shape.
- Derive tool parameter schemas and generated references from their owning
  Rust types. Prose explains behavior without duplicating those schemas.

### Boundaries

GDScript does not consume Rust types directly. Keep that language boundary
explicit and test its agreement; do not introduce a parallel schema catalog
merely to describe existing code.

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

- All required test layers must pass. Environment-dependent journeys may need a
  separate explicit invocation; the ordinary workspace test command does not
  count as evidence for them.
- New user-facing capability arrives with the journey that exercises it, not
  just unit coverage of its parts.
- A component that cannot be exercised in its real environment is treated as
  a design smell, not a testing inconvenience.
- Test meaningful behaviors, contracts, and regressions at stable interfaces,
  not every implementation detail or branch. Each test should justify its
  maintenance cost through the confidence it adds.

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

## Leave it simpler

Within the accepted scope, remove code, tests, checks, abstractions, and
compatibility paths that the change makes unnecessary. Prefer fewer concepts
when they preserve the behavior and guarantees that matter.

### Why

Theatre already crosses engine, process, language, and agent boundaries. Every
additional mechanism has a cost in understanding, deployment, and failure
handling. Simplification is useful when it removes that cost, not merely when it
reduces a line count.

### Boundaries

Preserve meaningful validation, safety, compatibility obligations, and measured
performance constraints. Avoid obvious plausible performance regressions. A
change to those guarantees needs explicit approval; an adjacent cleanup idea
belongs in the backlog rather than silently expanding the current work.
