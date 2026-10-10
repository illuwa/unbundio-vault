#!/bin/bash
# Build libunbundio_vault.a for an iOS target and type-check the Swift shell.
#
#   ./mobile/ios/build.sh sim      # simulator (fast, what CI can do)
#   ./mobile/ios/build.sh device   # aarch64-apple-ios (needs signing to run)
#
# The static library is built by Cargo; Xcode is only used to compile the Swift
# sources so a signature mistake fails here instead of in Xcode.
set -euo pipefail

MODE="${1:-sim}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

case "$MODE" in
  sim)    RUST_TARGET="aarch64-apple-ios-sim"; SDK="iphonesimulator" ;;
  device) RUST_TARGET="aarch64-apple-ios";      SDK="iphoneos" ;;
  *) echo "usage: $0 [sim|device]" >&2; exit 1 ;;
esac

echo "==> building libunbundio_vault.a for $RUST_TARGET"
cargo build --release --lib --target "$RUST_TARGET"
LIB="$ROOT/target/$RUST_TARGET/release/libunbundio_vault.a"
[ -f "$LIB" ] || { echo "missing $LIB" >&2; exit 1; }
echo "    $(ls -lh "$LIB" | awk '{print $5}') $LIB"

echo "==> type-checking the Swift shell ($SDK)"
SDK_PATH="$(xcrun --sdk "$SDK" --show-sdk-path)"
SDK_VERSION="$(xcrun --sdk "$SDK" --show-sdk-version)"
# The host triple's OS version must track the SDK, and the simulator needs the
# explicit "-simulator" suffix, or the Swift driver silently targets macOS.
if [ "$MODE" = "sim" ]; then
  TRIPLE="arm64-apple-ios$(echo "$SDK_VERSION" | cut -d. -f1).0-simulator"
else
  TRIPLE="arm64-apple-ios$(echo "$SDK_VERSION" | cut -d. -f1).0"
fi
echo "    target triple: $TRIPLE"

BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT

xcrun --sdk "$SDK" swiftc \
  -target "$TRIPLE" \
  -sdk "$SDK_PATH" \
  -module-name UnbundioVault \
  -emit-module \
  -emit-module-path "$BUILD_DIR/UnbundioVault.swiftmodule" \
  -import-objc-header "$ROOT/mobile/ios/UnbundioVault.h" \
  "$ROOT/mobile/ios"/*.swift

echo "==> Swift shell compiles for $TRIPLE"