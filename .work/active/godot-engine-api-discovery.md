---
id: godot-engine-api-discovery
kind: feature
status: active
tags: [godot]
parent: agent-godot-development-loop
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-04
updated: 2026-09-04
---

# Targeted engine API discovery

## Accepted outcome

Query installed Godot for focused classes/properties/types/signals/enums and relevant defaults. Bound results and demonstrate value on representative authoring failures. No parallel catalog.

## Progress

Implemented the typed `engine_api` boundary, ClassDB-backed operation, surgical
backend/CLI dispatch, focused contract tests, and real-engine authoring evidence. The query
defaults to a class summary; focused categories support exact selection, inherited-member
ownership, and bounded deterministic pages. Unsupported complex defaults use explicit
text-only or unavailable representations rather than a round-trip claim.

Focused Rust verification passes (1 module unit test and 2 router/CLI tests). Seven ignored
real-engine tests pass serially against Godot 4.7.1, covering scene/resource authoring from
discovered metadata, properties/methods/signals/enums, inherited members, unknown inputs,
pagination, non-scalar fallback, daemon dispatch, and batch dispatch. A direct headless
probe also exercised the editor dispatch fallthrough.

The standard-review correction filters ClassDB Inspector category/group/subgroup headings
before property counts, exact lookup, pagination, and declaration mapping while retaining
real properties without Inspector usage. Raw GDScript entry points now reject fractional or
nonnumeric offsets and limits before integer conversion while accepting integral JSON numbers.
The focused real-engine suite remains 7/7 after assertions for heading removal, retained
non-Inspector properties, and raw pagination validation. Scoped formatting and diff checks
pass. No second review cycle was run.

The parent epic owns generated references, foundations, integrated review, and closure.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Discover the installed Godot engine's API

Parked from the Godot architecture assessment at Nathan's request.

An agent would benefit from querying the installed engine for class properties,
expected types, methods, signals, enum values, and relevant defaults instead of
guessing from model memory or discovering mistakes through failed edits.

`crates/director/src/mcp/mod.rs` has no dedicated public API-discovery tool in the
inspected router. Engine-side property validation already consults property
lists in `addons/director/ops/node_ops.gd`; resource tools accept class names in
`crates/director/src/mcp/resource.rs`.

Explore targeted, bounded engine-native discovery. Avoid dumping the entire
ClassDB into context or hand-maintaining a second Godot API catalog. Measure
usefulness on actual resource/scene-authoring failures before expanding coverage.

## Design

**Primary lens:** new work.

Expose one focused Director engine API query, backed by ClassDB in the selected
Godot process. Inputs select one class and one member category (summary,
properties, methods, signals, or enums), optional exact member name, and bounded
pagination. Default to summary rather than dumping inherited engine metadata.
Return engine version and actual class identity with total/next-page information.
Property detail includes native type/hint metadata and the engine-reported default
where meaningful. Enum detail pairs names with the engine's integer constants.
Use existing Godot Variant serialization for non-JSON defaults. Rust boundary
types own tool structure; ClassDB owns engine structure.

The installed Godot and bundled 4.5 API both expose the needed ClassDB methods,
including class_get_property_default_value. Do not instantiate arbitrary user
scripts, scrape a website, build a catalog, or cache across engine versions.
Unknown classes/members and invalid pages produce actionable errors.

Verify a real engine query supplies usable property/type/default/enum information
for a representative scene/resource authoring operation. Check inherited members,
unknown class, exact member selection and explicit pagination. Run normal typed
router/schema tests and generate the public tool reference from the router.
This is local integration with existing dispatch, not a new discovery subsystem.

One standard Astra design review completed. Existing Variant serialization does
not structurally cover every native type. For unsupported defaults, report the
native type and explicit unavailable/text-only representation rather than claim
round-trip JSON. Include a non-scalar default in the real engine check. Do not
build a universal Variant conversion subsystem for discovery.
