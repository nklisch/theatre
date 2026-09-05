---
id: cli-install-deploy-journey-coverage
kind: feature
tags: [cli, tests, investigation]
parent: null
release: null
completed: 2026-09-05
---

# Check the remaining install/deploy journey evidence

Install, init and deploy share staged native-payload replacement rather than truncating mapped binaries. Tests cover replacing a running executable on Linux, preserving destinations after copy failure and cleaning temporary payloads. CLI test sessions are isolated from consumer listeners; telemetry opt-out tests no longer race on shared process environment. Actual deployment passes without stopping installed consumers.
