---
id: director-local-listener-boundary
tags: [director, security, networking]
created: 2026-09-04
updated: 2026-09-04
---

# Establish Director's intended local listener boundary

Preserved during Workbench setup's source-versus-foundation reconciliation.
Theatre is described as a local development tool, but
`addons/director/plugin.gd` and `addons/director/daemon.gd` call
`TCPServer.listen(_port)` without an explicit bind address. Director's Rust
clients connecting to `127.0.0.1` does not constrain the server's bind address.
The operation protocol has no caller authentication and permits project writes.

Investigate actual interface exposure and the intended local-only boundary.
The concrete threat is an untrusted network peer reaching authoring operations
that can modify a developer's project. A narrow loopback bind may address that
threat without adding a server-style authentication or permission framework.
Do not claim external reachability was reproduced: the setup pass inspected
source and corrected the foundation claim, but did not perform a network test.

This is deferred implementation work, not authorization to change listeners
while reconciling documentation. Related but independent:
[project/session identity](../active/godot-project-session-identity.md).
