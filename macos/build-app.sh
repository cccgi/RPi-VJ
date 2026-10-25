#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="$(printenv TARGET 2>/dev/null || true)"
if [ -z "$TARGET" ]; then
  MACHINE="$(uname -m)"
  if [ "$MACHINE" = "arm64" ]; then TARGET=aarch64-apple-darwin; else TARGET=x86_64-apple-darwin; fi
fi
APP="$ROOT/target/mac-app/RPi-VJ.app"
CONTENTS="$APP/Contents"
cargo build --release --manifest-path "$ROOT/macos/app/Cargo.toml" --target "$TARGET"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"
cp "$ROOT/target/$TARGET/release/rpi-vj-mac" "$CONTENTS/MacOS/RPi-VJ"
cp "$ROOT/macos/app/Info.plist" "$CONTENTS/Info.plist"
python3 "$ROOT/macos/app/package_library.py" "$CONTENTS/Resources/library"
chmod +x "$CONTENTS/MacOS/RPi-VJ"
echo "APP=$APP"
echo "Open with: open \"$APP\""
