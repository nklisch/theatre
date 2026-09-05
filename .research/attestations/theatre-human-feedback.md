---
source_handle: theatre-human-feedback
fetched: 2026-09-04
source_title: Theatre human marker and event paths at commit 2486f021bad0c81efab5a04f982614b5bc81938e
source_url: https://github.com/nklisch/theatre/tree/2486f021bad0c81efab5a04f982614b5bc81938e
---

Read the committed runtime, debugger bridge, and server event-handler sources
from the observed Theatre HEAD during this engagement. Details describe that
revision, not uncommitted implementation of the development-loop epic.

## Attested details

1. The runtime creates an in-game flag button connected to `_drop_marker`.
   [runtime.gd, lines 166–176](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/addons/stage/runtime.gd#L166-L176).
2. The shortcut handler recognizes configured marker and pause keys and consumes
   only those handled key events. `_drop_marker` calls `flush_dashcam_clip("human")`
   when recording is active; it passes no cursor or clicked-node metadata.
   [runtime.gd, lines 191–217](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/addons/stage/runtime.gd#L191-L217).
3. Runtime status is sent to the Godot editor with `EngineDebugger.send_message`.
   The debugger command handler accepts `stage:command` with `add_marker`.
   [runtime.gd, lines 87–105](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/addons/stage/runtime.gd#L87-L105).
4. The Stage server's addon-event branch buffers `signal_emitted` data for deltas;
   its other-event branch writes a debug log. This inspected branch does not
   forward a human marker as an agent notification.
   [tcp.rs, lines 309–329](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/tcp.rs#L309-L329).
