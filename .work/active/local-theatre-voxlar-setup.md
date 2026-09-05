---
id: local-theatre-voxlar-setup
kind: story
status: active
tags: [developer-environment]
parent: null
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-05
updated: 2026-09-05
---

# Install verified Theatre and equip Voxlar

## Outcome

Update the local Theatre binaries and prepare `../voxlar` for practical
agent-assisted development. Nathan explicitly prioritized this rollout and
parked the intermittent native accessibility investigation rather than waiting
for that investigation or the remaining broad native qualification.

## Boundary

Inspect Voxlar's actual layout, existing instructions, project configuration and
uncommitted changes before editing. Reconcile AGENTS.md and useful supporting
instructions, skills, Theatre addon/MCP wiring and optional client integration as
appropriate. Preserve existing guidance and user work. Do not infer authorization
to adopt Workbench, change gameplay/assets, or silently enable global client
hooks. Respect client trust and any required ignore-file consent.

## Verification

Verify installed executable versions/paths and matched addon payloads. Check the
actual Voxlar setup through its relevant configuration and native Godot interfaces
where applicable. Report required client/editor restarts and unavailable evidence.
Avoid overwriting loaded extension files in place or saving unrelated human edits.

Nathan also authorized committing and pushing the finished work to GitHub.
Inspect each repository's remotes, branch state and contribution rules. Commit
and push the authorized Theatre and Voxlar changes without force-pushing or
including unrelated work. This does not authorize version tags or release
publication. Commits use Nathan's identity without agent attribution.

## Local installation evidence

Release builds of Stage, Director, Theatre and the Stage extension passed.
Installed 70 files into the user-local binary/share directories, including
shared Godot support and both optional client packages. Each copied file was
hash-checked against staging; executable/library replacements used atomic rename
so existing processes need not be killed. The CLI version remains the workspace
version 0.3.4; new feedback help and Stage runtime-status/diagnostics/viewport/
feedback commands are present. The installed Godot remains 4.7.1. Owned release
staging and the unused isolated 4.7.2 download were removed. Voxlar configuration
and selected-project smoke verification are continuing.
