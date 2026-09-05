---
source_handle: theatre-client-feedback-claude-hooks
fetched: 2026-09-04
source_title: Hooks reference - Claude Code Docs
source_url: https://docs.anthropic.com/en/docs/claude-code/hooks
---

Claude Code documents lifecycle hooks and their event-specific context, control,
and delivery behavior. This attestation records the documented mechanisms only;
it does not recommend a Theatre integration.

## Attested details

1. [Hook lifecycle and hook locations] Claude Code supports hooks from settings, plugins, skills, and subagents. Plugin hooks are loaded from `hooks/hooks.json` when the plugin is enabled. MCP tools appear as normal tool names for relevant tool events.
2. [Additional context] A hook can return `hookSpecificOutput.additionalContext`. Claude Code inserts it as a system reminder at the event boundary; `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, and `PostToolBatch` place it next to the tool result for the next model request.
3. [PostToolUse] A `PostToolUse` hook runs after a successful tool call and receives tool input and result. It can provide `additionalContext` or replace a result with `updatedToolOutput`; this does not undo the completed operation.
4. [PostToolBatch] `PostToolBatch` runs once after a parallel tool batch resolves and before the next model call. It supports `additionalContext` once for that batch.
5. [Async delivery] A command hook with `async: true` runs in the background. Its context is delivered only on the next conversation turn; if the session is idle it waits for user interaction. An `asyncRewake` command hook is the documented exception: on exit code 2 it wakes Claude and exposes a system reminder.
6. [Notification limits] The `Notification` event is for side effects. It cannot block or modify notifications, and its context/control fields are discarded.
7. [Stop behavior] A `Stop` hook can keep the agent working by returning `decision: "block"` or feedback context, subject to a loop guard. This is a turn-continuation mechanism, not an asynchronous event subscription.
8. [Version caveat] The document labels features with version prerequisites in several places; its listed `asyncRewake` behavior and the event/output schema should be checked against the installed Claude Code version before configuration.
