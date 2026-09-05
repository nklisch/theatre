---
id: cli-install-deploy-journey-coverage
tags: [cli, tests, investigation]
created: 2026-09-04
updated: 2026-09-04
---

# Check the remaining install/deploy journey evidence

Preserved while retiring the historical
[Theatre CLI test plan](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/docs/design/theatre-cli-e2e-tests.md).
Most of its proposed setup, rules, configuration, and error-path tests are
represented in `crates/theatre-cli/tests/cli_integration.rs`; do not treat the
old matrix or its priorities as current requirements.

The source plan left build-dependent installation and deployment testing as an
environment constraint. Before adding tests, compare existing source-install,
platform, release, and deployment journeys with the real user boundary: built
binaries and addon templates reaching a usable Godot project without affecting
unrelated files. Preserve useful isolated-environment fixture practice rather
than turning every old proposed case into a new test.

This is an evidence question, not a confirmed missing feature or an accepted
expansion of the test suite. Existing completion records include successful
platform-specific installation/deployment runs; consult those and current code
before deciding whether any additional maintained test earns its cost.
