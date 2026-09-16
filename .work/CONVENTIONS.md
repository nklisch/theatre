---
owner: workbench
schema: 1
workbench_version: 0.24.3
completed_items: summarize
review_weight: standard
simplification_posture: balanced
autonomy: adaptive
execution_posture: adaptive
commit_posture: adaptive
---

# Workbench Conventions

## Project verification

Run from the repository root with Rust and Godot available. `GODOT_BIN` can
select the Godot executable. Engine journeys require a deployed GDExtension;
windowed visual journeys also require a working graphical session.

Use the smallest evidence set that exercises the changed behavior at a stable
boundary. Ordinary implementation should normally use formatting, Clippy for
the affected package or workspace as appropriate, and no more than three focused
test invocations. Prefer one representative test or journey per materially
different affected boundary; do not repeat a green check after documentation or
other changes that cannot affect it.

When engine behavior changes, run one representative real-engine journey. Add a
second only when the change spans a genuinely distinct boundary such as headless
and windowed behavior or Director and Stage. Deploy the test project only when
the selected journey needs the updated addon payload. Report unavailable platform
or rendering evidence rather than substituting a broad mock suite.

Expand verification when a genuine failure, unusually broad shared-boundary
change, release preparation, or explicit user request provides a reason. A
failure normally earns one diagnostic reproduction and one post-fix rerun of the
affected check; it does not automatically require every repository suite. The
commands below are available for that escalation or for CI/release qualification,
not as a mandatory sequence for every delivery:

```bash
cargo build --workspace
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p theatre-cli -- deploy tests/godot-project
cargo test --workspace
cargo test --workspace -- --ignored --test-threads=1
```

Do not introduce feature flags that silently omit selected journeys. Record the
focused commands actually run and their results; do not claim exhaustive coverage
from a targeted check.

For documentation- or workflow-only changes, validate the affected documents,
references, generated artifacts, and Workbench/research substrate. Do not claim
runtime verification from those checks. Public site changes use its existing
`site/package.json` generation/build scripts.

## Documentation and authority

Durable contributor foundations live in root `docs/`: `VISION.md` owns purpose
and boundaries, `PRINCIPLES.md` owns engineering decision rules,
`ARCHITECTURE.md` owns component and engineering structure, `CONTRACT.md` owns
cross-boundary semantics, and `JOURNEYS.md` owns observable operating workflows.
Use focused documents rather than competing specifications. Scope-owned
foundations may live under a sub-project's `docs/` when it has a distinct durable
ownership boundary.

Code and generated schemas own structural contracts. Foundations explain
semantics, constraints, and rationale without duplicating parameter catalogs.
Public product guides and generated tool references live in `site/`.
`.work/` owns active work, deferred ideas, designs, verification evidence, and
completion state; none of those belong in foundation prose.

Write for contributors and coding agents in plain technical prose with
progressive disclosure: purpose first, then ownership and workflows, then
constraints and deeper references. Prefer Markdown trees, tables, and Mermaid
where they make relationships clearer; do not add a diagram toolchain merely
for documentation. Agent operating rules live in `AGENTS.md`; the portable
implementation-pattern catalog lives in `.agents/skills/patterns/`.

Roadmaps remain user-owned and unmanaged by Workbench. No additional Workbench
release gates are configured; existing engineering verification and release
rules still apply.

## Overbuilding calibration

Theatre is a local Godot toolkit for developers and coding agents. Dependable
edits and real engine feedback justify typed boundaries, native serialization,
targeted validation, and real-engine tests. Extra services, speculative tool
wrappers, compatibility layers, and transaction machinery need concrete
workflow or failure evidence rather than architectural completeness. Preserve
measured capture performance and meaningful guarantees when simplifying.
Revisit this guidance when real agent sessions, deployment failures, or measured
engine limits show that the current approach is insufficient.
