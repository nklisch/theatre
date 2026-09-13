---
id: lightweight-godot-presentation
kind: feature
status: completed
completed: 2026-09-12
---

# Lightweight Godot launch presentation

Windows agent-launched editor runs now accept explicit `automated`, `deferred`,
and `interactive` presentation intent. Automated runs use bounded descendant
window discovery, ordinary physical-input suppression, and conditional foreground
restoration; deferred runs preserve launch focus without application handoff state;
interactive and omitted requests retain the ordinary editor path. Automated and
deferred reject before dispatch on other platforms, leaving Linux launch behavior
unchanged.

Director, the Theatre CLI, base and project agent guidance, bundled skills,
generated schemas, and public foundations agree on the four Windows workflows.
Godot 4.7.1 verified the default embedded path with Stage viewport capture and
synthetic input, all three explicit presentations, omission, stop recovery,
unsaved-work preservation, and operation without Stage. The integrated review
removed an ineffective window call and found no remaining scope-expanding
isolation or handoff machinery.

Native-window and Linux engine runs were not repeated after the project adopted
focused, risk-based verification. Their code paths remain structurally separate;
attended focus and input behavior will be observed during local-project rollout.
