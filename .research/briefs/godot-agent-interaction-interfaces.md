---
id: godot-agent-interaction-interfaces
kind: research-brief
summary: Current Godot interface choices, deliberate human feedback, and the existing temporal-vision integration
updated: 2026-09-04
source_handles: [godot-latest-releases, godot-latest-docs-status, godot-latest-editor-interface, godot-latest-gdext-compatibility, godot-latest-lazy-function-tables, godot-latest-unsaved-scenes-version, godot-latest-gdext-4-7, godot-latest-logger, godot-latest-logger-debugger, godot-interaction-input, godot-interaction-viewport, godot-interaction-debugger, temporal-current-theatre, temporal-current-krometrail, temporal-current-registry, theatre-human-feedback, godot-local-api-probe]
relationships:
  - type: informs
    target: .work/active/agent-godot-development-loop.md
---

# Godot interfaces and deliberate human feedback

## Decision boundary

Which current Godot interfaces can improve Theatre's editor/runtime loop, and
how could a developer point at a moment for an agent to inspect? Also verify
whether Theatre already uses the Rust visual-analysis crate from Krometrail.
The user subsequently approved Godot 4.7 as the minimum. The verified binding
migration below supports that decision. No preview engine or new human-interaction
UI is authorized by this research.

## Current engine and binding choices

The official archive lists Godot 4.7.2 as stable and 4.8-dev4 as a preview.
The installed executable reports 4.7.1. It is one maintenance release behind
that stable listing. [godot-latest-releases]{1} [godot-latest-releases]{2}
[godot-latest-releases]{3} [godot-local-api-probe]{1}

The stable EditorInterface reference exposes open-scene roots, the active root,
unsaved-scene reporting, native undo access, save operations and play controls.
The installed engine also reports a subset of these methods, including open-scene
roots, unsaved-scene reporting, undo access, save and custom-scene play controls. [godot-latest-editor-interface]{1}
[godot-latest-editor-interface]{2} [godot-latest-editor-interface]{3}
[godot-latest-editor-interface]{4} [godot-latest-editor-interface]{5}
[godot-local-api-probe]{2}

A compiled extension API target and an engine runtime version are different
choices. Godot-rust documents running extensions on an engine version at or above
their API target, within its stated compatibility scope. Updating the engine does
not by itself require raising the extension API floor. [godot-latest-gdext-compatibility]{1}
[godot-latest-gdext-compatibility]{2}

**Recommendation (inference, following the approved 4.7 floor):** use the published
`godot` 0.5.5 release with `api-4-7`, retaining `experimental-godot-api` and
`lazy-function-tables`. The 4.7 API feature first appears in 0.5.4; 0.5.5 retains it.
This requires Rust 1.94 and the documented 0.4-to-0.5 source migration. Use stable,
version-qualified APIs and verify actual behavior rather than switching to preview
Godot. [godot-latest-gdext-4-7]{1} [godot-latest-gdext-4-7]{2}
[godot-latest-gdext-4-7]{4} [godot-latest-gdext-4-7]{7}
[godot-latest-gdext-4-7]{8} [godot-latest-docs-status]{1}

One concrete version difference is `get_unsaved_scenes`: the fetched 4.5 and
4.6 stable declarations lack it, while 4.7 declares it. This can improve native
status reporting directly under the now-approved minimum, without simulating
native unsaved-scene state. [godot-latest-unsaved-scenes-version]{1}
[godot-latest-unsaved-scenes-version]{2} [godot-latest-unsaved-scenes-version]{3}

Lazy function tables defer function lookup and possible failure until use. They
do not prove a newer method exists on an older engine. [godot-latest-lazy-function-tables]{1}
[godot-latest-lazy-function-tables]{2}

One useful observed lifecycle result: a temporary native editor probe disabled
autosave only around the synchronous play call. It started the saved scene while
preserving an unsaved editor change. That supports native play control without
adding another process owner, but does not establish behavior for every release
or third-party build hook. [godot-local-api-probe]{4}

## Current-run diagnostic source

Godot's native `Logger` receives structured errors, warnings, script errors and
shader errors with origin fields and optional script backtraces. This is distinct
from ordinary printed output. [godot-latest-logger]{1} [godot-latest-logger]{2}
[godot-latest-logger]{3}

**Recommendation (inference):** use a small runtime-owned, bounded collector for
current-run diagnostics, associated with Theatre's engine-owned run identity.
Callbacks may arrive on multiple threads, so synchronize the retained data and
avoid scene-tree work or recursive logging inside callbacks. Do not collect
variables or promise pre-registration history. Log settings can suppress events,
and release builds may omit backtraces. [godot-latest-logger]{4}
[godot-latest-logger]{5} [godot-latest-logger]{6} [godot-latest-logger]{8}

The documented editor debugger session provides lifecycle and breakpoint state,
not a generic structured-error signal. Its namespaced message path would still
need a runtime producer. **Inference:** a second debugger framework is not
justified by this public API comparison. [godot-latest-logger-debugger]{1}
[godot-latest-logger-debugger]{2} [godot-latest-logger-debugger]{3}

## Pointing at a moment

Theatre already has a configurable marker shortcut and an in-game flag button.
They save a human-triggered dashcam clip while recording is active. That path
currently carries neither cursor coordinates nor clicked-node context.
[theatre-human-feedback]{1} [theatre-human-feedback]{2}

**Minimum useful extension (inference):** a deliberate share-moment shortcut
could associate a marker with viewport-local cursor coordinates and a captured
image or nearby recorded frames. Reuse the marker path rather than recording
every key and click. Viewport-local coordinates and Godot's transforms provide
a basis for matching the pointer to the image. [godot-interaction-viewport]{1}
[theatre-human-feedback]{2}

**Optional richer interaction (inference):** an explicit point-at-object mode
could attach a selected node's state. It needs a clear rule for whether the click
also reaches gameplay, and separate resolution for UI, 2D and 3D content. A
physics pick is not automatically the visual object a person meant. Built-in
2D picking has ordering and object-count limits. [godot-interaction-input]{2}
[godot-interaction-viewport]{5}

Capturing evidence and notifying an idle agent are separate boundaries. The
inspected server event path buffers signal events and logs other addon events;
it does not forward a human marker as an agent notification. **Inference:** a
portable pull interface could let an active agent retrieve pending feedback,
while automatic wake-up needs an explicitly verified client/harness path. The
existing Godot debugger bridge connects editor and game, not automatically the
agent conversation. [theatre-human-feedback]{3} [theatre-human-feedback]{4}
[godot-interaction-debugger]{1} [godot-interaction-debugger]{2}

The smallest unresolved product question is whether the developer wants to
share a moment from the running game, a selection in the editor, or both—and
whether that action should wake an idle agent or wait for its next tool call.
Do not treat those choices as an accepted UI design.

## Krometrail integration is already present

Theatre declares the published `temporal-vision` dependency and resolves version
0.1.1 from crates.io. Registry metadata identifies Krometrail as its repository,
and the registry checksum matches Theatre's lockfile. The sibling repository
also contains the crate. [temporal-current-theatre]{1} [temporal-current-theatre]{2}
[temporal-current-registry]{1} [temporal-current-registry]{2}
[temporal-current-krometrail]{1} [temporal-current-krometrail]{2}

The Stage server already uses it for saved-clip storyboards, motion histories,
difference maps and node-following filmstrips. Inputs come from retained clip
screenshots, markers and gaps. Existing engine journeys and fixture tests cover
parts of that integration; this research did not rerun those tests.
[temporal-current-theatre]{3} [temporal-current-theatre]{4}
[temporal-current-theatre]{5} [temporal-current-theatre]{8}
[temporal-current-theatre]{9}

**Implication (inference):** no second integration of the same crate is needed.
The remaining human-feedback problem is capture association and delivery, not
adding another storyboard generator. The current clip tool does not expose a
human-click stream or live visual-artifact capture. [temporal-current-theatre]{6}

## Disconfirming evidence

- `latest` Godot documentation explicitly describes an unstable development
  version. The viewport/debugger references are useful API evidence, not a
  version-unqualified promise of stable or minimum-version support.
  [godot-latest-docs-status]{1} [godot-interaction-debugger]{4}
- Viewport textures can be black or outdated when read too early. Godot advises
  waiting for `frame_post_draw`; an input event and screenshot must not be
  described as an atomic simulation snapshot. [godot-interaction-viewport]{2}
  [godot-interaction-viewport]{3}
- Headless rendering cannot provide ordinary viewport evidence. A share-moment
  action needs an explicit no-image result rather than losing its spatial
  context or pretending the screen was unchanged. [godot-interaction-viewport]{6}
- Input observation and input injection are different capabilities. `push_input`
  executes the propagation/picking path; it is not needed merely to share what
  a human saw. [godot-interaction-viewport]{4}
- A newer API surface and a binding upgrade do not establish correct save/undo
  behavior. The installed native probe is narrower evidence than that claim.
  [godot-local-api-probe]{4} [godot-latest-gdext-compatibility]{6}

The main tensions are version scope and evidence timing, not contradictory
requirements. Recommendations remain conditional on the desired human workflow.

## Verification

Resolved rigor: standard. An independent Astra source-support pass reviewed all
load-bearing conclusions against the cited attestations. One minor qualifier was
corrected: the installed-engine probe establishes a subset of documented methods,
not every method in the reference. The lead rechecked that corrected sentence
against the enumerated probe result. No material unsupported conclusion or
contradiction remained. Research lint and knowledge-index build/check passed.
