#!/usr/bin/env bash
# Install Theatre from a release tarball.
# Usage: ./install.sh [--bin-dir DIR] [--share-dir DIR]
#
# Defaults:
#   bin-dir:   ~/.local/bin
#   share-dir: ~/.local/share/theatre

set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
SHARE_DIR="${HOME}/.local/share/theatre"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin-dir)   BIN_DIR="$2"; shift 2 ;;
        --share-dir) SHARE_DIR="$2"; shift 2 ;;
        *)           echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Theatre Install (from release)"
echo

# Copy binaries
echo "  Installing binaries to ${BIN_DIR}/"
mkdir -p "${BIN_DIR}"
for bin in theatre spectator-server director; do
    if [[ -f "${SCRIPT_DIR}/bin/${bin}" ]]; then
        cp "${SCRIPT_DIR}/bin/${bin}" "${BIN_DIR}/"
        chmod +x "${BIN_DIR}/${bin}"
        echo "  ✓ ${bin}"
    elif [[ -f "${SCRIPT_DIR}/bin/${bin}.exe" ]]; then
        cp "${SCRIPT_DIR}/bin/${bin}.exe" "${BIN_DIR}/"
        echo "  ✓ ${bin}.exe"
    fi
done
echo

# Copy share data
echo "  Installing addons to ${SHARE_DIR}/"
mkdir -p "${SHARE_DIR}"
if [[ -d "${SCRIPT_DIR}/share/theatre" ]]; then
    cp -r "${SCRIPT_DIR}/share/theatre/"* "${SHARE_DIR}/"
    echo "  ✓ addons/spectator/"
    echo "  ✓ addons/director/"
fi
echo

# PATH check
if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${BIN_DIR}"; then
    echo "  ⚠ ${BIN_DIR} is not in your PATH. Add it:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo
fi

echo "Install complete. Run 'theatre init <project>' to set up a Godot project."
