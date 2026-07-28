#!/usr/bin/env bash
# Set up a Godot dogfood project wired to theatre, then optionally start it.
#
# Usage:
#   ./scripts/dogfood-setup.sh [project-dir]            setup only (default: ~/dev/voxel-dogfood)
#   ./scripts/dogfood-setup.sh [project-dir] --editor   setup, then open the Godot editor
#   ./scripts/dogfood-setup.sh [project-dir] --run      setup, then run the game
#
# The script only wires the project to theatre. The game itself is generated
# from the prompt written to <project-dir>/PROMPT.md — hand that to your
# agent inside the project directory, then re-run with --editor or --run.
#
# Godot binary resolution: $GODOT_BIN, else `godot` on PATH, else ~/godot/Godot_*.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Parse args: first non-flag arg is the project dir; --editor/--run select a mode.
PROJECT_ARG=""
MODE=""
for arg in "$@"; do
	case "$arg" in
		--editor|--run)
			MODE="$arg"
			;;
		--*)
			echo "Unknown flag: $arg (use --editor or --run)" >&2
			exit 1
			;;
		*)
			if [ -z "$PROJECT_ARG" ]; then
				PROJECT_ARG="$arg"
			else
				echo "Unexpected extra argument: $arg" >&2
				exit 1
			fi
			;;
	esac
done
PROJECT_DIR="$(realpath -m "${PROJECT_ARG:-$HOME/dev/voxel-dogfood}")"

# --- Resolve Godot binary -----------------------------------------------------

GODOT="${GODOT_BIN:-}"
if [ -z "$GODOT" ]; then
	if command -v godot >/dev/null 2>&1; then
		GODOT="$(command -v godot)"
	else
		GODOT="$(ls "$HOME"/godot/Godot_*_linux.x86_64 2>/dev/null | head -1 || true)"
	fi
fi
if [ -z "$GODOT" ] || [ ! -x "$GODOT" ]; then
	echo "Error: Godot binary not found. Set GODOT_BIN=/path/to/godot" >&2
	exit 1
fi
echo "Godot:  $GODOT"
echo "Project: $PROJECT_DIR"

# --- Wire addons --------------------------------------------------------------

mkdir -p "$PROJECT_DIR/addons"
ln -sfn "$REPO_ROOT/addons/stage" "$PROJECT_DIR/addons/stage"
ln -sfn "$REPO_ROOT/addons/director" "$PROJECT_DIR/addons/director"
echo "Linked addons/stage and addons/director"

# --- Build + copy the GDExtension ---------------------------------------------

GDEXT_DST="$REPO_ROOT/addons/stage/bin/linux/libstage_godot.so"
if [ ! -f "$GDEXT_DST" ]; then
	echo "Building stage-godot (first-time setup)..."
	(cd "$REPO_ROOT" && cargo build -p stage-godot)
	# Cargo target dir may be redirected (see ~/.cargo/config.toml); ask cargo.
	TARGET_DIR="$(cd "$REPO_ROOT" && cargo metadata --no-deps --format-version 1 \
		| sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
	mkdir -p "$(dirname "$GDEXT_DST")"
	cp "$TARGET_DIR/debug/libstage_godot.so" "$GDEXT_DST"
	echo "Copied GDExtension → addons/stage/bin/linux/"
else
	echo "GDExtension already built (rebuild with: cargo build -p stage-godot && cp \"\$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*\"target_directory\":\"\([^\"]*\)\".*/\1/p')\"/debug/libstage_godot.so addons/stage/bin/linux/)"
fi

# --- project.godot -------------------------------------------------------------

if [ ! -f "$PROJECT_DIR/project.godot" ]; then
	cat > "$PROJECT_DIR/project.godot" <<'EOF'
; Engine configuration file.
; It's best edited using the editor UI and not directly,
; but it can also be manually edited with care.

[application]

config/name="Blockfall (theatre dogfood)"
config/features=PackedStringArray("4.7", "Forward Plus")

[autoload]

StageRuntime="*res://addons/stage/runtime.gd"

[editor_plugins]

enabled=PackedStringArray("res://addons/stage/plugin.cfg")
EOF
	echo "Wrote project.godot (StageRuntime autoload enabled)"
else
	if ! grep -q "StageRuntime" "$PROJECT_DIR/project.godot"; then
		echo "Warning: project.godot exists but has no StageRuntime autoload — add:" >&2
		echo '  [autoload] StageRuntime="*res://addons/stage/runtime.gd"' >&2
	else
		echo "project.godot already wired"
	fi
fi

# --- Editor import pass ---------------------------------------------------------
# The runtime loads GDExtensions from .godot/extension_list.cfg, which only an
# editor import pass generates. Headless import needs no display or GPU.

echo "Running editor import pass (generates .godot state)..."
# Transient crashes (e.g. flaky GPU driver state) are possible on first run;
# retry once if the extension list didn't materialize.
for attempt in 1 2; do
	"$GODOT" --headless --import --path "$PROJECT_DIR" >/dev/null 2>&1 || true
	if [ -f "$PROJECT_DIR/.godot/extension_list.cfg" ]; then
		break
	fi
	if [ "$attempt" = 2 ]; then
		echo "Warning: import pass did not produce .godot/extension_list.cfg — open the editor once manually before running." >&2
	fi
done

# --- The game prompt --------------------------------------------------------------

if [ ! -f "$PROJECT_DIR/PROMPT.md" ]; then
	cat > "$PROJECT_DIR/PROMPT.md" <<'EOF'
Build a small 3D Minecraft-style sandbox in Godot 4.7 called "Blockfall":

- A 32×32 block terrain, 1–4 blocks high with simple height variation
  (dirt blocks, grass on top, a few stone blocks mixed in). Individual
  cube MeshInstances are fine — no chunk system, no infinite world.
- First-person player: CharacterBody3D with WASD movement, mouse look,
  gravity, and spacebar jump. Camera3D as a child of the player at head height.
- Left-click breaks the block you're looking at (raycast from camera,
  block disappears with a quick scale-down animation). Right-click places
  a dirt block on the face you're looking at, if the spot is empty.
- One "creeper" mob: a green cube CharacterBody3D that wanders randomly,
  and walks toward the player when within 10m. Name its node "Creeper".
- A torch block that flickers its OmniLight3D intensity randomly
  (0.8–1.2) every 0.1s.
- Falling off the terrain edge respawns the player at the center.
- Day/night cycle: DirectionalLight3D slowly rotating, 60s per cycle.
- Set the main scene as run/main_scene in project.godot.
EOF
	echo "Wrote PROMPT.md (Blockfall game spec)"
fi

# --- Done / start -------------------------------------------------------------------

cat <<EOF

Setup complete. Next:
  1. Generate the game: hand $PROJECT_DIR/PROMPT.md to your agent
     (run it inside $PROJECT_DIR so it writes scenes/scripts there).
  2. Open the editor:  $GODOT --path "$PROJECT_DIR" -e
  3. Run the game:     $GODOT --path "$PROJECT_DIR"
  4. Attach theatre via MCP or CLI once the game is running.
EOF

case "$MODE" in
	--editor)
		exec "$GODOT" --path "$PROJECT_DIR" -e
		;;
	--run)
		echo "Note: a windowed game needs a working GPU driver (host Vulkan is currently wedged — reboot first if this fails)."
		exec "$GODOT" --path "$PROJECT_DIR"
		;;
	"") ;;
esac
