---
id: responsive-dashcam-capture
kind: feature
status: active
tags: [stage, recording, performance]
parent: null
blocked_by: []
related_to: []
research_refs: [.research/briefs/responsive-dashcam-capture.md]
mock_refs: []
created: 2026-09-05
updated: 2026-09-05
---

# Keep dashcam capture responsive

## Outcome

Dashcam recording must not make Voxlar's controls unusable. Prefer delayed or skipped visual evidence over interrupting gameplay, with explicit coverage and availability reporting.

## Accepted boundary

The user selected spatial-only capture, GPU-side downsampling before readback, skip-before-expensive-work scheduling, and investigation and delivery of genuinely asynchronous readback where feasible for Voxlar's current renderer. Use only Astra agents and drive the outcome through verification, local installation and Voxlar deployment. Do not use computer-control tools for testing. Automated engine journeys and bounded measurements remain in scope. Keep recording opt-in; preserve unrelated work and consumer processes.

Do not change save-time persistence, replace the recorder with a service, render the scene a second time merely to obtain screenshots, or change Voxlar's rendering backend without a consequential choice. No unsupported promise of a hard real-time frame budget or deterministic pixels-and-physics correspondence.

## Current evidence

Voxlar uses Godot 4.7.1 and Compatibility/OpenGL. The recorder currently calls `Texture2D.get_image`, resizes, converts and copies pixels on the main thread before sending owned bytes to its JPEG worker. Worker queue rejection occurs after that expensive work. Previous representative debug measurements showed pacing p95 of 18.4 ms disabled, 37.1 ms Lightweight and 43.1 ms Detailed; these are not isolated Voxlar attribution measurements. The user reports dashcam is unusable in Voxlar.

## Design

**Primary lens:** performance, with renderer feasibility and retained-evidence correctness.

The [source-backed feasibility brief](../../.research/briefs/responsive-dashcam-capture.md) establishes public GPU blitting and native interop on Godot 4.7.1 Compatibility, but not verified native asynchronous integration or performance. RenderingDevice is unavailable under Compatibility. Reuse the shared configuration, recorder, encoder, gap ledger and existing engine measurement fixture. Keep Godot objects on their permitted thread and carry owned plain data across workers.

### Chosen approach

Keep one recorder and its existing JPEG worker. Add a Spatial only preset to the shared vocabulary and existing native selector; it disables new screenshots without starting recording or discarding historical images. Preserve current project startup intent, including Voxlar's disabled setting; this does not change Theatre's existing global defaults.

The visual backend choice is conditional on a first bounded engine slice proving the complete sequence: public drawable allocation and blitting, valid-context native buffer submission, and reduced pixels becoming readable on a later zero-timeout poll. Measure submission and completed-copy cost as well as pacing; moving stalls to a render callback is not sufficient. Once proven, use Godot's public drawable-texture blit to downsample the existing root viewport on the GPU. For Compatibility/OpenGL, submit the reduced texture to a bounded native pixel-pack buffer and fence, poll with zero timeout in the proper graphics context on later frames, and only map/copy completed transfers. No immediate mapping, positive fence wait, GPU finish, second scene render or implicit synchronous fallback. Isolate native graphics and function-loading code in one capture-local module; keep Godot objects on the main thread. Render callbacks may operate on native graphics handles and owned plain data only, through Godot's public render-thread scheduling boundary. Preserve native graphics state and free owned buffers/fences on their valid context. Outstanding callbacks retain capture-local resource ownership until context-valid retirement, including Stop, resize and recorder destruction. Invalidation prevents publication, not premature destruction; physically outstanding work remains counted. Fence failure or missing completion disables affected visual work with gaps/availability reporting and retires resources without waiting. No callback may use a freed/replaced texture name.

Use one pending readback initially to preserve image ordering. Admission counts pending readback, encoding and completed-but-not-ingested images against the existing queue setting; check capacity before any expensive work. Keep one encoder alive through normal setting changes and invalidate stale capture generations rather than waiting for encoding on Stop. Carry capture-request frame/time and settings to completion; completion time is not the image's physics frame. Reset worker image-comparison history and recorder anomaly continuity when the capture generation changes, even when dimensions remain equal, so discarded pixels cannot seed new anomaly markers. Drain without waiting, retain existing no-catch-up scheduling, and reuse the gap ledger. Saved windows must describe unfinished images at save time without waiting or changing the SQLite persistence contract.

Provide explicit synchronous readback as an opt-in recovery choice for unsupported platforms/renderers, not an automatic normal-capture fallback. Automatic mode selects the supported nonblocking path or reports visual capture unavailable while retaining spatial recording. Native controls and status distinguish current image availability from historical buffered images. Keep loading optional: absence of the graphics capability must not prevent Stage or the game from starting.

New attribution evidence shows spatial collection is also a primary cost. Correct repeated work in the existing collector where behavior can be preserved, rather than hiding it behind lower sampling rates or introducing caches, node registries or new tracking policies. In particular, inspect property-list enumeration/name conversion for exported script state. Qualify this optimization with real exported-state observations, not only timing.

### Alternatives and risks

CPU resize after readback and moving `get_image` into another callback do not remove synchronization. RenderingDevice is unavailable on Compatibility; switching Voxlar to another renderer or requiring a custom engine is outside this repair. A SubViewport adds unnecessary rendering-order/lifecycle ownership when public drawable blitting already exists. More than one native pending slot and independent backpressure configuration are deferred unless measurements demonstrate the need. Native buffer/fence lifetime is necessary to avoid use-after-free and premature mapping, not a generalized job system. Driver submission, memory allocation and GPU contention can still cost time: measure the result rather than promising hard real-time behavior.

### Baseline attribution

The existing four-profile fixture now compares identical Lightweight spatial cadence with and without images. Before product changes, physics pacing p95 was 18.34 ms disabled, 34.40 ms Lightweight, 34.17 ms Lightweight without images, and 43.00 ms Detailed. Physics-processing p95 was 1.05/37.67/35.95/45.69 ms respectively. Capture readback EMA was 5.37 ms Lightweight and 9.27 ms Detailed, zero without images. These are debug-extension measurements in the existing 64-moving-Polygon2D fixture, not Voxlar gameplay measurements. The visual optimization must be paired with reduced spatial collection cost to address the observed recording impact.

A bounded consumer baseline also ran the unchanged combined starter after surface readiness, using the deployed release Stage extension, debug Voxlar extension, Compatibility renderer, fresh isolated user data and private ports. Across five-second idle-camera samples, render pacing p95 was 18.218 ms disabled, 62.252 ms Lightweight without images, and 63.744 ms Lightweight; maxima were 22.579/77.779/68.009 ms. Visual readback EMA was 6.993 ms. These are short idle-scene observations, not full interactive gameplay qualification. The existing nonfatal Voxlar tool-GUI focus diagnostic appeared in all profiles and is not claimed fixed.

## Acceptance evidence

- Spatial-only selection preserves spatial/marker behavior and performs no periodic pixel readback.
- Visual scheduling rejects work before readback when capacity is exhausted, never catches up in bursts, and exposes skipped/unavailable evidence truthfully.
- Supported optimized capture transfers reduced images without a second scene render; genuinely asynchronous claims require source and engine evidence, not simply moving synchronous work to another callback.
- Preserve image dimensions/orientation, capture-time provenance, resize/configuration/stop lifecycle and existing retained-analysis behavior.
- Compare baseline and delivered pacing/capture cost using existing automated graphical fixtures, including a bounded Voxlar-relevant check without computer tools. Report renderer and build profile with limitations.
- Run required workspace, ignored real-engine, lint, format, documentation and deployment checks. Complete one independent Astra design review and one integrated implementation review at standard weight, adjudicate findings, reconcile affected foundations and close this item.

## Progress

The native backend feasibility condition is satisfied on the current Linux/Godot 4.7.1 Compatibility setup. Four focused engine journeys passed, including separate rendering-thread operation, reduced dimensions/orientation, delayed request provenance, pending lifecycle transitions, explicit synchronous recovery, headless degradation and a loading stall longer than two seconds. Pending work retains its ownership; elapsed time alone does not disable capture. Stale failures cannot poison corrected configurations. The gdext high-level callable limitation and narrow local workaround are recorded in the backlog.

Integrated native-control and preset journeys pass, as do schema generation for 60 tools and the documentation build. Windows/macOS runtime qualification is unperformed. Workspace build, formatting, warnings-denied linting, test deployment and 611 ordinary workspace tests pass. The corrected native/readback target passed five journeys and the live target passed 16 journeys. The complete rerun passed 324 tests and failed two: the excluded AccessKit undo crash and a movement-fixture startup race retaining a default-cadence sample before its deferred configuration. The fixture now stops the directly constructed recorder before deferred setup, then starts with the tested cadence. A first attempt to use project intent alone did not affect this direct-construction fixture; the full suite did not run after that failed focused attempt.

The single independent integrated implementation review found no blocking correctness or memory-safety concerns. Its minor finding is accepted: old-generation requests must not receive a second save-local or late-error gap after invalidation already accounted for their loss. The correction is implemented: save-local gaps include only current-generation requests, and stale native failures do not add another loss. The pending → Spatial only → immediate Save regression passes within all five native journeys. No artificial native fault-injection machinery was added. The corrected movement fixture also passes. The full post-correction gate remains pending. The known engine crash is neither repaired nor hidden by these fixture corrections. Local tool installation and the final documentation build passed.

Performance acceptance remains open. The initial integrated debug run did not improve pacing. A release deployment and repeat of the bounded Voxlar idle-scene check measured render pacing p95 of 16.263 ms disabled, 68.169 ms Lightweight without images and 54.796 ms Lightweight. Visual capture reported `opengl_async`, about 1.946 ms measured readback work, and delayed completion. Spatial collection still measured roughly 42–46 ms per sample. These short samples are subject to other host workload and do not establish a completed latency repair. A focused Astra pass attributed roughly 76% of debug collection time to Rust-side property filtering and moved that filtering into a small addon helper using the same live property list; Rust retains value conversion and a binary-only fallback. Seven snapshot regressions passed, including changing dynamic and validated usage. Identical fixture collector EMA fell from 23.886 ms to 6.334–6.988 ms in debug and from 5.493 ms to 4.204–4.223 ms in release. Host compilation activity limits timing precision. The final release deployment and consumer comparison passed. The same bounded idle-scene check measured render pacing p95 of 11.988 ms disabled, 43.070 ms Lightweight without images and 42.537 ms Lightweight; Lightweight's maximum was 46.397 ms. Measured spatial collection EMA was 33.018 ms and image readback work EMA 0.855 ms, with `opengl_async` active. Compared with the original 63.744 ms Lightweight p95, this is a substantial improvement, not a claim that recording is free or that short samples define a universal budget.

The user reports that Voxlar has become much more usable with the deployed asynchronous path. This is meaningful human acceptance evidence; remaining work is bounded final qualification and review, not a new universal frame-time target or open-ended optimization effort.

## Execution

The parent owns design synthesis, integration and acceptance. Source research is recorded in the linked brief. The standard independent Astra design review is complete; its conditional-feasibility, resource-retirement, analysis-continuity and foundation-reconciliation findings are accepted and reflected above. No repeat design review is required. Backend implementation begins with the bounded feasibility slice; spatial collection and native controls are independent implementation surfaces. Astra owns backend/recorder/shared configuration and its focused engine journey; a separate Astra owns collector/native controls and existing control/performance fixtures. The parent owns documentation, final measurement, integration, independent implementation review, local installation/deployment and closure. Reuse the existing shared Cargo target with `RUSTC_WRAPPER=`; remove any newly created isolated build directories and scratch artifacts on completion.
