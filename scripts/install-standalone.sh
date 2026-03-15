#!/bin/sh
# Theatre standalone installer
# Usage: curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh
#        sh install.sh [--bin-dir DIR] [--share-dir DIR] [--version VERSION] [--yes] [--no-modify-path]

set -eu

REPO="nklisch/theatre"
BIN_DIR="${HOME}/.local/bin"
SHARE_DIR="${HOME}/.local/share/theatre"
VERSION=""
YES=0
NO_MODIFY_PATH=0

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

err() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

warn() {
    printf 'warning: %s\n' "$*" >&2
}

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------

usage() {
    cat <<'EOF'
Theatre installer

USAGE:
    curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh
    sh install.sh [OPTIONS]

OPTIONS:
    -h, --help            Print this help and exit
    -y, --yes             Skip confirmation prompt
    --no-modify-path      Skip adding bin dir to shell profile
    --bin-dir DIR         Override binary install directory (default: ~/.local/bin)
    --share-dir DIR       Override share directory (default: ~/.local/share/theatre)
    --version VERSION     Install a specific version (e.g. 0.2.0, no v prefix)

SUPPORTED PLATFORMS:
    Linux x86_64
    macOS arm64
    macOS x86_64 (via Rosetta)
    Windows x86_64 (MINGW/MSYS)
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        -y|--yes)
            YES=1
            shift
            ;;
        --no-modify-path)
            NO_MODIFY_PATH=1
            shift
            ;;
        --bin-dir)
            [ $# -ge 2 ] || err "--bin-dir requires an argument"
            BIN_DIR="$2"
            shift 2
            ;;
        --share-dir)
            [ $# -ge 2 ] || err "--share-dir requires an argument"
            SHARE_DIR="$2"
            shift 2
            ;;
        --version)
            [ $# -ge 2 ] || err "--version requires an argument"
            VERSION="$2"
            shift 2
            ;;
        *)
            err "Unknown option: $1"
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)
            case "$ARCH" in
                x86_64)
                    TARGET="x86_64-unknown-linux-gnu"
                    EXT="tar.gz"
                    ;;
                *)
                    err "Unsupported Linux architecture: ${ARCH}
Supported platforms:
  Linux x86_64
  macOS arm64
  macOS x86_64 (Rosetta)
  Windows x86_64 (MINGW/MSYS)"
                    ;;
            esac
            ;;
        Darwin)
            case "$ARCH" in
                arm64|x86_64)
                    # x86_64 maps to arm64 target (Rosetta)
                    TARGET="aarch64-apple-darwin"
                    EXT="tar.gz"
                    ;;
                *)
                    err "Unsupported macOS architecture: ${ARCH}
Supported platforms:
  Linux x86_64
  macOS arm64
  macOS x86_64 (Rosetta)
  Windows x86_64 (MINGW/MSYS)"
                    ;;
            esac
            ;;
        MINGW*|MSYS*)
            case "$ARCH" in
                x86_64)
                    TARGET="x86_64-pc-windows-msvc"
                    EXT="zip"
                    ;;
                *)
                    err "Unsupported Windows architecture: ${ARCH}
Supported platforms:
  Linux x86_64
  macOS arm64
  macOS x86_64 (Rosetta)
  Windows x86_64 (MINGW/MSYS)"
                    ;;
            esac
            ;;
        *)
            err "Unsupported operating system: ${OS}
Supported platforms:
  Linux x86_64
  macOS arm64
  macOS x86_64 (Rosetta)
  Windows x86_64 (MINGW/MSYS)"
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Download tool detection
# ---------------------------------------------------------------------------

detect_downloader() {
    if command -v curl > /dev/null 2>&1; then
        DOWNLOADER="curl"
    elif command -v wget > /dev/null 2>&1; then
        DOWNLOADER="wget"
    else
        err "Neither curl nor wget found. Please install one and try again."
    fi
}

download() {
    src="$1"
    dest="$2"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -fsSL -o "$dest" "$src"
    else
        wget -qO "$dest" "$src"
    fi
}

download_stdout() {
    src="$1"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -sS -H "Accept: application/json" "$src"
    else
        wget -qO- --header="Accept: application/json" "$src"
    fi
}

# ---------------------------------------------------------------------------
# Version detection
# ---------------------------------------------------------------------------

get_latest_version() {
    url="https://api.github.com/repos/${REPO}/releases/latest"
    response="$(download_stdout "$url")"
    ver="$(printf '%s' "$response" | sed -n 's/.*"tag_name" *: *"v\([^"]*\)".*/\1/p')"
    if [ -z "$ver" ]; then
        # Fallback: follow redirect and extract version from URL
        if [ "$DOWNLOADER" = "curl" ]; then
            redirect_url="$(curl -sS -o /dev/null -w '%{url_effective}' -L \
                "https://github.com/${REPO}/releases/latest")"
        else
            redirect_url="$(wget --spider -S -o- \
                "https://github.com/${REPO}/releases/latest" 2>&1 \
                | sed -n 's/.*Location: *\(.*\)/\1/p' | tail -1)"
        fi
        ver="$(printf '%s' "$redirect_url" | sed 's|.*/v||')"
    fi
    if [ -z "$ver" ]; then
        err "Could not determine latest Theatre version. Use --version to specify one."
    fi
    printf '%s' "$ver"
}

# ---------------------------------------------------------------------------
# Checksum verification
# ---------------------------------------------------------------------------

verify_checksum() {
    archive_file="$1"
    archive_name="$(basename "$archive_file")"
    checksums_url="https://github.com/${REPO}/releases/download/v${VERSION}/SHA256SUMS.txt"
    checksums_file="${TMPDIR}/SHA256SUMS.txt"

    download "$checksums_url" "$checksums_file" || {
        warn "Could not download SHA256SUMS.txt — skipping checksum verification"
        return 0
    }

    # Extract the line for our archive
    expected="$(grep " ${archive_name}$" "$checksums_file" || true)"
    if [ -z "$expected" ]; then
        warn "No checksum entry found for ${archive_name} — skipping verification"
        return 0
    fi

    # Write a single-line checksums file for our archive
    single_check="${TMPDIR}/check.txt"
    printf '%s\n' "$expected" > "$single_check"

    # Verify: try sha256sum (Linux), then shasum (macOS)
    if command -v sha256sum > /dev/null 2>&1; then
        (cd "$(dirname "$archive_file")" && sha256sum -c "$single_check") || \
            err "Checksum verification failed for ${archive_name}"
    elif command -v shasum > /dev/null 2>&1; then
        (cd "$(dirname "$archive_file")" && shasum -a 256 -c "$single_check") || \
            err "Checksum verification failed for ${archive_name}"
    else
        warn "sha256sum and shasum not found — skipping checksum verification"
    fi
}

# ---------------------------------------------------------------------------
# PATH modification
# ---------------------------------------------------------------------------

add_to_path() {
    # Already in PATH?
    case ":${PATH}:" in
        *":${BIN_DIR}:"*)
            return 0
            ;;
    esac

    line="export PATH=\"${BIN_DIR}:\$PATH\""

    shell_name="$(basename "${SHELL:-sh}")"
    case "$shell_name" in
        zsh)  profile="${HOME}/.zshrc" ;;
        bash) profile="${HOME}/.bashrc" ;;
        *)    profile="${HOME}/.profile" ;;
    esac

    if ! grep -qF "$BIN_DIR" "$profile" 2>/dev/null; then
        printf '\n# Added by Theatre installer\n%s\n' "$line" >> "$profile"
        printf '  Added %s to %s\n' "$BIN_DIR" "$profile"
    fi
}

# ---------------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------------

TMPDIR=""

cleanup() {
    if [ -n "$TMPDIR" ] && [ -d "$TMPDIR" ]; then
        rm -rf "$TMPDIR"
    fi
}

trap cleanup EXIT

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

detect_platform
detect_downloader

if [ -z "$VERSION" ]; then
    printf 'Fetching latest Theatre version...\n'
    VERSION="$(get_latest_version)"
fi

ARCHIVE_NAME="theatre-v${VERSION}-${TARGET}"
ARCHIVE_FILE="${ARCHIVE_NAME}.${EXT}"
ARCHIVE_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE_FILE}"

printf '\n'
printf 'Theatre v%s installer\n' "$VERSION"
printf '\n'
printf '  Platform:  %s\n' "$TARGET"
printf '  Binaries:  %s\n' "$BIN_DIR"
printf '  Addons:    %s\n' "$SHARE_DIR"
printf '\n'

# Confirmation
if [ "$YES" != "1" ] && [ -t 0 ]; then
    printf 'Proceed? [Y/n] '
    read -r answer
    case "$answer" in
        [nN]*)
            printf 'Aborted.\n'
            exit 0
            ;;
    esac
fi

# Temp directory
TMPDIR="$(mktemp -d)"

# Download archive
printf 'Downloading %s...\n' "$ARCHIVE_FILE"
download "$ARCHIVE_URL" "${TMPDIR}/${ARCHIVE_FILE}"

# Verify checksum
verify_checksum "${TMPDIR}/${ARCHIVE_FILE}"

# Extract
printf 'Extracting...\n'
if [ "$EXT" = "tar.gz" ]; then
    tar xzf "${TMPDIR}/${ARCHIVE_FILE}" -C "$TMPDIR"
elif [ "$EXT" = "zip" ]; then
    if command -v unzip > /dev/null 2>&1; then
        unzip -q "${TMPDIR}/${ARCHIVE_FILE}" -d "$TMPDIR"
    else
        err "unzip not found. Please install unzip and try again."
    fi
fi

EXTRACTED="${TMPDIR}/${ARCHIVE_NAME}"
[ -d "$EXTRACTED" ] || err "Expected extracted directory not found: ${EXTRACTED}"

# Install binaries
printf 'Installing binaries...\n'
mkdir -p "$BIN_DIR"
for bin in theatre stage director; do
    if [ -f "${EXTRACTED}/bin/${bin}" ]; then
        cp "${EXTRACTED}/bin/${bin}" "${BIN_DIR}/"
        chmod +x "${BIN_DIR}/${bin}"
    elif [ -f "${EXTRACTED}/bin/${bin}.exe" ]; then
        cp "${EXTRACTED}/bin/${bin}.exe" "${BIN_DIR}/"
    fi
done

# Install addons
printf 'Installing addons...\n'
mkdir -p "$SHARE_DIR"
if [ -d "${EXTRACTED}/share/theatre" ]; then
    cp -r "${EXTRACTED}/share/theatre/." "${SHARE_DIR}/"
fi

# PATH modification
if [ "$NO_MODIFY_PATH" != "1" ]; then
    add_to_path
fi

printf '\n'
printf 'Theatre v%s installed successfully!\n' "$VERSION"
printf '\n'
printf '  Binaries:  %s/{theatre,stage,director}\n' "$BIN_DIR"
printf '  Addons:    %s/addons/\n' "$SHARE_DIR"
printf '\n'
printf 'Next steps:\n'
printf '  1. theatre init ~/path/to/your-godot-project\n'
printf '  2. Open the project in Godot and enable the plugin(s)\n'
printf '\n'
printf 'Docs: https://nklisch.github.io/theatre/\n'
