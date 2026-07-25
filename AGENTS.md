# Theatre — Agent Instructions

See `CLAUDE.md` for the full project instructions (repository layout, build
commands, architecture rules, code style, and git conventions).

<!-- workbench:start -->
## Workbench

Work is tracked in `.work/`: active items in `.work/active/`, deferred context
in `.work/backlog/`, project behavior in `.work/CONVENTIONS.md`, and—when release
summaries are enabled—temporary completion stubs in `.work/archive/` plus
summaries in `.work/releases/`. Grounded evidence lives in `.research/` and
interactive requirements walkthroughs live in `.mockups/`; work items reference
both. Confirm `owner: workbench` before operating. Optional project defaults for
interaction, rigor, review, capability, execution, and commits also live in
`CONVENTIONS.md`; explicit user direction overrides them for the current request
without changing the stored defaults.

Treat natural-language requests as the workflow. Gather consequential
requirements before confident execution: inspect the repository and research
facts first, then ask the user for choices. Use a structured question tool when
available; otherwise ask inline and pause. UI mockups are requirements evidence
and should converge on a working walkthrough that is browser/vision-inspected
before presentation when those tools exist.

Foundation documents contain only current or clearly intended future vision,
direction, architectural boundaries, high-level design, and durable contracts.
Code is the source of truth for implementation details. Reconcile affected
foundation assertions before completing work. Project-owned engineering values
live in `docs/PRINCIPLES.md`; observed recurring code structures live in
`.agents/skills/patterns/`.

Compatibility is earned, not assumed. Unless the project declares external
consumers (in `docs/PRINCIPLES.md` or `CONVENTIONS.md`), only two things
create compatibility obligations: dependencies outside the repository that are
not owned by the author, and substantial real data that must be preserved or
transformed. Never version project-owned schemas (v1/v2/v3) or keep
compatibility shims for surfaces the project owns—agent tooling such as MCP
servers, internal services, and unpublished libraries included; change them in
place. Real-data migrations are planned by the agent but approved and executed
by the user for production data; do not run production data transforms
autonomously.

Completed items never remain active. With summarized releases they immediately
become small archive stubs and later collapse into one release summary; with no
release lifecycle they are removed. Sweep stale terminal items whenever working
in the substrate. Prefer coherent feature-sized delivery commits over commits
for individual workflow transitions.
<!-- workbench:end -->
