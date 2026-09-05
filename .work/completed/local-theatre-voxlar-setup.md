---
id: local-theatre-voxlar-setup
kind: story
tags: [developer-environment]
parent: null
release: null
completed: 2026-09-05
---

# Install Theatre and equip the consuming project

Built release Stage, Director, Theatre and the Stage extension. Installed and
hash-verified 70 user-local binary/template/support/client-package files, using
atomic replacements for loaded executables and libraries. The workspace version
remains 0.3.4; the new CLI surfaces were checked. Godot remains 4.7.1. Owned staging
and the isolated patch download were removed; the shared Cargo target was reused.

Configured the requested Voxlar repository with preserved existing guidance,
portable operating skills, explicit nested-project Stage selection and local addon
payloads. Director still requires an explicit project path. Native import,
generator idempotence, addon payload comparison, Director scene reading and Stage
identity/readiness/snapshot smoke checks passed. The smoke process was stopped.
The shared support ignore entry was explicitly approved. No gameplay changes,
global hook/trust changes or Workbench migration were made.

Theatre checkpoint e442e3d and Voxlar setup 7a75cf2 were pushed to their existing
GitHub main branches. Restart the agent from the Voxlar repository root to load
the updated project configuration and skills. Optional client hooks remain opt-in.

Nathan explicitly deferred the intermittent native accessibility investigation;
see `.work/backlog/godot-editor-accesskit-undo-crash.md`. This rollout does not
claim the broader epic's remaining native/combined-journey verification passed.
Claude host-hook execution remains unverified under the accepted shipping caveat;
the native Codex journey passed. Voxlar's older Workbench stamp was left alone.
