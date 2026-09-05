---
id: theatre-marketplace-registration
kind: story
tags: [distribution, integration]
parent: null
release: null
completed: 2026-09-05
---

# Advertise Theatre in the shared skills marketplace

Registered theatre-feedback in both nklisch-skills native catalogs using external
Git-subdirectory sources in nklisch/theatre. Claude and Codex retain their existing
self-contained package roots, shared identity and canonical skills/hooks; no
marketplace-owned copy or third package was added. Marketplace guidance explains
separate CLI/addon installation, optional hook trust and the older prebuilt
0.3.4 helper limitation. Source metadata explicitly declares Codex hooks and
marketplace presentation, and the installed local manifest was refreshed.

One standard Astra review accepted the registration without findings. JSON,
ordered identities, package parity, references, substrate and whitespace checks
passed. Isolated native Claude and Codex installations passed. After publishing
the updated Theatre metadata, a fresh Codex Git-subdirectory install through the
new nklisch-skills entry verified current metadata, both skills, references and
the hook. Temporary configuration was removed and the global configuration was
unchanged. No authentication exercise, global activation, engine investigation,
version tag or release publication was performed.
