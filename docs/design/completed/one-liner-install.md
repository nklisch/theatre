# Design: One-Liner Install Script

## Overview

Enable users to install Theatre with:

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh
```

A standalone POSIX shell script that detects platform, downloads the correct
release archive from GitHub, extracts it, installs binaries and addons, and
prints next steps.

## Implementation Units

### Unit 1: Standalone Install Script

**File**: `scripts/install-standalone.sh`

POSIX `sh` (no bashisms) for maximum portability. macOS ships zsh by default
and bash 3.2 — avoid arrays, `[[ ]]`, `set -o pipefail`, process substitution.

#### Platform Detection

Map `uname -s` / `uname -m` to Rust target triples matching `release.yml`:

```
uname -s     uname -m    Target triple                   Archive ext
--------     --------    -------------                   -----------
Linux        x86_64      x86_64-unknown-linux-gnu        tar.gz
Darwin       arm64       aarch64-apple-darwin             tar.gz
Darwin       x86_64      aarch64-apple-darwin             tar.gz  (Rosetta)
MINGW*       x86_64      x86_64-pc-windows-msvc           zip
MSYS*        x86_64      x86_64-pc-windows-msvc           zip
```

Unsupported combinations (e.g., Linux aarch64) print a clear error listing
supported platforms and exit 1.

macOS x86_64 maps to the arm64 target since macOS runners only build arm64 and
Rosetta handles translation. If a native x86_64 build is added to CI later,
add a row to the table.

#### CLI Flags

```
--help, -h            Print usage and exit
--no-modify-path      Skip adding bin dir to shell profile
--bin-dir DIR         Override binary install directory (default: ~/.local/bin)
--share-dir DIR       Override share directory (default: ~/.local/share/theatre)
--version VERSION     Install specific version (e.g., 0.2.0, no v prefix)
--yes, -y             Skip confirmation prompt
```

Parse with a `while` loop over `$#` using `case`. No `getopts` (not portable
enough for long flags).

#### Script Flow

```
1. Parse flags
2. Detect OS + arch → target triple + archive extension
3. Detect download tool (curl preferred, wget fallback)
4. Determine version:
   - If --version set, use it
   - Else, query GitHub API for latest release tag
5. Construct archive URL:
   https://github.com/nklisch/theatre/releases/download/v{VER}/theatre-v{VER}-{TARGET}.{EXT}
6. Print install summary (version, platform, directories)
7. Prompt for confirmation (unless --yes or stdin is not a tty)
8. Create temp dir, trap EXIT for cleanup
9. Download archive
10. Download SHA256SUMS.txt, verify checksum (warn if sha256sum/shasum missing)
11. Extract archive
12. Copy bin/* to $BIN_DIR (chmod +x)
13. Copy share/theatre/* to $SHARE_DIR
14. Optionally add $BIN_DIR to PATH in shell profile (unless --no-modify-path)
15. Print success + next steps
```

#### Version Detection (no jq dependency)

```sh
get_latest_version() {
    url="https://api.github.com/repos/nklisch/theatre/releases/latest"
    if [ "$DOWNLOADER" = "curl" ]; then
        response=$(curl -sS -H "Accept: application/json" "$url")
    else
        response=$(wget -qO- --header="Accept: application/json" "$url")
    fi
    echo "$response" | sed -n 's/.*"tag_name" *: *"v\([^"]*\)".*/\1/p'
}
```

If the API is rate-limited (empty result), fall back to scraping the redirect:

```sh
# Fallback: follow redirect from /releases/latest and extract tag from URL
if [ "$DOWNLOADER" = "curl" ]; then
    redirect_url=$(curl -sS -o /dev/null -w '%{url_effective}' -L \
        "https://github.com/nklisch/theatre/releases/latest")
else
    redirect_url=$(wget --spider -S -o- \
        "https://github.com/nklisch/theatre/releases/latest" 2>&1 \
        | sed -n 's/.*Location: *\(.*\)/\1/p' | tail -1)
fi
version=$(echo "$redirect_url" | sed 's|.*/v||')
```

#### Checksum Verification

```sh
verify_checksum() {
    checksums_url="https://github.com/nklisch/theatre/releases/download/v${VERSION}/SHA256SUMS.txt"
    # download checksums file
    # extract line matching our archive
    # verify with sha256sum (Linux) or shasum -a 256 (macOS)
    # if neither tool available, warn and continue
}
```

#### PATH Modification

Detect shell from `$SHELL` basename, append to one file only:

```sh
add_to_path() {
    line="export PATH=\"${BIN_DIR}:\$PATH\""
    case "$(basename "$SHELL")" in
        zsh)  profile="$HOME/.zshrc" ;;
        bash) profile="$HOME/.bashrc" ;;
        *)    profile="$HOME/.profile" ;;
    esac
    if ! grep -qF "$BIN_DIR" "$profile" 2>/dev/null; then
        printf '\n# Added by Theatre installer\n%s\n' "$line" >> "$profile"
        echo "  Added ${BIN_DIR} to ${profile}"
    fi
}
```

Skip if `--no-modify-path` or if `$BIN_DIR` is already in `$PATH`.

#### Confirmation Prompt

```sh
# Auto-yes if --yes flag or stdin is not a terminal (piped install)
if [ "$YES" = "1" ] || [ ! -t 0 ]; then
    # proceed
else
    printf "Proceed? [Y/n] "
    read -r answer
    case "$answer" in
        [nN]*) echo "Aborted."; exit 0 ;;
    esac
fi
```

Note: when piped via `curl | sh`, stdin is the script itself, not a tty, so
the prompt auto-confirms. This matches rustup/uv behavior — users who pipe to
sh have already consented. If the user runs `sh install.sh` interactively
(downloaded file), they get the prompt.

#### Error Handling

- `set -eu` at top (no `pipefail` — not POSIX)
- `err()` helper prints to stderr and exits
- `warn()` helper prints to stderr, does not exit
- `trap cleanup EXIT` removes temp directory
- `curl -fSL` causes non-zero exit on HTTP errors (404, etc.)

#### Success Output

```
Theatre v0.2.0 installed successfully!

  Binaries:  ~/.local/bin/{theatre,stage,director}
  Addons:    ~/.local/share/theatre/addons/

Next steps:
  1. theatre init ~/path/to/your-godot-project
  2. Open the project in Godot and enable the plugin(s)

Docs: https://nklisch.github.io/theatre/
```

**Acceptance Criteria**:
- [ ] Script is valid POSIX sh (passes `shellcheck -s sh`)
- [ ] Detects Linux x86_64, macOS arm64, macOS x86_64 (Rosetta), MINGW/MSYS
- [ ] Exits with clear error on unsupported platforms
- [ ] Downloads correct archive for detected platform
- [ ] Verifies SHA256 checksum when tools available
- [ ] Installs binaries to `~/.local/bin` and addons to `~/.local/share/theatre`
- [ ] `--bin-dir` and `--share-dir` override defaults
- [ ] `--version` installs a specific version
- [ ] `--no-modify-path` skips PATH modification
- [ ] `--yes` skips confirmation
- [ ] Cleans up temp directory on success and failure
- [ ] Works when piped: `curl ... | sh`
- [ ] Works when piped with args: `curl ... | sh -s -- --yes`

---

### Unit 2: Release Workflow Update

**File**: `.github/workflows/release.yml`

Add a step in the `release` job to copy the standalone script into release
assets so it appears at the `latest/download/install.sh` URL.

```yaml
# After "Collect release assets", before "Generate checksums"
      - name: Copy standalone install script
        run: cp scripts/install-standalone.sh release-assets/install.sh
```

**Acceptance Criteria**:
- [ ] `install.sh` appears as a top-level release asset
- [ ] Accessible at `https://github.com/nklisch/theatre/releases/latest/download/install.sh`
- [ ] SHA256SUMS.txt includes `install.sh`

---

## Implementation Order

1. **Unit 1**: Write `scripts/install-standalone.sh`
2. **Unit 2**: Update `release.yml` to upload the script
3. Test locally with an existing release (or create a test release)

## Testing

### Manual Testing (pre-release)

Point the script at an existing release to verify download + extract + install:

```bash
# Test on current machine
sh scripts/install-standalone.sh --yes --version 0.1.0

# Verify
theatre --version
ls ~/.local/share/theatre/addons/stage/
ls ~/.local/share/theatre/addons/director/

# Test custom dirs
sh scripts/install-standalone.sh --yes --version 0.1.0 \
    --bin-dir /tmp/t-bin --share-dir /tmp/t-share
ls /tmp/t-bin/theatre /tmp/t-bin/stage /tmp/t-bin/director
```

### Lint

```bash
shellcheck -s sh scripts/install-standalone.sh
```

### Post-Release Smoke Test

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh -s -- --yes
theatre --version
```

## Files Changed

| File | Action |
|---|---|
| `scripts/install-standalone.sh` | Create |
| `.github/workflows/release.yml` | Add 3-line step |

## Out of Scope

- Uninstall command
- Self-update mechanism
- Linux aarch64 / macOS x86_64 native builds (just add CI matrix rows later)
- Homebrew formula / winget package
- CI smoke test workflow for the install script

## Verification Checklist

```bash
shellcheck -s sh scripts/install-standalone.sh
sh scripts/install-standalone.sh --help
sh scripts/install-standalone.sh --yes --version 0.1.0
theatre --version
```
