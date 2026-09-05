---
owner: workbench
schema: 1
workbench_version: 0.19.0
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

```bash
cargo build --workspace
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p theatre-cli -- deploy tests/godot-project
cargo test --workspace
cargo test --workspace -- --ignored --test-threads=1
```

All test layers must pass for implementation delivery: unit, integration,
scenarios, wire tests, Director operations, and Stage/live end-to-end journeys.
The second test command explicitly runs environment-dependent tests marked
`#[ignore]`; the ordinary workspace command alone does not run them. Do not
introduce feature flags that silently omit required journeys. Report unavailable
platform or rendering verification rather than claiming it passed.

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
