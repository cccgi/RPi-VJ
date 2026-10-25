#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$(uname -s)" != "Darwin" ]; then
  echo "Run this setup on macOS. It prepares Rust dependencies and builds the Mac app."
  exit 1
fi
if ! xcode-select -p >/dev/null 2>&1; then
  echo "Apple Command Line Tools are required. Starting Apple's installer; rerun this script when installation finishes."
  xcode-select --install || true
  exit 1
fi
if ! command -v rustup >/dev/null 2>&1; then
  echo "Installing the stable Rust toolchain with the official rustup installer..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install stable
rustup default stable
MACHINE="$(uname -m)"
if [ "$MACHINE" = "arm64" ]; then
  TARGET=aarch64-apple-darwin
elif [ "$MACHINE" = "x86_64" ]; then
  TARGET=x86_64-apple-darwin
else
  echo "Unsupported Mac CPU architecture: $MACHINE"
  exit 1
fi
rustup target add "$TARGET"
echo "Fetching Rust dependencies..."
cargo fetch --manifest-path "$ROOT/macos/app/Cargo.toml"
echo "Building the RPi-VJ Mac app..."
TARGET="$TARGET" bash "$ROOT/macos/build-app.sh"
echo "Ready: $ROOT/target/mac-app/RPi-VJ.app"
