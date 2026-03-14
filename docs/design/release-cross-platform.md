# Design: GitHub Releases + Cross-Platform Builds

## Overview

Automated release pipeline that builds Theatre for Linux x86_64, macOS
x86_64, macOS aarch64 (Apple Silicon), and Windows x86_64 on every tagged
version. Produces:

1. **Per-platform tarballs** — self-contained bundles that unpack into the
   same `~/.local/share/theatre` + `~/.local/bin` layout that `theatre install`
   produces. Users download, unpack, and run `theatre init`.
2. **GitHub Release** — created automatically on tag push with all platform
   artifacts attached and auto-generated release notes.
3. **Updated `.gdextension` manifest** — declares all platform binaries so
   Godot loads the correct one.

### End-User Install (No Rust Needed)

```bash
# Linux/macOS
curl -sSL https://github.com/nklisch/theatre/releases/latest/download/theatre-linux-x86_64.tar.gz \
  | tar xz -C ~/.local
theatre init ~/godot/my-game

# Or download .zip on Windows, extract, add to PATH
```

### Developer Install (From Source)

```bash
git clone https://github.com/nklisch/theatre && cd theatre
cargo run -p theatre-cli -- install
```

Both paths produce the same directory layout — the tarball IS the
`~/.local/{bin,share/theatre}` tree, pre-built.

## Implementation Units

### Unit 1: Update `.gdextension` Manifest for All Platforms

**File**: `addons/spectator/spectator.gdextension`

```ini
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = "4.5"
reloadable = true

[libraries]
linux.debug.x86_64 = "res://addons/spectator/bin/linux/libspectator_godot.so"
linux.release.x86_64 = "res://addons/spectator/bin/linux/libspectator_godot.so"
macos.debug.x86_64 = "res://addons/spectator/bin/macos/libspectator_godot.dylib"
macos.release.x86_64 = "res://addons/spectator/bin/macos/libspectator_godot.dylib"
macos.debug.arm64 = "res://addons/spectator/bin/macos/libspectator_godot.dylib"
macos.release.arm64 = "res://addons/spectator/bin/macos/libspectator_godot.dylib"
windows.debug.x86_64 = "res://addons/spectator/bin/windows/spectator_godot.dll"
windows.release.x86_64 = "res://addons/spectator/bin/windows/spectator_godot.dll"
```

**Implementation Notes**:
- macOS debug and release point to the same file (we ship one build). Same
  pattern as Linux currently.
- macOS x86_64 and arm64 point to the same dylib path. For now, we build
  a single-arch dylib per CI target. Future: could use `lipo` to create a
  universal binary, but that adds complexity. Ship separate platform
  tarballs instead — the user downloads the one matching their arch.
  Godot selects the library by arch tag, so we need separate files OR a
  universal binary. **Decision**: universal binary on macOS (via `lipo`).
  Both arch entries point to the same file, and CI produces a fat binary.
  Only 3 release artifacts: linux, macos-universal, windows.
- Windows uses `spectator_godot.dll` (no `lib` prefix) — matches the
  existing `gdext_filename()` in `paths.rs`.

**Acceptance Criteria**:
- [ ] All 8 library entries present (linux/macos/windows × debug/release, macos × x86_64/arm64)
- [ ] Paths follow `res://addons/spectator/bin/<platform>/` convention

---

### Unit 2: Release Workflow — Cross-Platform Build Matrix

**File**: `.github/workflows/release.yml`

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            runner: ubuntu-latest
            platform: linux
            gdext: libspectator_godot.so
            archive: tar.gz
          - target: x86_64-apple-darwin
            runner: macos-latest
            platform: macos
            gdext: libspectator_godot.dylib
            archive: tar.gz
          - target: aarch64-apple-darwin
            runner: macos-latest
            platform: macos
            gdext: libspectator_godot.dylib
            archive: tar.gz
          - target: x86_64-pc-windows-msvc
            runner: windows-latest
            platform: windows
            gdext: spectator_godot.dll
            archive: zip

    steps:
      - uses: actions/checkout@v5

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ matrix.target }}-cargo-release-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ matrix.target }}-cargo-release-

      - name: Build release binaries
        run: >
          cargo build --release --target ${{ matrix.target }}
          -p spectator-server -p director -p theatre-cli -p spectator-godot

      - name: Prepare archive layout
        shell: bash
        run: |
          TAG="${GITHUB_REF#refs/tags/}"
          ARCHIVE_NAME="theatre-${TAG}-${{ matrix.target }}"
          mkdir -p "${ARCHIVE_NAME}/bin"
          mkdir -p "${ARCHIVE_NAME}/share/theatre/addons"

          # Copy binaries
          BIN_EXT=""
          if [[ "${{ matrix.platform }}" == "windows" ]]; then
            BIN_EXT=".exe"
          fi
          cp "target/${{ matrix.target }}/release/spectator-server${BIN_EXT}" "${ARCHIVE_NAME}/bin/"
          cp "target/${{ matrix.target }}/release/director${BIN_EXT}" "${ARCHIVE_NAME}/bin/"
          cp "target/${{ matrix.target }}/release/theatre${BIN_EXT}" "${ARCHIVE_NAME}/bin/"

          # Copy addon templates (spectator without bin/)
          rsync -a --exclude='bin/' addons/spectator/ "${ARCHIVE_NAME}/share/theatre/addons/spectator/"

          # Copy GDExtension binary into correct platform dir
          mkdir -p "${ARCHIVE_NAME}/share/theatre/addons/spectator/bin/${{ matrix.platform }}"
          cp "target/${{ matrix.target }}/release/${{ matrix.gdext }}" \
            "${ARCHIVE_NAME}/share/theatre/addons/spectator/bin/${{ matrix.platform }}/"

          # Copy director addon (pure GDScript)
          rsync -a addons/director/ "${ARCHIVE_NAME}/share/theatre/addons/director/"

          # Include install script and README
          cp scripts/install-release.sh "${ARCHIVE_NAME}/install.sh" || true
          cp LICENSE "${ARCHIVE_NAME}/"

          echo "ARCHIVE_NAME=${ARCHIVE_NAME}" >> $GITHUB_ENV

      - name: Create macOS universal binary (macOS only)
        if: matrix.target == 'aarch64-apple-darwin'
        run: echo "skip — single arch per tarball for now"
        # Future: lipo -create arm64.dylib x86_64.dylib -output universal.dylib

      - name: Create archive (tar.gz)
        if: matrix.archive == 'tar.gz'
        run: tar czf "${ARCHIVE_NAME}.tar.gz" "${ARCHIVE_NAME}"

      - name: Create archive (zip)
        if: matrix.archive == 'zip'
        shell: bash
        run: 7z a "${ARCHIVE_NAME}.zip" "${ARCHIVE_NAME}"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.ARCHIVE_NAME }}
          path: ${{ env.ARCHIVE_NAME }}.${{ matrix.archive }}

  # Merge macOS builds into a single universal tarball
  macos-universal:
    name: macOS Universal Binary
    runs-on: macos-latest
    needs: build
    steps:
      - uses: actions/checkout@v5

      - name: Download macOS artifacts
        uses: actions/download-artifact@v4
        with:
          pattern: theatre-*-*-apple-darwin
          path: artifacts

      - name: Create universal tarball
        shell: bash
        run: |
          TAG="${GITHUB_REF#refs/tags/}"
          UNIVERSAL="theatre-${TAG}-universal-apple-darwin"
          mkdir -p "${UNIVERSAL}"

          # Extract aarch64 as base (has all files)
          AARCH64_TAR=$(ls artifacts/theatre-*-aarch64-apple-darwin/*.tar.gz)
          tar xzf "${AARCH64_TAR}" --strip-components=1 -C "${UNIVERSAL}"

          # Extract x86_64 dylib
          X86_TAR=$(ls artifacts/theatre-*-x86_64-apple-darwin/*.tar.gz)
          X86_DIR=$(mktemp -d)
          tar xzf "${X86_TAR}" --strip-components=1 -C "${X86_DIR}"

          # Create universal dylib with lipo
          lipo -create \
            "${UNIVERSAL}/share/theatre/addons/spectator/bin/macos/libspectator_godot.dylib" \
            "${X86_DIR}/share/theatre/addons/spectator/bin/macos/libspectator_godot.dylib" \
            -output "${UNIVERSAL}/share/theatre/addons/spectator/bin/macos/libspectator_godot_universal.dylib"
          mv "${UNIVERSAL}/share/theatre/addons/spectator/bin/macos/libspectator_godot_universal.dylib" \
            "${UNIVERSAL}/share/theatre/addons/spectator/bin/macos/libspectator_godot.dylib"

          # Use aarch64 CLI binaries (native on Apple Silicon, Rosetta on Intel)

          tar czf "${UNIVERSAL}.tar.gz" "${UNIVERSAL}"
          echo "UNIVERSAL_ARCHIVE=${UNIVERSAL}.tar.gz" >> $GITHUB_ENV

      - name: Upload universal artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.UNIVERSAL_ARCHIVE }}
          path: ${{ env.UNIVERSAL_ARCHIVE }}

  release:
    name: Create Release
    runs-on: ubuntu-latest
    needs: [build, macos-universal]
    steps:
      - uses: actions/checkout@v5

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: false

      - name: Collect release assets (exclude per-arch macOS — only universal)
        shell: bash
        run: |
          mkdir release-assets
          find artifacts -name '*.tar.gz' -o -name '*.zip' | while read f; do
            # Skip per-arch macOS tarballs (only ship universal)
            if echo "$f" | grep -q 'apple-darwin' && ! echo "$f" | grep -q 'universal'; then
              continue
            fi
            cp "$f" release-assets/
          done
          ls -la release-assets/

      - name: Generate checksums
        working-directory: release-assets
        run: sha256sum * > SHA256SUMS.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: release-assets/*
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Implementation Notes**:

- **Build matrix**: 4 targets. `macos-latest` runs on Apple Silicon (M1+),
  so `aarch64-apple-darwin` is native. `x86_64-apple-darwin` cross-compiles
  on the same runner (Rust supports this natively with `--target`).
- **macOS universal binary**: A separate `macos-universal` job downloads
  both macOS artifacts, uses `lipo` to merge the GDExtension `.dylib` into
  a fat binary, and produces a `universal-apple-darwin` tarball. The CLI
  binaries use the aarch64 build (native on Apple Silicon, runs via Rosetta
  on Intel). Users who want native x86_64 binaries download the
  `x86_64-apple-darwin` tarball directly.
- **Archive layout** mirrors the install layout:
  ```
  theatre-v0.1.0-x86_64-unknown-linux-gnu/
    bin/
      theatre
      spectator-server
      director
    share/theatre/
      addons/
        spectator/   (with bin/linux/*.so)
        director/
    install.sh
    LICENSE
  ```
- **Windows**: Uses `7z` (pre-installed on windows-latest) to create zip.
  Binaries have `.exe` extension.
- **rsync**: Available on all GitHub runners. Cleaner than shell loops for
  directory copies with exclusions.
- **Release notes**: `generate_release_notes: true` auto-generates from
  merged PRs and commit messages. No manual CHANGELOG needed initially.
- **Checksums**: `SHA256SUMS.txt` included in release for verification.
- **`softprops/action-gh-release@v2`**: De facto standard for creating
  GitHub releases. Handles idempotent re-runs (updates existing release if
  tag already has one).

**Acceptance Criteria**:
- [ ] Workflow triggers on `v*` tag push only
- [ ] Builds 4 targets: linux-x86_64, macos-x86_64, macos-aarch64, windows-x86_64
- [ ] macOS universal binary produced via lipo
- [ ] Each target produces a correctly-structured archive
- [ ] GitHub Release created with all archives + SHA256SUMS
- [ ] Release notes auto-generated from commits

---

### Unit 3: Install Script for Tarball Users

**File**: `scripts/install-release.sh`

```bash
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
```

**Implementation Notes**:
- Included inside each release tarball as `install.sh` (see Unit 2 archive
  layout step).
- Mirrors the behaviour of `theatre install` but without cargo — just
  copies pre-built files.
- Windows users won't use this script (they'll have `.exe` and can add to
  PATH manually, or a future `.msi`/`winget` package). Including it in the
  zip doesn't hurt.
- `chmod +x` applied to binaries after copy (tarballs preserve permissions
  but paranoia doesn't hurt).

**Acceptance Criteria**:
- [ ] `./install.sh` copies binaries to `~/.local/bin/`
- [ ] `./install.sh` copies addons to `~/.local/share/theatre/`
- [ ] `./install.sh --bin-dir /opt/bin` overrides bin location
- [ ] `./install.sh` warns when bin_dir not in PATH
- [ ] Script is included in release archives

---

### Unit 4: CI Cross-Platform Checks (Update Existing `ci.yml`)

**File**: `.github/workflows/ci.yml` (modifications)

Add a cross-platform build validation job to the existing CI workflow. This
catches compilation failures on macOS/Windows before a release tag is pushed.

```yaml
  # Existing jobs: check, e2e, build — keep as-is.
  # Add new job:

  cross-build:
    name: Cross-build ${{ matrix.target }}
    runs-on: ${{ matrix.runner }}
    needs: check
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-apple-darwin
            runner: macos-latest
          - target: aarch64-apple-darwin
            runner: macos-latest
          - target: x86_64-pc-windows-msvc
            runner: windows-latest
    steps:
      - uses: actions/checkout@v5

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ matrix.target }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ matrix.target }}-cargo-

      - name: Build (release)
        run: >
          cargo build --release --target ${{ matrix.target }}
          -p spectator-server -p director -p theatre-cli -p spectator-godot
```

**Implementation Notes**:
- `needs: check` — only runs after Linux lint/test passes. No point
  building on 3 more platforms if formatting or tests fail.
- No tests on macOS/Windows — tests require Godot binary which adds
  significant CI complexity. Build verification is sufficient for now.
- `fail-fast: false` — one platform failing doesn't cancel others. Easier
  to diagnose multi-platform issues.
- `aarch64-apple-darwin` on `macos-latest` — GitHub's macOS runners are
  Apple Silicon (M-series). Cross-compiling to `x86_64-apple-darwin` works
  via `--target` (Rust handles it natively, no cross-compilation toolchain
  needed for macOS→macOS).
- Windows: `x86_64-pc-windows-msvc` needs MSVC toolchain, which is
  pre-installed on `windows-latest`.
- `rusqlite` with `bundled` feature compiles the vendored SQLite C code,
  so no system SQLite dependency on any platform.
- `godot` crate (gdext) uses `api-4-5` with `lazy-function-tables` — this
  is pure Rust code generation against Godot's API JSON, no Godot binary
  needed at compile time.

**Acceptance Criteria**:
- [ ] Cross-build job runs on PRs and pushes to main
- [ ] Builds succeed on macOS (x86_64 and aarch64) and Windows
- [ ] Job depends on `check` (doesn't run if lint/test fails)
- [ ] Existing `check`, `e2e`, `build` jobs unchanged

---

### Unit 5: Version Tagging Convention

No new files — this is a process definition referenced by the workflow.

**Tagging convention**:
```bash
# When ready to release:
git tag v0.1.0
git push origin v0.1.0
```

- Tags match `v*` pattern (e.g., `v0.1.0`, `v0.2.0-beta.1`).
- Tag version should match `workspace.package.version` in `Cargo.toml`.
- The release workflow triggers on tag push. The CI workflow runs on the
  same commit via the push-to-main trigger (assuming the tag is on main).

**Release checklist** (for CLAUDE.md or a future RELEASING.md):

```markdown
## Releasing a New Version

1. Update `version` in root `Cargo.toml` `[workspace.package]`
2. Commit: `release: v0.X.Y`
3. Tag: `git tag v0.X.Y`
4. Push: `git push origin main v0.X.Y`
5. CI builds all platforms → GitHub Release created automatically
6. Verify release at https://github.com/nklisch/theatre/releases
```

**Acceptance Criteria**:
- [ ] Release workflow triggers only on `v*` tags
- [ ] Tag name appears in archive filenames and release title
- [ ] Release checklist documented

---

## Implementation Order

1. **Unit 1: `.gdextension` manifest** — Trivial file edit, no dependencies.
   Must be done first so the addon templates include all platform entries
   when copied into tarballs.
2. **Unit 3: `install-release.sh`** — Standalone script, no dependencies.
3. **Unit 4: CI cross-build job** — Add to existing `ci.yml`. Should be
   merged before the release workflow so cross-platform builds are validated
   on PRs.
4. **Unit 2: Release workflow** — The main deliverable. Depends on Unit 1
   (correct `.gdextension`) and Unit 3 (install script bundled in archives).
5. **Unit 5: Version tagging** — Process, documented after workflows are in
   place.

## Testing

### Unit 1: `.gdextension`
- No automated test. Verified by Godot loading the extension on each
  platform (manual or future CI with Godot on macOS/Windows).
- Structural check: grep for all 8 expected library entries.

### Unit 3: `install-release.sh`
- Manual test: create a mock archive layout in a tempdir, run
  `./install.sh --bin-dir /tmp/test-bin --share-dir /tmp/test-share`,
  verify files land correctly.
- Could add a CI step that runs the script against the built archive as a
  smoke test, but low priority.

### Unit 4: CI cross-build
- Self-testing: the job either succeeds (builds compile) or fails
  (compilation error). No separate test needed.

### Unit 2: Release workflow
- **Dry-run test**: Push a `v0.0.0-test` tag to a branch. Verify:
  - All 4 build matrix jobs succeed
  - macOS universal job succeeds
  - Archives contain correct files
  - GitHub Release is created with all artifacts
  - SHA256SUMS.txt is present and correct
- **Artifact inspection**: Download a tarball, extract, verify layout
  matches the expected structure.

### End-to-end verification (manual, post-merge)
```bash
# 1. Tag and push
git tag v0.1.0
git push origin v0.1.0

# 2. Wait for CI (watch Actions tab)

# 3. Download and verify Linux tarball
cd /tmp
curl -sSL https://github.com/nklisch/theatre/releases/download/v0.1.0/theatre-v0.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
cd theatre-v0.1.0-x86_64-unknown-linux-gnu
./install.sh --bin-dir /tmp/test-bin --share-dir /tmp/test-share

# 4. Verify installed files
ls /tmp/test-bin/{theatre,spectator-server,director}
ls /tmp/test-share/addons/spectator/plugin.cfg
ls /tmp/test-share/addons/spectator/bin/linux/libspectator_godot.so
ls /tmp/test-share/addons/director/plugin.cfg

# 5. Test init on a Godot project
/tmp/test-bin/theatre init ~/godot/test-harness --yes
cat ~/godot/test-harness/.mcp.json
```

## Verification Checklist

```bash
# Verify .gdextension has all entries
grep -c 'res://addons/spectator/bin/' addons/spectator/spectator.gdextension
# Expected: 8

# Verify install script is executable
test -x scripts/install-release.sh

# Verify CI workflow syntax
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"

# Verify release workflow triggers on tags
grep 'tags:' .github/workflows/release.yml

# Verify cross-build targets
grep 'target:' .github/workflows/ci.yml | grep -c 'apple\|windows'
# Expected: 3 (x86_64-apple, aarch64-apple, windows)
```
