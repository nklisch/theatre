---
id: diagnose-client-addon-handshake-mismatch
tags: [director, stage, diagnostics, deployment]
created: 2026-09-05
updated: 2026-09-05
---

# Explain stale-addon handshake failures and their repair

During Voxlar windowed qualification, installed Theatre clients could not use
an existing project's older local addon payloads. The reported errors resembled
missing tool arguments rather than a client/addon protocol mismatch.

## Observed behavior

- Director `editor_run` received a valid absolute `project_path`,
  `action: "start"`, and `scene_path: "res://main.tscn"`, but returned:
  `editor project identity could not be verified: missing field project_path`.
  Its response also repeated the expected tool parameters, despite the supplied
  project path. The editor was open on that project.
- Stage `runtime_status {}` returned `connected: false`, `ready: false`, with
  `Failed to read handshake: deserialization error: missing field identity`.
  The running addon logged that it sent its handshake.
- Installed CLI reported Theatre `0.3.4`; Godot was
  `4.7.1.stable.official.a13da4feb` on Linux. The old addon version was not
  recorded before refresh, so the exact mismatched version pair is unknown.

Refreshing the project's generated addons with
`theatre init --yes --godot-bin <approved-godot> <project>` and restarting the
runtime restored Stage. `runtime_status` then reported the correct project,
process, scene, and `ready: true`. Director was not retested after refresh.
This supports an outdated-payload diagnosis; it does not demonstrate failure
with matching current client/addon builds.

The useful follow-up is an actionable mismatch diagnosis and repair guidance,
not an assumed requirement for legacy protocol support or bypassing project
identity checks.

## Evidence

Consumer: Voxlar, worktree `/storage/voxlar-mixed-runtime`, commit `70b288c`;
project `examples/user-player-sandbox-starter`.

Local evidence under
`/storage/voxlar-smooth-reconstruction/godot-integration/`:
`windowed-editor.log`, `theatre-refresh.log`, and `windowed-runtime.log`.
The exact client errors above were returned during the September 5 session;
retain them here because the client responses are not all in the runtime logs.
