---
id: godot-script-authoring-escape-hatch
kind: feature
status: active
tags: [prototype]
parent: agent-godot-development-loop
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-04
updated: 2026-09-04
---

# Compare scripted authoring with batches

## Accepted outcome

Learning experiment only: compare representative procedural construction using existing batch tools and ordinary GDScript. Record evidence and adopt/revise/discard disposition, remove discarded experiment code. No production arbitrary execution tool or sandbox claim.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Prototype GDScript authoring for operations beyond typed tools

Parked from the Godot architecture assessment at Nathan's request.

For procedural geometry, complex resources, and repetitive construction, a short
loop against Godot's own API may be easier for an agent than hundreds of JSON
operations or a new specialized wrapper for each resource class.

Director currently exposes typed operations and sequential batches through
`crates/director/src/mcp/mod.rs` and `addons/director/ops/meta_ops.gd`. Consider a
small prototype for executing an authoring script with captured results/errors
and explicit execution context and persistence behavior.

This is a learning proposal, not authorization for arbitrary execution tooling or
a committed architecture. Compare a representative construction task using the
current batch surface versus an ordinary Godot script. Preserve convenient typed
tools for common tasks; do not invent a new scene-description language, claim a
sandbox, or promise automatic rollback for arbitrary script effects.

The existing Rust/GDExtension architecture need not be rewritten to test this.

## Experiment design

Compare the strongest practical versions of each approach, not an artificially
large handwritten JSON batch against a compact script. Construct a small repeated
node layout using a generated Director batch and an ordinary Godot script. Compare
authored code, returned errors, engine startup/operation time, and reloaded scene
structure. A second small procedural mesh case can establish whether native API
calls supply capability absent from current typed resource tools.

Use isolated temporary projects and the installed Godot binary. Both approaches
must preserve native scene/resource serialization; reload outputs through Godot
to compare meaning rather than byte-identical serialization. Do not build a
benchmark framework or infer broad performance claims from this sample. Record
whether ordinary scripts already provide an adequate escape hatch, or whether a
specific missing integration justifies proposing production tooling separately.
Remove temporary scripts/projects after retaining the decision and measured facts.

## Experiment result and disposition

Adopt ordinary developer-owned GDScript as the existing direct-API escape hatch;
do not add a production arbitrary-execution tool. Both approaches remain useful.

The corrected isolated experiment built 120 MeshInstance3D nodes with shared
StandardMaterial3D and reloaded both outputs through Godot 4.7.1. Both had 120
nodes, valid transforms and one shared material instance. Both persisted native
main.tscn and shared.tres. No procedural mesh case was attempted.

The generated-batch authoring code was 944 bytes/14 lines, excluding common test
setup. It produced 122 operations, with project_path only on the outer batch.
Two CLI invocations took 848.47 ms and 691.11 ms and both returned JSON success:true.
These are first/second invocation observations, not a verified warm-engine claim.
The ordinary GDScript was 627 bytes/22 lines and its invocation took 310.22 ms.
This small sample supports neither broad performance guarantees nor a claim
that scripts are substantially more compact than a practical batch generator.

An unknown Director property produced structured success:false data, although
its existing CLI exit status remained 0. A valid SceneTree runner with an invalid
Vector3 assignment produced parse diagnostics and exit 1 under --check-only.
Initial harness/ownership failures were corrected and excluded from the results.
Temporary projects, scripts and processes were removed. The repository was not
modified by the experiment worker. This is bounded learning evidence, not a new
sandbox, rollback mechanism, rendering benchmark or production script runner.

One standard Astra evidence/alignment pass accepted this disposition. It confirmed
that the bounded experiment supports an ordinary project-owned GDScript escape
hatch, not a new execution tool or broad performance/compactness claims. The
review did not rerun the experiment. Related misleading Director exit-status
claims are being corrected in public guidance; no production exit-code change
was authorized or introduced by this experiment.
