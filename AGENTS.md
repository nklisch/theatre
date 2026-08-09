# Theatre — Agent Instructions

See `CLAUDE.md` for the full project instructions (repository layout, build
commands, architecture rules, code style, and git conventions).

## Repository paths

Use repository-relative paths for files that belong to this Git repository and
are not ignored by Git. Do not record local machine paths for these files.

If a file should remain local-only, notify the user that it needs a
`.gitignore` entry. Do not change `.gitignore` until the user confirms the
change.

<!-- workbench:start -->
## Workbench

Confirm `owner: workbench` in `.work/CONVENTIONS.md`. Track active outcomes in
`.work/active/` and deferred context in `.work/backlog/`. Treat natural-language
requests as the workflow. Consult `.knowledge/index.json` when present. Ask the
human about consequential requirements and pause for the answer. Park useful
out-of-scope findings instead of silently expanding scope. Test behavior at
stable interfaces, verify the full requested boundary, reconcile affected
foundation truth, and remove or summarize completed items immediately.
<!-- workbench:end -->
