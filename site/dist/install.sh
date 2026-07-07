#!/usr/bin/env bash
set -euo pipefail

# utxray installer
# Usage:
#   curl -sSfL https://raw.githubusercontent.com/cauu/utxray/main/install.sh | bash
#   # or locally:
#   bash install.sh

REPO="cauu/utxray"
BINARY="utxray"
INSTALL_DIR="${UTXRAY_INSTALL_DIR:-/usr/local/bin}"

# Colors (disabled if not a TTY)
if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; BOLD='\033[1m'; RESET='\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BOLD=''; RESET=''
fi

info()  { echo -e "${GREEN}[info]${RESET}  $*"; }
warn()  { echo -e "${YELLOW}[warn]${RESET}  $*"; }
error() { echo -e "${RED}[error]${RESET} $*" >&2; }
fatal() { error "$@"; exit 1; }

detect_os() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  OS="linux" ;;
    Darwin) OS="darwin" ;;
    *)      fatal "Unsupported OS: $os" ;;
  esac

  case "$arch" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)   ARCH="aarch64" ;;
    *)               fatal "Unsupported architecture: $arch" ;;
  esac
}

check_deps() {
  for cmd in curl tar; do
    command -v "$cmd" >/dev/null 2>&1 || fatal "Required command not found: $cmd"
  done
}

# Try downloading a prebuilt release from GitHub
try_github_release() {
  info "Checking for prebuilt release..."

  local release_url="https://api.github.com/repos/${REPO}/releases/latest"
  local release_json
  release_json="$(curl -sf "$release_url" 2>/dev/null || echo "")"

  if [ -z "$release_json" ]; then
    warn "No GitHub releases found. Will build from source."
    return 1
  fi

  local tag
  tag="$(echo "$release_json" | grep '"tag_name"' | head -1 | sed 's/.*: *"\(.*\)".*/\1/')"

  if [ -z "$tag" ]; then
    warn "Could not parse release tag. Will build from source."
    return 1
  fi

  # Try to find a matching asset
  local asset_pattern="${BINARY}-${ARCH}-${OS}"
  local download_url
  download_url="$(echo "$release_json" | grep '"browser_download_url"' | grep "$asset_pattern" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')"

  if [ -z "$download_url" ]; then
    warn "No prebuilt binary for ${ARCH}-${OS} in release ${tag}. Will build from source."
    return 1
  fi

  info "Downloading ${BINARY} ${tag} for ${ARCH}-${OS}..."
  local tmp
  tmp="$(mktemp -d)"
  CLEANUP_DIR="$tmp"
  trap 'rm -rf "${CLEANUP_DIR:-}"' EXIT

  if curl -sfL "$download_url" -o "$tmp/release.tar.gz"; then
    tar xzf "$tmp/release.tar.gz" -C "$tmp" 2>/dev/null || {
      # Maybe it's a raw binary, not a tarball
      mv "$tmp/release.tar.gz" "$tmp/$BINARY"
    }

    if [ -f "$tmp/$BINARY" ]; then
      chmod +x "$tmp/$BINARY"
      install_binary "$tmp/$BINARY"
      return 0
    fi

    # Search for the binary in extracted files
    local found
    found="$(find "$tmp" -name "$BINARY" -type f | head -1)"
    if [ -n "$found" ]; then
      chmod +x "$found"
      install_binary "$found"
      return 0
    fi
  fi

  warn "Download failed. Will build from source."
  return 1
}

# Build from source using cargo
build_from_source() {
  info "Building from source..."

  if ! command -v cargo >/dev/null 2>&1; then
    error "cargo not found."
    echo ""
    echo "  Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    fatal "Cannot build without Rust toolchain."
  fi

  local rust_version
  rust_version="$(rustc --version | awk '{print $2}')"
  info "Using Rust ${rust_version}"

  # If we're in the repo directory, build locally
  if [ -f "Cargo.toml" ] && grep -q 'name = "utxray"' Cargo.toml 2>/dev/null; then
    info "Building in current directory..."
    cargo build --release
    install_binary "target/release/${BINARY}"
    return 0
  fi

  # Otherwise, clone and build
  local tmp
  tmp="$(mktemp -d)"
  CLEANUP_DIR="$tmp"
  trap 'rm -rf "${CLEANUP_DIR:-}"' EXIT

  info "Cloning ${REPO}..."
  git clone --depth 1 "https://github.com/${REPO}.git" "$tmp/utxray"
  cd "$tmp/utxray"

  info "Building (this may take a few minutes)..."
  cargo build --release

  install_binary "target/release/${BINARY}"
}

install_binary() {
  local src="$1"

  if [ ! -f "$src" ]; then
    fatal "Binary not found: $src"
  fi

  info "Installing to ${INSTALL_DIR}/${BINARY}..."

  # Try direct copy first, fall back to sudo
  if cp "$src" "${INSTALL_DIR}/${BINARY}" 2>/dev/null; then
    chmod +x "${INSTALL_DIR}/${BINARY}"
  elif command -v sudo >/dev/null 2>&1; then
    warn "Permission denied. Retrying with sudo..."
    sudo cp "$src" "${INSTALL_DIR}/${BINARY}"
    sudo chmod +x "${INSTALL_DIR}/${BINARY}"
  else
    error "Cannot write to ${INSTALL_DIR}."
    echo ""
    echo "  Try one of:"
    echo "    sudo bash install.sh"
    echo "    UTXRAY_INSTALL_DIR=~/.local/bin bash install.sh"
    echo ""
    fatal "Installation failed."
  fi

  # Verify
  if ! command -v "$BINARY" >/dev/null 2>&1; then
    warn "${INSTALL_DIR} may not be in your PATH."
    echo ""
    echo "  Add to your shell profile:"
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
  fi
}

verify_install() {
  local installed
  installed="$(command -v "$BINARY" 2>/dev/null || echo "")"

  if [ -z "$installed" ]; then
    warn "utxray installed but not found in PATH."
    echo "  Binary location: ${INSTALL_DIR}/${BINARY}"
    return
  fi

  local version
  version="$("$BINARY" --version 2>/dev/null || echo "unknown")"
  echo ""
  info "${BOLD}utxray installed successfully!${RESET}"
  echo ""
  echo "  Version:  ${version}"
  echo "  Location: ${installed}"
  echo ""
  echo "  Get started:"
  echo "    utxray env                    # Check environment"
  echo "    utxray --help                 # See all commands"
  echo ""
}

main() {
  echo ""
  echo "  ${BOLD}utxray installer${RESET}"
  echo "  UTxO X-Ray — Cardano smart contract debugger for AI agents"
  echo ""

  detect_os
  check_deps

  info "Detected: ${OS}/${ARCH}"

  # Strategy: try prebuilt release first, fall back to source build
  if try_github_release; then
    verify_install
    return 0
  fi

  build_from_source
  verify_install
}

main "$@"
