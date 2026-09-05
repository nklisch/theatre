---
id: diagnose-client-addon-handshake-mismatch
kind: story
tags: [director, stage, diagnostics, deployment]
parent: null
release: null
completed: 2026-09-05
---

# Explain stale-addon handshake failures and their repair

Stage and Director identify malformed or missing engine identity as a response-origin compatibility problem, preserving the concrete error and directing users to deploy/restart the addon rather than change tool arguments. Existing project/run identity checks and no-replay behavior remain intact.
