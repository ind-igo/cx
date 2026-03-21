#!/bin/sh
set -e

REPO="ind-igo/cx"
BINARY="cx"
INSTALL_DIR="/usr/local/bin"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  OS="unknown-linux-gnu" ;;
  Darwin) OS="apple-darwin" ;;
  *)      echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64)  ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *)       echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"
URL="https://github.com/${REPO}/releases/latest/download/cx-${TARGET}.tar.gz"

echo "Installing cx (${TARGET})..."

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

curl -sL "$URL" | tar xz -C "$tmpdir"

if [ -w "$INSTALL_DIR" ]; then
  mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
else
  sudo mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
fi

echo "cx installed to ${INSTALL_DIR}/cx"
