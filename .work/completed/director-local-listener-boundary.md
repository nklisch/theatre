---
id: director-local-listener-boundary
kind: story
tags: [director, security, networking]
parent: null
release: null
completed: 2026-09-05
---

# Establish Director's intended local listener boundary

Director editor, daemon and mock listeners explicitly bind IPv4 loopback, matching their local-development clients. Real-engine coverage verifies local editor/daemon operations and rejects connections through an alternate loopback address. The change does not add local-client authentication.
