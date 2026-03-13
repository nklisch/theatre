# Headless Daemon Backend

The headless daemon is Director's fallback when the Godot editor is not open. It runs a Godot process in headless mode — no window, no GUI — and processes Director operations over TCP.

## How it works

The daemon is a Godot script (`addons/director/daemon.gd`) that runs under `godot --headless`. It:

1. Loads your project (runs `_ready` on the main scene, or runs without a main scene if none is configured)
2. Starts a TCP listener on **port 6551** (localhost only)
3. Receives Director operations from the `director` binary
4. Executes them using Godot's API (same as the editor plugin)
5. Returns results and keeps running for the next operation

The key advantage over one-shot mode: the Godot process **stays running** between operations. Subsequent operations do not pay the startup cost (~500-2000ms per operation in one-shot mode).

## Starting the daemon

```bash
# Start the daemon for your project
godot --headless --script addons/director/daemon.gd --path /home/user/my-game

# Or use the provided shell wrapper
./addons/director/start-daemon.sh /home/user/my-game
```

The daemon prints to stderr when ready:

```
[Director Daemon] Loaded project: /home/user/my-game
[Director Daemon] Listening on port 6551
```

Keep the daemon running in a terminal while you work. The `director` binary will automatically use it when the editor backend is unavailable.

## Stopping the daemon

```bash
# Send SIGTERM (Ctrl+C in the terminal)
# Or:
pkill -f "director/daemon.gd"
```

When stopped, the daemon closes its TCP listener cleanly.

## One-shot fallback

If neither port 6550 (editor) nor port 6551 (daemon) is reachable, the `director` binary falls back to **one-shot mode**:

1. Spawn `godot --headless --script addons/director/oneshot.gd -- <serialized_operation>`
2. Godot loads, executes the operation, exits
3. The binary reads the output and returns the result

One-shot is always available — it requires only `godot` on the PATH. But it is slow: each operation costs the full Godot startup time (500-2000ms). For batches of operations, use the daemon.

## When each backend is used

| Scenario | Backend selected |
|---|---|
| Editor open, project matches | Editor (port 6550) |
| Editor open, wrong project | Daemon (port 6551) |
| Editor closed, daemon running | Daemon (port 6551) |
| Editor closed, no daemon | One-shot |
| CI/CD pipeline | Daemon (started by CI) or One-shot |

The `director` binary tries backends in this order: port 6550 → port 6551 → one-shot. Connection attempts time out in 200ms, so the fallback chain is fast.

## Using the daemon in CI/CD

In a GitHub Actions workflow:

```yaml
- name: Start Director daemon
  run: |
    godot --headless --script addons/director/daemon.gd --path ${{ github.workspace }}/my-game &
    # Wait for daemon to be ready
    sleep 3

- name: Run scene generation
  run: |
    ./target/release/director batch-run scene-generation-script.json
```

Alternatively, use one-shot mode in CI (simpler, no background process management):

```yaml
- name: Generate level
  run: |
    # director will use one-shot automatically since no daemon is running
    echo '{"op": "batch_execute", ...}' | ./target/release/director run-stdin
```

## Port configuration

The default daemon port is 6551. Change it with:

```bash
godot --headless --script addons/director/daemon.gd -- --port 7551
```

Or in project settings: **Theatre → Director → Daemon Port**.

## Performance comparison

| Mode | First operation | Subsequent operations |
|---|---|---|
| Editor backend | 10-50ms | 10-50ms |
| Daemon backend | 50-200ms | 50-200ms |
| One-shot | 500-2000ms | 500-2000ms each |

For any batch of more than 1 operation, the daemon provides a significant speedup over one-shot. Start the daemon when doing a session of Director work.
