#!/bin/sh
# Install the latest bountui release from GitHub.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Cedware/bountui/main/scripts/install.sh | sh
#
# The install directory can be customized:
#   curl -fsSL ... | BOUNTUI_INSTALL_DIR=/usr/local/bin sh
set -eu

REPO="Cedware/bountui"
INSTALL_DIR="${BOUNTUI_INSTALL_DIR:-$HOME/.local/bin}"

error() {
    echo "error: $*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "required command not found: $1"
}

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)
            case "$arch" in
                x86_64) echo "x86_64-unknown-linux-musl" ;;
                aarch64 | arm64) echo "aarch64-unknown-linux-gnu" ;;
                *) error "unsupported architecture: $arch" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                aarch64 | arm64) echo "aarch64-apple-darwin" ;;
                *) error "unsupported architecture: $arch" ;;
            esac
            ;;
        *) error "unsupported operating system: $os (on Windows, download the release zip manually)" ;;
    esac
}

need_cmd curl
need_cmd unzip

target="$(detect_target)"

echo "Fetching latest bountui release..."
latest_url="https://api.github.com/repos/$REPO/releases/latest"
version="$(curl -fsSL "$latest_url" | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')"
[ -n "$version" ] || error "failed to determine the latest release version"

asset="bountui-$version-$target.zip"
download_url="https://github.com/$REPO/releases/download/v$version/$asset"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "Downloading $asset..."
curl -fsSL "$download_url" -o "$tmp_dir/$asset"
unzip -q -o "$tmp_dir/$asset" -d "$tmp_dir"

mkdir -p "$INSTALL_DIR"
mv "$tmp_dir/bountui" "$INSTALL_DIR/bountui"
chmod +x "$INSTALL_DIR/bountui"

echo "bountui $version installed to $INSTALL_DIR/bountui"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo "note: $INSTALL_DIR is not in your PATH, add it with:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

echo "bountui will offer future releases in an update dialog on startup."
