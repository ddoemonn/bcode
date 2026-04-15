#!/usr/bin/env bash
set -euo pipefail

REPO="ddoemonn/bcode"
BINARY="bcode"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

detect_platform() {
    local os arch

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux)
            case "$arch" in
                x86_64|amd64) echo "linux-x86_64" ;;
                aarch64|arm64) echo "linux-aarch64" ;;
                *) echo "unsupported arch: $arch" >&2; exit 1 ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64) echo "macos-x86_64" ;;
                arm64) echo "macos-aarch64" ;;
                *) echo "unsupported arch: $arch" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "unsupported OS: $os" >&2
            exit 1
            ;;
    esac
}

main() {
    local platform
    platform=$(detect_platform)

    local artifact="${BINARY}-${platform}"

    local latest_tag
    latest_tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"([^"]+)".*/\1/')

    if [[ -z "$latest_tag" ]]; then
        echo "failed to fetch latest release tag" >&2
        exit 1
    fi

    local url="https://github.com/${REPO}/releases/download/${latest_tag}/${artifact}"

    echo "installing bcode ${latest_tag} for ${platform}"

    local tmp
    tmp=$(mktemp)
    curl -fsSL "$url" -o "$tmp"
    chmod +x "$tmp"

    if [[ -w "$INSTALL_DIR" ]]; then
        mv "$tmp" "${INSTALL_DIR}/${BINARY}"
    else
        sudo mv "$tmp" "${INSTALL_DIR}/${BINARY}"
    fi

    echo "installed: ${INSTALL_DIR}/${BINARY}"
    echo ""
    echo "run: bcode"
}

main "$@"
