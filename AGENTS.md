# Theatre — Agent Instructions

Theatre connects coding agents to Godot. Director authors project content;
Stage observes and interacts with a running game. The Theatre CLI installs,
configures, and deploys both.

## Read the owning guidance

- [Architecture](docs/ARCHITECTURE.md): crate ownership, dependencies, runtime,
  persistence, platform support, and generated references.
- [Contracts](docs/CONTRACT.md): boundary semantics and field naming.
- [Principles](docs/PRINCIPLES.md): engineering trade-offs.
- [Journeys](docs/JOURNEYS.md): supported operating workflows and limitations.
- [Workbench conventions](.work/CONVENTIONS.md): verification commands and
  delivery defaults.
- [Project patterns](.agents/skills/patterns/SKILL.md): focused implementation
  references. Load relevant Godot, gdext, rmcp, and schemars skills when working
  on those surfaces.

Code is the implementation authority. Foundations explain durable semantics
and ownership; generated schemas describe tool structure. Do not promote a
proposal, old example, or roadmap aspiration into a current guarantee.

## Repository paths

Use repository-relative paths for files that belong to this Git repository and
are not ignored by Git. Do not record local machine paths for these files.

If a file should remain local-only, notify the user that it needs a
`.gitignore` entry. Do not change `.gitignore` until the user confirms the
change.

## Coding invariants

- Keep stdout protocol-clean. In MCP serve mode it carries the protocol; in
  CLI mode it carries JSON results. Send logs to stderr through `tracing` or
  `eprintln!`, never `println!`.
- Access Godot objects and the scene tree only on the engine main thread.
  Never send `Gd<T>` across threads; worker threads may process owned plain data.
- Keep `stage-core` independent of Godot and MCP. `stage-godot` depends on
  `stage-protocol`, not `stage-core`; server-side reasoning belongs outside the
  engine boundary.
- GDScript owns the editor-plugin lifecycle. Use GDExtension classes through
  that glue, not as an EditorPlugin base.
- Follow Rust edition 2024 and workspace dependency/version ownership. Use
  `anyhow` for application errors, library error types for reusable boundaries,
  and explicit MCP error conversion at tool handlers.
- Do not unwrap in library code. Tests and deliberate executable setup may use
  unwraps. Follow the existing serde naming and default conventions.
- Parameters must have an implemented effect or be explicitly rejected. Follow
  [contract naming rules](docs/CONTRACT.md) rather than inventing field aliases.
- Verify with a small, risk-based set of focused checks. When engine behavior
  changes, include one representative real-engine journey and add another only
  for a materially distinct boundary or a genuine failure. Use the escalation
  commands and repetition rules in `.work/CONVENTIONS.md`; do not run every
  ignored environment-dependent test for ordinary delivery.

## Consumer independence

Keep Theatre independent of individual consuming projects. Project-specific
readiness conditions, metric providers, scene conventions, and input bindings
belong in the consuming project, not Theatre's runtime or shared defaults.
Do not identify a consumer by name or path to select behavior: that makes the
same Theatre contract behave differently across projects.

When adding integration contracts, test them with self-contained Godot fixtures.
Consumer projects can supply additional evidence, but must not become required
dependencies for Theatre's tests or releases. Treat reference implementations
and their measurements as evidence, not automatic product requirements.

## Installation and deployment

`theatre install` builds and installs binaries and addon templates.
`theatre init` copies from the installed template directory, not the source
checkout. Use `theatre deploy` when changes in this repository must reach a
Godot project. Generated MCP commands are the portable names `stage` and
`director`; the agent process needs their installation directory on `PATH`.
The legacy `scripts/theatre-deploy` helper remains supported, but new workflows
use the CLI.

Keep Windows support additive to Unix workflows. The test project uses tracked
addon symlinks: native Windows checkouts need Git symlink support and Developer
Mode or equivalent permission. Preserve platform-aware deployment rather than
hard-coding shared-library suffixes or a local executable path.

The GDExtension targets Godot 4.7 through gdext's `api-4-7` and
`lazy-function-tables` features. Keep the API target and manifest minimum
compatible; do not infer compatibility from a Godot version string alone.

## Git and releases

Use short imperative commit subjects, normally at most 72 characters. Do not add
AI co-author trailers or agent attribution to commits or pull requests.
Preserve unrelated worktree changes.

Use `scripts/release.sh patch`, `minor`, `major`, or an explicit version for an
explicitly authorized release. It updates version locations, commits, tags,
and pushes; CI builds and publishes platform bundles. Do not run it as part of
ordinary delivery or Workbench release-summary preparation. Never update
version strings manually; add new versioned locations to the release script.

<!-- workbench:start -->
## Workbench

This repository is Workbench-owned. For stateful Workbench work, read
`.work/CONVENTIONS.md`, relevant foundation documents, and the selected skill
before acting. Follow that skill's required references.
Never run setup without explicit user direction. Keep unrelated requests
outside Workbench.

Use `work` to own a continuous outcome, drawing on `design` for consequential
choices and `deliver` for ready implementation without restarting the workflow.
Reuse unchanged context. Use `ideate` for valuable early exploration, `scan` for
opportunities without remediation, `park` for selected out-of-scope findings,
and `release` only for a requested versioned summary.

The user's request and effective autonomy posture define the authorized
boundary. Ask about consequential requirements; do not invent requirements,
expand scope, or treat repository aspirations as current work. Use features as
the normal delivery unit; follow `work/references/lifecycle.md` for hierarchy and
splitting. Keep independent items parallel; `blocked_by` needs a real dependency.
Completion cleanup is mandatory: summarize or discard per conventions, remove
completed active files and attachments; see the same lifecycle reference.

Before any design or review, including a loose request, apply the current
`## Overbuilding calibration` from `.work/CONVENTIONS.md`. Loose work gets the
lens without other Workbench mechanics. Pass it to delegated roles rather than
assuming fresh context inherited it.

The work item is the contract between design, review, and implementation. The
assigned designer authors and revises its design directly. The outcome owner
adjudicates scope and acceptance. Implementers read the recorded contract, not
an orchestrator's reconstruction. Optional linked design attachments belong to
the item and are always deleted when it completes, even if a summary is kept.

`.work/` is the operational record; foundation documents describe durable
project truth, including the engineering shape contributors need to build and
operate the repository coherently. Only write durable artifacts named by the
active workflow. Questions, proposals, progress, recommendations, and
completion reports belong in chat. Keep human-facing documents clean and
self-contained: lead with
business or real-world meaning, define important non-obvious domain concepts
before using them, and omit agent history or review narration.

Apply configured execution, review, simplification, and commit postures.
Align optional design review once per run. Choose adaptive implementation review
boundaries, including shared reviews across features or deliveries. Verify each
unit promptly and keep deferred review visible until the owning items can close.
Scale effort to the work. Quick implementation and focused review often benefit
from the current context; use another when it adds enough value or is requested.
Test meaningful behavior at stable
interfaces, verify the full requested boundary, reconcile affected foundation
truth and indexes, and close completed work. Reviewers propose; the outcome
owner verifies and adjudicates. Park valuable adjacent findings instead of
silently adding them to scope.
<!-- workbench:end -->
