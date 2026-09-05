---
id: theatre-client-feedback
kind: research-brief
summary: Claude and Codex hooks for next-boundary delivery of queued Godot feedback
updated: 2026-09-05
source_handles: [theatre-client-feedback-claude-hooks, theatre-client-feedback-claude-plugins, theatre-client-feedback-codex-hooks, theatre-client-feedback-codex-plugins]
relationships:
  - type: informs
    target: .work/active/agent-human-feedback.md
---

# Client hooks for human feedback

## Question and boundary

Can Claude Code and Codex plugins make queued Godot feedback visible to a working
agent? The requested flow is a small pending notice followed by retrieval of
selection, pointer context, image and any note. Automatic idle wake is excluded.

## Supported mechanisms

Claude Code documents additional context at tool boundaries, including
PostToolUse and PostToolBatch. Plugin hooks use the same mechanism as settings
hooks. [theatre-client-feedback-claude-hooks]{1}
[theatre-client-feedback-claude-hooks]{2}
[theatre-client-feedback-claude-hooks]{3}
[theatre-client-feedback-claude-hooks]{4}

Codex documents PostToolUse additionalContext after a completed tool result.
It supports hooks from plugins, with explicit trust requirements. These sources
support next-boundary context delivery, not interruption of arbitrary running
model work. [theatre-client-feedback-codex-hooks]{1}
[theatre-client-feedback-codex-hooks]{3}
[theatre-client-feedback-codex-plugins]{4}

Each client has its own native plugin manifest and hook packaging. Claude uses
.claude-plugin/plugin.json; Codex uses .codex-plugin/plugin.json. Both can package
hooks independently of bundled MCP configuration. [theatre-client-feedback-claude-plugins]{1}
[theatre-client-feedback-codex-plugins]{1}
[theatre-client-feedback-codex-plugins]{2}

**Recommendation (inference):** provide two small packages around one local queue
reader. A synchronous PostToolUse hook can return compact textual context without
connecting to Godot or consuming evidence. Keep images in the retrieval path,
not embedded as base64 text. Do not duplicate an existing project MCP setup.

The queue must remain usable without either hook integration. Claude plugin
activation can require a reload or restart. Codex plugin installation does not
implicitly trust its hooks. [theatre-client-feedback-claude-plugins]{4}
[theatre-client-feedback-codex-plugins]{4}

## Disconfirming evidence

- A completed background hook does not generally start an idle agent turn.
  Claude documents a separate asyncRewake exception; Codex waits for an active
  or subsequent user turn. Neither exception is required by this workflow.
  [theatre-client-feedback-claude-hooks]{5}
  [theatre-client-feedback-codex-hooks]{5}
- Stop hooks can force continuation, but that is a different control mechanism
  from presenting queued feedback. It is not selected here.
  [theatre-client-feedback-claude-hooks]{7}
  [theatre-client-feedback-codex-hooks]{7}
- Claude's Notification event discards context/control output. It is not an
  alternative context-injection route. [theatre-client-feedback-claude-hooks]{6}
- Codex bounds model-visible hook output. Large notes or image data should not
  be dumped into additionalContext. [theatre-client-feedback-codex-hooks]{4}
- Client versions, managed policy and host surface affect availability. Published
  documentation and installed binaries do not establish successful hook execution.
  [theatre-client-feedback-claude-hooks]{8}
  [theatre-client-feedback-claude-plugins]{4}
  [theatre-client-feedback-codex-hooks]{8}
  [theatre-client-feedback-codex-plugins]{5}

These are scope and delivery differences, not contradictory claims. No cited
contract establishes automatic image attachment through textual hook output.
Actual trusted-client tests remain implementation evidence to collect.

## Verification

Resolved rigor: standard. The lead checked the load-bearing claims against all
four fetched attestations, including asynchronous delivery and trust limits.
This source-support check was not independent. A separate design review inspected
the same evidence but is not counted as a client runtime test. Research lint and
knowledge-index checks validate the source chain, not actual host integration.
