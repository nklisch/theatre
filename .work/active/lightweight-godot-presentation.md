---
id: lightweight-godot-presentation
kind: feature
status: active
tags: [godot, windows, agents]
related_to: [agent-godot-playtest-loop]
created: 2026-09-12
updated: 2026-09-12
---

# Lightweight Godot launch presentation

## Outcome

Reduce focus stealing and accidental physical input during agent-launched Godot
runs without replacing the existing editor lifecycle or changing established
Linux behavior. Windows gains explicit automated, deferred, and interactive
presentation choices. Base and project agent guidance then select among four
workflows deliberately instead of relying on a safety-oriented product default.

This is a best-effort usability improvement, not a claim that focus stealing or
physical input can never occur. The implementation must report known limitations
honestly and leave room for later hardening when real usage demonstrates its value.

## Authoritative workflows

Presentation describes how a windowed run may interact with the workstation.
Background testing is a workflow choice that may avoid windowed presentation
entirely, so it is not a fourth `presentation` enum value. These four selection
rules govern Windows, where the new presentation behavior exists. Linux agents
retain the project's established headless, graphical, and wrapper-based launch
and test conventions; the Windows presentation modes are not prerequisites for
those existing workflows.

| Workflow | Agent choice | Expected behavior |
|---|---|---|
| Agent visual review | `presentation: "automated"` | Keep windowed rendering available to Stage while making a best-effort attempt to preserve the current foreground and suppress ordinary physical keyboard and mouse input. |
| Background testing | Headless when that retains the required evidence; otherwise `presentation: "automated"` | Use the least disruptive test path that still proves the requested behavior. |
| Prepare human testing | `presentation: "deferred"` | Prepare the windowed run without deliberate activation and suppress ordinary physical input until a separate explicit handoff. |
| Human-visible review | `presentation: "interactive"` | Permit normal visible launch, focus, and physical user input because the user requested an interactive review. |

Capture operator, presets, readiness, observation, recording, and expanded
controls remain independent from presentation. None of them grant permission to
activate a window or accept physical input.

## Recovery baseline

Implement from `main`. Commit `ab678ee579e4132ab197aa17941f8a203a9a0bc7`
on `codex/focus-preserving-godot-launches` is reference-only evidence about
Windows process and window behavior, failure modes, and useful tests. Do not
cherry-pick or simplify that commit in place.

Bring code forward from that evidence only when a specific function is the
smallest justified way to implement the selected presentation behavior. Do not
carry forward its isolated window station and desktop, disposable editor profile,
automation working copy, long-running preparation owner, lifetime lease, native
attestation, or replacement Windows-only graphical harnesses.

## Contract and defaults

Add the top-level `presentation` field to Director `editor_run` and the equivalent
`--presentation` option to `theatre run`, with the values `automated`, `deferred`,
and `interactive`.

Omission preserves the existing normal-editor behavior on every operating system.
It must not resolve differently by platform and must not silently become
`automated`. Explicit `interactive` selects that same normal visible behavior
while recording the caller's intent. This preserves existing Linux callers and
keeps the public contract predictable.

Start and restart accept presentation. Stop remains available regardless of the
run's presentation so cleanup cannot be blocked by a protection check. Status is
observational. A deferred run gains one explicit handoff operation that transitions
the current game to interactive presentation without restarting it. The exact
action name should follow the existing `editor_run` action vocabulary; `handoff`
is the working name. The initial callable route is Director `editor_run`; this
does not require redesigning `theatre run`, whose existing command shape starts or
restarts a named scene. Preparation alone never authorizes handoff: it follows a
later deliberate user instruction or explicit human action. Handoff rejects a
missing, stopped, non-deferred, or replaced run with an actionable result rather
than guessing which window to activate. A run is replaced when the originally
tracked game process or window has disappeared or changed; this does not imply a
new stale-client authorization contract.

On non-Windows systems, omitted and explicit interactive presentation follow the
existing code path. Explicit automated and deferred requests return unsupported
before dispatch because Theatre cannot claim protections it does not provide.
This new rejection must not alter ordinary launches, existing process management,
or any Linux graphical test harness.

## Windows implementation boundary

Reuse the verified editor connection and `EditorInterface` run lifecycle already
owned by Director. Do not launch or supervise a second editor, create another
project copy, or add a persistent preparation service.

A small Windows-only presentation helper may observe the foreground window,
identify the game window belonging to the selected editor run, apply reversible
no-activation and input-suppression behavior, and restore the prior foreground
when Godot activates itself. Prefer ordinary Win32 window operations over process
isolation. Scope all platform dependencies behind `cfg(windows)` and keep the
non-Windows launch path structurally unchanged.

Foreground restoration is conditional as well as bounded. Restore only while the
current foreground still belongs to the positively identified Theatre-launched
game or another launch-caused surface. If the user has already selected a different
application, leave that foreground unchanged. Do not add a background focus monitor.

The editor plugin owns only run-local presentation state needed for deferred
handoff, such as the current presentation and reversible game-window information.
Clear it when the game stops, restarts, exits, or the editor plugin shuts down.
Do not persist presentation state, create identity files, issue handoff tokens, or
add a second lifecycle owner. Stage remains independent: presentation protection
must not require observation, recording, a Stage connection, or Stage-provided
input filtering. Synthetic agent input through Stage should remain usable during
automated testing.

The helper makes a bounded attempt rather than proving isolation. If its Windows
capability is unavailable before launch, an explicit automated or deferred request
fails before mutation. If Godot starts but a later window operation fails, attempt
to restore the previous foreground, return an actionable degraded-protection
result, and retain ordinary stop recovery. Preserve the response's existing
`launch_requested` and `game_running` facts, and add only the smallest explicit
presentation outcome and reason needed to distinguish no launch from a launched
run with degraded protection. Do not manufacture native proof or silently describe
the run as protected. A degraded deferred run is not successfully prepared for
handoff.

Deferred handoff reverses only presentation controls that Theatre applied to the
current run, then deliberately activates that game window. It must not change
capture configuration, restart the scene, save editor work, or promote an unrelated
window discovered after the original run disappeared.

## Three delivery stages

### 1. Theatre mechanism

Implement the optional contract and lightweight Windows behavior from the clean
baseline. Preserve Director's existing saved-scene, unsaved-work, readiness, and
run-identity semantics. Restore no test or runtime behavior from the evidence
branch merely because it exists there.

### 2. Base agent policy

Add the four-workflow decision rule to the user-owned base agent instructions.
Agents explicitly request automated or deferred presentation on Windows; they do
not depend on an API default. Background tests choose headless only when headless
execution retains the evidence being sought. Interactive presentation requires an
expressly user-visible review request. If automated presentation is unavailable,
Windows agents use a meaningful headless path or report the limitation rather than
silently launching interactively. If a requested launch occurred with degraded
protection, the agent inspects the returned launch and running facts, stops that run
before attempting a fallback, and never reports a degraded deferred run as ready
for human handoff. Linux agents continue to follow existing project launch and test
conventions instead of being prohibited from established graphical workflows.

Repository-owned Stage and Director operating skills must agree with this policy
where they teach launch behavior. That synchronization prevents examples from
silently selecting a different presentation through omission.

### 3. Project agent policy

Update Theatre's project-rules template so a consuming project can add its actual
launch target, readiness transition, Quick Sandbox or equivalent review target,
and any local background-test convention beneath the four-workflow rule. Generic
generated guidance must not invent those project facts.

Update only project instruction files explicitly selected for this rollout. Do
not add a general rules-version registry, migrate every existing consumer, or
silently overwrite existing `AGENTS.md` or Claude rules. If an existing generated
section cannot be reconciled safely, report the exact manual update required.

## Verification

Use existing unit, CLI, Director editor, Stage live, and graphical harnesses. Do
not create a parallel presentation test framework.

1. Contract tests prove all three explicit values, omission preserving the legacy
   path, stop/status independence, handoff validation, and capture settings having
   no presentation side effects.
2. Linux builds and ordinary tests retain their existing behavior. Existing Unix
   graphical and wrapper paths remain present and runnable; no Windows capability
   guard may replace or skip them. Run available Linux engine journeys or report
   the exact missing environment evidence.
3. A bounded Windows editor journey exercises automated launch, deferred launch
   and handoff, interactive launch, stop recovery, preserved unsaved editor work,
   Stage viewport capture, and synthetic Stage input. Reuse the existing fixture
   and keep manual foreground/input observation clearly labeled as attended evidence
   rather than an automated guarantee. That observation includes ordinary typing
   and clicking during automated and deferred launch, successful human input after
   handoff, and a short legacy-behavior comparison.
4. Verify that automated/deferred behavior does not depend on Stage activation.
   Exercise known helper failure after preflight and confirm that the response is
   degraded, preserves launch/running facts, and leaves stop available. Exercise a
   user foreground change during launch and confirm that Theatre does not restore
   over the newly selected application.
5. Walk the base and project instructions through all four workflows. Confirm that
   they neither launch interactively without user intent nor choose headless when
   doing so would discard the visual behavior under test.
6. Run the complete repository checks from `.work/CONVENTIONS.md`, regenerate
   affected schemas and public references, check bundled client-skill parity, and
   reconcile affected architecture, contract, journey, and public launch guidance.

## Risks and recovery

The weakest assumption is that a small, reversible Windows window-control helper
can identify and modify the correct game window soon enough to improve ordinary
focus and input behavior without harming rendering. Validate that assumption first
against representative native and embedded game-window configurations. The first
implementation probe must also establish the concrete minimal boundary by which the
GDScript editor plugin invokes native Windows operations, where that helper is
delivered, and how run-local reversible state survives a one-shot Director launch
until a later Director handoff or editor shutdown. Demonstrate one one-shot launch
followed by a later handoff before broad contract or instruction implementation.
Do not prescribe a new GDExtension, service, or Stage dependency before that probe.
If a lightweight boundary cannot satisfy the lifecycle, stop and bring the evidence
back to design; do not reintroduce the isolated-desktop system as an implementation
detail.

Best-effort foreground restoration may be refused by Windows, and disabling a
window may not cover every physical device class. These are accepted initial
limitations when they are documented accurately. A failed or stale handoff must
leave the user with ordinary Director stop and process-level recovery; it must not
strand an editor or require deletion of persistent coordination state.

The change is recoverable by removing the optional presentation plumbing and
Windows helper: omitted calls already use the unchanged legacy path, and no stored
presentation data requires migration.

## Implementation and review approach

The main agent owns this design, reviewer adjudication, integration, and final
acceptance. Implementation will be delegated to bounded subagents after design
approval, with separate ownership for the Theatre mechanism and instruction
surfaces where parallel work is safe. Subagents may not broaden the outcome or
revive hardened machinery from the evidence branch without explicit approval.

The configured standard review requires one design pass and one integrated
implementation pass. The selected design reviewer is Astra at high reasoning.
Accepted design findings are applied here before implementation; material proposals
outside the lightweight boundary return to the user for disposition.

## Closure evidence

The feature closes only when the three delivery stages agree, the four workflows
are usable as described, omitted presentation preserves existing cross-platform
behavior, Windows evidence demonstrates the best-effort improvement without
claiming isolation, Linux launch and test behavior remains intact, the required
repository verification is complete, and the integrated implementation review has
been adjudicated.
