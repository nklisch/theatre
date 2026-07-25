---
owner: workbench
schema: 1
completed_items: summarize
---

# Workbench Conventions

## Project verification

```bash
cargo build --workspace
cargo clippy --workspace
cargo fmt --check
# Full test run requires the GDExtension deployed to the test project first:
theatre deploy ~/dev/theatre/tests/godot-project
cargo test --workspace
```

All test layers must pass — unit, integration, scenarios, and E2E journeys.
Never skip E2E journey tests. See CLAUDE.md for details.

## Project-specific guidance

Engineering conventions, architecture rules, and build/deploy workflows live in
`CLAUDE.md`; engineering principles live in `docs/PRINCIPLES.md`. Foundation
documents in `docs/` follow the trust levels documented in `CLAUDE.md`
(code is ground truth; `docs/design/completed/` is historical design intent).
