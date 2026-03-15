#!/usr/bin/env bash
# Usage: ./scripts/release.sh <patch|minor|major|x.y.z>
#
# Bumps version everywhere, commits, tags, and pushes.
# The release.yml workflow picks up the tag and builds all platforms.

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: ./scripts/release.sh <patch|minor|major|x.y.z>"
    echo
    echo "Examples:"
    echo "  ./scripts/release.sh patch    # 0.1.0 → 0.1.1"
    echo "  ./scripts/release.sh minor    # 0.1.0 → 0.2.0"
    echo "  ./scripts/release.sh major    # 0.1.0 → 1.0.0"
    echo "  ./scripts/release.sh 2.0.0    # explicit version"
    exit 1
fi

ARG="$1"
CARGO_TOML="Cargo.toml"

# Parse current version from workspace Cargo.toml
CURRENT=$(grep -A5 '^\[workspace\.package\]' "$CARGO_TOML" \
    | grep '^version' \
    | head -1 \
    | sed 's/.*"\(.*\)".*/\1/')

if [[ -z "$CURRENT" ]]; then
    echo "Error: could not parse version from $CARGO_TOML"
    exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$ARG" in
    patch) NEXT="${MAJOR}.${MINOR}.$((PATCH + 1))" ;;
    minor) NEXT="${MAJOR}.$((MINOR + 1)).0" ;;
    major) NEXT="$((MAJOR + 1)).0.0" ;;
    [0-9]*.[0-9]*.[0-9]*)  NEXT="$ARG" ;;
    *)
        echo "Error: unknown version argument: $ARG"
        echo "Expected: patch, minor, major, or x.y.z"
        exit 1
        ;;
esac

echo "${CURRENT} → ${NEXT}"
TAG="v${NEXT}"
TODAY=$(date +%Y-%m-%d)

# --- 1. Cargo.toml (workspace version) ---
sed -i "s/^version = \"${CURRENT}\"/version = \"${NEXT}\"/" "$CARGO_TOML"
echo "  updated $CARGO_TOML"

# Verify it parses
cargo metadata --format-version=1 --no-deps > /dev/null 2>&1 \
    || { echo "Error: Cargo.toml is invalid after version bump"; exit 1; }

# --- 2. Godot addon plugin.cfg files ---
for cfg in addons/stage/plugin.cfg addons/director/plugin.cfg; do
    if [[ -f "$cfg" ]]; then
        sed -i "s/^version=\"${CURRENT}\"/version=\"${NEXT}\"/" "$cfg"
        echo "  updated $cfg"
    fi
done

# --- 3. Changelog (site/changelog.md) ---
CHANGELOG="site/changelog.md"
if [[ -f "$CHANGELOG" ]]; then
    # Insert new version header after the "## [Unreleased]" section marker
    # Adds a blank separator and new version header right before the first "---" after [Unreleased]
    sed -i "/^## \[Unreleased\]/,/^---$/ {
        /^---$/ i\\\\n## [${NEXT}] — ${TODAY}
    }" "$CHANGELOG"

    # Update footer links: add new version link and update [Unreleased] compare base
    sed -i "s|\[Unreleased\]: \(.*\)/compare/v${CURRENT}\.\.\.HEAD|[Unreleased]: \1/compare/v${NEXT}...HEAD|" "$CHANGELOG"
    # Append new version link before [Unreleased] line if not already present
    if ! grep -q "^\[${NEXT}\]:" "$CHANGELOG"; then
        sed -i "/^\[Unreleased\]:/a [${NEXT}]: https://github.com/nklisch/theatre/releases/tag/v${NEXT}" "$CHANGELOG"
    fi
    echo "  updated $CHANGELOG"
fi

# --- 4. Documentation version strings ---
# installation.md: version in CLI output example
INSTALL_MD="site/guide/installation.md"
if [[ -f "$INSTALL_MD" ]]; then
    sed -i "s/\"version\": \"${CURRENT}\"/\"version\": \"${NEXT}\"/g" "$INSTALL_MD"
    # Update --version flag example
    sed -i "s/--version ${CURRENT}/--version ${NEXT}/g" "$INSTALL_MD"
    echo "  updated $INSTALL_MD"
fi

# wire-format.md: handshake version examples
WIRE_MD="site/api/wire-format.md"
if [[ -f "$WIRE_MD" ]]; then
    sed -i "s/\"version\": \"${CURRENT}\"/\"version\": \"${NEXT}\"/g" "$WIRE_MD"
    sed -i "s/server ${CURRENT}/server ${NEXT}/g" "$WIRE_MD"
    echo "  updated $WIRE_MD"
fi

# --- 5. Regenerate Cargo.lock ---
cargo check --workspace > /dev/null 2>&1 || true
echo "  updated Cargo.lock"

# --- Commit, tag, push ---
git add "$CARGO_TOML" Cargo.lock \
    addons/stage/plugin.cfg addons/director/plugin.cfg \
    "$CHANGELOG" "$INSTALL_MD" "$WIRE_MD" 2>/dev/null || true
git commit -m "release: ${TAG}"
git tag "$TAG"
git push
git push origin "$TAG"

echo
echo "Released ${TAG}. GitHub Actions will build and publish the release."
echo "  https://github.com/nklisch/theatre/actions"
