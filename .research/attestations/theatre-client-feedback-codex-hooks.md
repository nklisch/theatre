---
source_handle: theatre-client-feedback-codex-hooks
fetched: 2026-09-04
source_title: Hooks - ChatGPT Learn
source_url: https://developers.openai.com/codex/hooks
---

Codex documents hooks as scripts or MCP tool calls at agent lifecycle events,
with explicit trust and output-delivery behavior.

## Attested details

1. [Hook sources and trust] Codex loads `hooks.json` or inline `[hooks]` from active configuration layers and can load hooks from enabled plugins. Project-local hooks require project trust; non-managed hooks require review/trust by their current definition hash.
2. [MCP hooks] Codex supports command and `mcp_tool` hook handlers. MCP hooks use an already-connected server, do not start or reconnect it, run synchronously, and do not themselves trigger other hooks.
3. [Tool-bound context] `PreToolUse` and `PostToolUse` cover MCP tools and other listed local tools. `PostToolUse` accepts `additionalContext`, and the documented result is model-visible feedback after the completed tool result; it cannot undo the side effect.
4. [Output constraints] The documentation says Codex limits each model-visible hook output to roughly 2,500 tokens by default, spilling larger `additionalContext` to disk with a preview. It advises concise hook/plugin context.
5. [Async delivery] An `async: true` command hook runs in the background. If a turn is active, its informational output is made available at the next model request in that turn after current work completes. If no turn is active, delivery waits until the next user turn; completion does not start a new turn.
6. [Async limits] Background hooks cannot block, approve, rewrite, or otherwise control the triggering operation. Codex may run up to eight background hooks per session, cancels unfinished hooks at session end, and discards undelivered output.
7. [Stop steering] A synchronous `Stop` hook can return `decision: "block"` to continue the agent with a generated continuation prompt. This is a hook-time control point, not a queue event waking an idle agent.
8. [Release caveat] The source explicitly says linked `main`-branch schemas can contain fields absent from the current release and directs readers to the page as release behavior; use the installed client’s `/hooks` view when configuring it.
