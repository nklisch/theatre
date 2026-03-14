#!/usr/bin/env bash
# Usage: ./scripts/release.sh <patch|minor|major|x.y.z>
#
# Bumps version in Cargo.toml [workspace.package], commits, tags, and pushes.
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

# Update version in Cargo.toml
sed -i "s/^version = \"${CURRENT}\"/version = \"${NEXT}\"/" "$CARGO_TOML"
echo "  updated $CARGO_TOML"

# Verify it parses
cargo metadata --format-version=1 --no-deps > /dev/null 2>&1 \
    || { echo "Error: Cargo.toml is invalid after version bump"; exit 1; }

# Commit, tag, push
TAG="v${NEXT}"
git add "$CARGO_TOML"
git commit -m "release: ${TAG}"
git tag "$TAG"
git push
git push origin "$TAG"

echo
echo "Released ${TAG}. GitHub Actions will build and publish the release."
echo "  https://github.com/nklisch/theatre/actions"
