---
id: agent-human-feedback
kind: feature
status: active
tags: [godot, integration]
parent: agent-godot-development-loop
blocked_by: []
related_to: [stage-immediate-viewport, godot-project-session-identity]
research_refs: [.research/briefs/theatre-client-feedback.md]
mock_refs: []
created: 2026-09-05
updated: 2026-09-05
---

# Share human feedback with the agent

## Accepted direction

Nathan selected feedback from both the running game and Godot editor. Queue
selection/pointer context, an image and any supplied note for later retrieval.
A small pending-feedback notice should appear on the next tool/API response.
He requested investigating Claude/Codex plugin or hook packaging so a working
agent can receive that context without requiring another Theatre-specific call.

No automatic idle-agent wake or forced Stop-hook continuation is selected.
Do not confuse next-boundary context injection with asynchronous steering.
Do not consume the evidence merely by displaying its pending notice.

## Design readiness

Primary lens: data and integration, with UI/UX and privacy considerations.
The source-grounded hook findings are under
.research/attestations/theatre-client-feedback-*.md. Both clients document
PostToolUse context injection, with separate plugin formats and trust/availability
conditions. Actual installed-client behavior must be verified before claiming
client integration passed. The reviewed design below governs the implemented
queue, shared handling, project routing and native editor/game composers.
Implementation evidence and remaining host/integration limits are recorded below.

## Verification boundary

Demonstrate a deliberate share from editor and running game, pending notice,
retrieval of matching context/image/note, and explicit handling without another
client silently consuming the evidence. Check project isolation, bounded storage
and absent/disconnected engine behavior. Exercise actual supported client hooks
where available and disclose unavailable host verification. Preserve human scene
edits, gameplay input and the continuous recorder path.

## Design

**Primary lens:** data and integration, with UX/privacy overlays. Balanced
simplification. Nathan confirmed shared project-level handling and persistent
project-local `.theatre/feedback/` storage. He explicitly approved its Git ignore
entry. Reads and notices never handle or delete evidence. Handling suppresses
pending notices for all readers but leaves evidence retrievable. Deletion is a
separate deliberate operation. No per-reader delivery registry is needed.

### Capture and interaction

Use native Godot controls, not another application. Provide a distinct share
affordance in the runtime and Director editor integration, preserving existing
marker behavior and the runtime marker API. A small composer shows the captured
image and available context, accepts an optional note, and queues explicitly.
Capture before showing that composer. Do not pause gameplay, change selection,
mark scenes dirty or save scenes. Do not intercept ordinary gameplay clicks or
infer a runtime node from the cursor position. Runtime pointer coordinates remain
spatial evidence, not a claim of object selection.

Copy selection, pointer, source dimensions, scene and engine/process identity when
capture starts. Bind any later image completion to that same request, not mutable
selection or a latest-image slot. Reuse the existing synchronous capture shape
where possible. State readback timing honestly. Headless/unavailable images leave
context and notes usable; an exited runtime cannot produce fresh pixels.

### Storage and consumers

Godot publishes one immutable item directory containing typed metadata and an
optional image, using a temporary directory followed by same-filesystem rename.
This prevents readers seeing half-written evidence without a queue index or
transaction framework. A small shared Rust module/crate serves Stage, Director
and CLI readers; a shared GDScript producer helper works with either addon and
does not make Director depend on the Stage extension. No extra plugin lifecycle.

Use the selected canonical project, not a display name or live connection, to
locate the queue. Hooks read files directly and must not take over live Godot
connections. Captured run/process identity describes the original evidence,
including after the game exits or a client reconnects. Editor-to-game commands,
if needed, target the selected debugger session rather than broadcasting.

Keep handled annotations separate from immutable evidence. Bound individual
images, notes and admission. Do not expire or evict unhandled feedback silently.
At capacity, preserve existing items and the unsent composition with clear
cleanup guidance. A small simultaneous-producer overrun is acceptable; no exact
global byte-ceiling guarantee or lock service. Explicit deletion releases space.
The ignore entry prevents accidental commits, not local access or encryption.

### Tool and client surfaces

Expose list/status, retrieve, handle and delete through one feedback operation
family in both MCP servers and the CLI. These work without a live engine and do
not launch a Director backend. Retrieval includes the retained image as an MCP
image block. At ordinary tool completion, append a compact pending notice without
altering the typed operation result, image content or success/error meaning.
Queue failures cannot turn a successful authoring operation into an apparent
failure. CLI output remains valid JSON, with a named notice field rather than
extra stdout text.

Ship separate small native Claude and Codex plugin packages around one CLI hook
helper. Their synchronous PostToolUse hooks read the local queue and return
supported textual context. A hook neither handles feedback nor inserts base64
as fake image context. Image retrieval remains explicit, as Nathan's requested
notice-and-retrieve flow states. Do not duplicate existing MCP registrations.
Installation and client activation/trust remain separate and explicit. Never
edit global client settings silently. Unsupported or disabled hooks leave the
standalone MCP workflow complete. No Stop continuation, asyncRewake, monitor,
idle wake, or claim of asynchronous steering.

### Verification and alternatives

Reuse existing Rust and real-engine fixtures. Verify matching editor/runtime
context, note and decoded image; publication interruption; project isolation;
two non-destructive readers; handling versus deletion; capacity failures; paused,
headless, exited and disconnected states; unchanged gameplay and editor dirty
state; and notices preserving ordinary results, errors and mixed images. Test
actual trusted Claude2.1.251 and Codex0.153.0 sessions receiving hook context after
a non-Theatre tool where available. Version probes or synthetic JSON alone do not
establish host integration. Disclose any unavailable actual-host verification.

An engine-memory queue loses evidence on exit and forces live connections. Clips
couple editor sharing to runtime recording. SQLite adds a new producer boundary
without a demonstrated need. Reject those alternatives for this scope, alongside
watcher services, exactly-once delivery, speculative migrations and receipt
registries. Native layout and conservative per-item bounds are reversible local
choices. Reconcile persistence/ownership in ARCHITECTURE, delivery/handling in
CONTRACT and the share/retrieve journey in JOURNEYS after implementation.

The single standard design review is complete. Its refinements below govern implementation.

### Review refinements

One standard Astra design review accepted the approach with three minor
clarifications. Own the shared GDScript source as an addon support payload (for
example addons/theatre_shared), not a third plugin. Include it in existing
install/init/deploy paths whenever either addon needs it, and verify Stage-only,
Director-only and combined installations. Test fixtures must use the same payload.

Incomplete publication directories are not feedback. Status and explicit cleanup
must account for their storage and offer a cleanup route. Do not infer abandoned
writers from age, add process tracking, or silently remove another active capture.

Runtime capture uses the root viewport. Editor capture uses the active 2D or 3D
scene viewport, not the entire desktop. Pointer coordinates are local to that
captured surface. Preserve source and output dimensions so resizing has a clear
mapping; outside/unavailable pointers remain explicit. Verify a scaled surface and
an editor viewport. Do not add a general coordinate system abstraction.

## Implementation evidence

The feedback worker implemented `crates/theatre-feedback` (typed directory
reader, shared handled annotations, cleanup, best-effort notices and MCP image
rendering), `addons/theatre_shared` (synchronous capture/publication and native
composer), Stage runtime and Director editor entrypoints, both feedback MCP/CLI
families, and `theatre feedback` / `theatre feedback-hook`. No queue index,
service, watcher, receipt registry or third Godot plugin was introduced.

Install/init/deploy and release packaging carry the shared support payload;
install/deploy also copy separate `client-plugins/claude` and
`client-plugins/codex` packages. Init/deploy add only the approved feedback Git
ignore entry. The linked Godot test project has the same support payload.
Source deployment does not replace the Theatre CLI itself; install the new CLI
before using its optional hook helper.

Focused verification passed:

- `cargo build -p stage-server -p director -p theatre-cli -p stage-godot`.
- Five shared queue tests: non-destructive independent readers, project
  isolation, handling versus deletion, incomplete publication/explicit cleanup,
  preserved MCP result/error/image meaning, and MCP root-object schemas.
- Five explicitly invoked Godot 4.7.1 journeys: editor 2D and 3D viewport capture
  with unchanged selection/dirty state; headless/paused composition and bounded
  admission; a 1920×1080 runtime with 960×540 logical input coordinates and
  1280×720 output image; and the actual Stage-only runtime entrypoint without an
  agent or recorder dependency, followed by sharing with the recorder active,
  an exercised silent marker and continued physics frames. Images were
  JPEG-decoded, including MCP image blocks. Evidence remained readable after the
  engine exited.
- CLI feedback/hook helper integration (including a non-Theatre tool result
  larger than 1 MiB), two focused helper/deployment unit tests, and all 41
  existing Theatre CLI integration tests. A Stage CLI regression test also
  proves disconnected retrieval and pending notices on early validation/session
  errors without changing exit codes. Single-addon support copying and the
  narrow/idempotent ignore entry are covered.
- Both real MCP stdio binaries were exercised against one isolated project:
  status/retrieve through each server, shared handling across servers, pending
  notice on stopped-runtime status, preserved Stage and Director MCP errors,
  an ordinary Director scene-list result, and disconnected CLI retrieval through
  both binaries. An initially invalid tagged-enum output schema was corrected
  and the real-server probe rerun successfully.
- Scoped clippy for the feedback, CLI, Stage server and Director crates with all
  targets and warnings denied. Shared Godot producer/composer scripts were
  loaded by real engine tests, not only checked as text.
- Native composer rendering was inspected. The first visual pass exposed a
  wrapped-label minimum-height bug hiding Queue feedback. Bounded scrolling
  context/error controls and a native dialog corrected it; real-engine tests
  now assert that the composer and Queue button fit on screen.

Client packaging was exercised with the real hosts, in temporary project and
settings directories: Claude's plugin validator accepted the package (optional
version/author warnings); Codex's native marketplace-add and plugin-add commands
installed `theatre-feedback@theatre-local`. Interactive launches reported
Claude 2.1.261 and Codex 0.153.4, newer than the earlier inspected versions.
Claude remained at isolated onboarding. Codex initially required sign-in; a
follow-up referencing its existing native authentication file (without copying
credentials or settings) reached the explicit directory-trust prompt instead.
No project or hook trust was granted, and no bypass flag was used. **Actual
trusted PostToolUse context injection remains unverified and requires
human/parent assistance.** These launches are packaging/availability evidence,
not a successful model-context hook journey.

Bounded parent integration copied `addons/theatre_shared` into the real-editor
authoring fixture whenever Director is present, while retaining the optional
Stage branch. The fixture now launches Godot with its default accessibility
behavior rather than forcing `--accessibility disabled`. Director's `editor_run`
CLI success, typed-parameter error and operation-error envelopes append the
same best-effort pending notice as other CLI branches without changing their
existing fields or exit codes.

Focused verification passed: `cargo test -p director --test cli_integration
editor_run_ -- --nocapture` reported 3 passed and 5 filtered out; `cargo build -p
stage-server` passed and supplied the Stage binary required by run control. The
latest full default-accessibility command `cargo test -p director --test
authoring_editor -- --ignored --nocapture --test-threads=1` reported 1 passed and
2 failed. Run control passed, including its CLI success notice; both native-undo
journeys missed their first Ctrl+Z. Across repeated full runs, run control passed
after the Stage build, while the sibling authoring journeys either missed the
shortcut or one editor exited after AccessKit panicked on a duplicate child.
One isolated history-journey retry passed after a startup delay, but the delay did
not stabilize the full suite and was removed along with the other ineffective
fixture experiments. Earlier separate Godot 4.7.1 probes of those two journeys
without an accessibility override passed, so this is recorded as a reproducible
current full-suite failure rather than a universal platform-safety claim. Final
`cargo fmt -p director -- --check` and owned-path `git diff --check` passed.

Parent still owns final foundations/site/schema generation, standard integrated
implementation review, wider verification and closure. This item remains active;
no completion claim or commit was made.

Nathan explicitly approved directory and plugin-hook trust for isolated native
Claude/Codex test projects using existing logins. This approval excludes global
trust changes, real-project approvals and credential copying. Actual-host delivery
verification is continuing under that scope.

### Actual native-client verification and accepted limitation

Codex 0.153.4 passed the isolated native journey. Only its temporary directory
and the one packaged PostToolUse hook were trusted through native prompts. An
ordinary cat command caused the pending notice to appear as hooks.additional_context
in model context. Explicit Theatre CLI retrieval and native view_image then
returned the matching randomized identifier, note, selection and image contents.
The JPEG remained byte-identical and the item remained pending and unhandled.
A second item published while idle caused no new turn during an 88-second probe.

Claude 2.1.261 reached subscription sign-in despite referencing existing native
authentication. No new authorization or legal terms were accepted. Its package
validation and helper tests passed, but actual Claude hook delivery remains
unverified. Nathan explicitly accepted shipping with this limitation rather than
starting a new login; expected similarity is not treated as execution evidence.

Temporary client processes, configurations, authentication symlinks, fixtures and
transcripts were removed. Global Claude/Codex settings hashes were unchanged.

## Standard implementation review

One standard Astra pass found no additional material code defect. It identified
a material documentation omission: manual installation did not copy the required
shared support directory. That command sequence is corrected and passed a fresh
Godot 4.7.1 headless editor import with Stage, Director, theatre_shared and the
native library, without preload or script errors. The reviewer inspected source
and supplied evidence; it did not independently rerun client or engine journeys.
No repeat formal review is required for the bounded correction.
