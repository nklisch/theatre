---
id: theatre-client-operating-skills
kind: feature
tags: [integration, guidance]
parent: null
release: null
completed: 2026-09-05
---

# Bundle Theatre operating skills and refresh consumer guidance

Both native client packages now include self-contained theatre-stage and
theatre-director skills with their references. Canonical guidance remains in
`.agents/skills/`; `scripts/sync-client-skills.py` synchronizes physical package
copies, and CI/release checks reject drift. Plugin identities are unchanged and
versions follow the existing release process. No additional MCP registrations
or global hook/trust changes were introduced.

Updated canonical and Voxlar skills explain sandbox installation, generator
ownership, Stage startup selection and restart requirements, Director's per-call
project path, CLI overrides, shared ports and project-skill fallbacks. The hook
now honors an explicit THEATRE_PROJECT_DIR for nested projects; invalid explicit
selections do not fall back to another queue. Unset selection retains ancestor
lookup. Client-hook environment and server-only MCP environment are distinguished.

Verification passed: 39 CLI unit tests, 41 CLI integration tests, two feedback
integration tests, scoped clippy, release CLI build, formatting, canonical/copy
parity, references and site build. Claude strict native package validation passed;
authenticated Claude discovery was not claimed. An isolated Codex installation
and native app-server skills/list discovered theatre-feedback:theatre-stage and
theatre-feedback:theatre-director. The initial probe expected unqualified names;
its corrected namespaced assertion passed. Temporary native configuration and
processes were removed, with the global configuration unchanged.

One standard Astra integrated review accepted the implementation without findings.
The local CLI and client packages were refreshed using 12 atomic file replacements;
installed skill parity and reference resolution passed. Voxlar's other domain and
Godot engineering skills were preserved. The accessibility investigation remains
parked; this feature did not run or reopen engine qualification.
