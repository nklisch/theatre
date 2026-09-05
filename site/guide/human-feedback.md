---
description: "Share deliberate Godot editor or game evidence with an agent through Theatre's project-local feedback queue."
---

# Human Feedback

Theatre lets a developer share a hard-to-describe observation from the running
game or the Godot editor. A feedback item can contain a viewport image, selection
or pointer context, scene and process identity, and an optional note.

Feedback is deliberate. Theatre does not watch ordinary clicks, pause gameplay,
or infer a runtime node from the pointer. The developer reviews the composer and
queues the item explicitly.

## Share from the running game

Use the Stage runtime **Share feedback** control or its displayed shortcut. The
runtime copies the latest completed root-viewport render and pointer context
before opening the composer. Add a note, review the preview, and queue it.

This path is independent of dashcam markers and recording. It still queues the
scene, pointer, run context, and note when rendered pixels are unavailable.

## Share from the editor

Use **Share feedback** in the Director editor integration. Theatre captures the
active 2D or 3D scene viewport and copies the current selection. It does not
change selection, mark the scene dirty, or save files.

With split 3D views and no pointer over one view, Theatre does not guess which
viewport the developer meant. The composer can still carry selection and a note.

## Retrieve and handle evidence

Stage and Director expose the same `feedback` operation family. The Theatre CLI
also reads the queue without a running engine:

```bash
theatre feedback --project /path/to/game '{"action":"status"}'
theatre feedback --project /path/to/game \
  '{"action":"retrieve","feedback_id":"feedback_..."}'
theatre feedback --project /path/to/game \
  '{"action":"handle","feedback_id":"feedback_..."}'
```

Use this sequence:

1. Read status and choose the matching `feedback_id`.
2. Retrieve the item and optional image.
3. Address the developer's observation.
4. Handle the item to suppress later pending notices.
5. Delete it only when retained evidence is no longer useful.

Status and retrieval do not consume evidence. Handling is shared across all
readers and keeps retrieval available. Deletion is a separate explicit action.
Incomplete publication directories appear in status and require deliberate
cleanup.

## Pending notices and optional client hooks

Normal Stage and Director results can include a compact pending-feedback notice.
The notice does not change the original result or error meaning.

The installed distribution also contains separate optional Claude and Codex
plugin packages. After explicit installation, activation, and project trust,
their synchronous post-tool hook can inject the same text notice at a later tool
boundary. The hook does not handle feedback, embed the JPEG as text, wake an idle
agent, or steer a running turn asynchronously. Explicit feedback retrieval remains
the image path.

## Storage and privacy

Items remain under `.theatre/feedback` in the selected project after Godot exits.
The project setup adds an ignore rule to prevent accidental commits. This does
not encrypt the evidence or restrict local filesystem access.

The queue bounds each note and image plus total retained storage. When full, it
preserves existing items and the current unsent composition. Delete old evidence
or clean up a confirmed incomplete publication before retrying.
