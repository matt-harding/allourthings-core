#!/usr/bin/env bash
# build-ios.sh — Build allourthings_core as an XCFramework for iOS
#
# Outputs (within this repo):
#   build/allourthings_core.xcframework        — XCFramework (gitignored)
#   Sources/AllourthingsCore/allourthings_core.swift  — Swift bindings (committed)
#
# The allourthings-ios Xcode project consumes this crate as a local Swift Package
# Manager dependency pointing at this repo. After running this script, open
# allourthings-ios in Xcode — SPM picks up the XCFramework automatically.
#
# Usage:  ./scripts/build-ios.sh [--debug]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(dirname "$SCRIPT_DIR")"

PROFILE="release"
CARGO_FLAGS="--release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    CARGO_FLAGS=""
fi

CRATE="allourthings_core"
XCFRAMEWORK_DIR="$CRATE_DIR/build/${CRATE}.xcframework"
SWIFT_SOURCES_DIR="$CRATE_DIR/Sources/AllourthingsCore"
HEADERS_DEVICE="$CRATE_DIR/target/headers-device"
HEADERS_SIM="$CRATE_DIR/target/headers-sim"

echo "==> Adding iOS Rust targets"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

echo "==> Building for iOS device (aarch64-apple-ios)"
cargo build $CARGO_FLAGS --features uniffi --target aarch64-apple-ios

echo "==> Building for iOS simulator arm64 (aarch64-apple-ios-sim)"
cargo build $CARGO_FLAGS --features uniffi --target aarch64-apple-ios-sim

echo "==> Building for iOS simulator x86_64 (x86_64-apple-ios)"
cargo build $CARGO_FLAGS --features uniffi --target x86_64-apple-ios

echo "==> Generating Swift bindings"
cargo run $CARGO_FLAGS --features uniffi --bin uniffi-bindgen -- generate \
    --library "target/aarch64-apple-ios/$PROFILE/lib${CRATE}.a" \
    --language swift \
    --out-dir "$CRATE_DIR/target/uniffi-swift"

mkdir -p "$SWIFT_SOURCES_DIR"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}.swift" "$SWIFT_SOURCES_DIR/"

echo "==> Creating fat simulator library (arm64 + x86_64)"
mkdir -p "$CRATE_DIR/target/ios-sim-fat/$PROFILE"
lipo -create \
    "$CRATE_DIR/target/aarch64-apple-ios-sim/$PROFILE/lib${CRATE}.a" \
    "$CRATE_DIR/target/x86_64-apple-ios/$PROFILE/lib${CRATE}.a" \
    -output "$CRATE_DIR/target/ios-sim-fat/$PROFILE/lib${CRATE}.a"

echo "==> Copying headers"
mkdir -p "$HEADERS_DEVICE" "$HEADERS_SIM"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.h"         "$HEADERS_DEVICE/"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.modulemap" "$HEADERS_DEVICE/"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.modulemap" "$HEADERS_DEVICE/module.modulemap"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.h"         "$HEADERS_SIM/"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.modulemap" "$HEADERS_SIM/"
cp "$CRATE_DIR/target/uniffi-swift/${CRATE}FFI.modulemap" "$HEADERS_SIM/module.modulemap"

echo "==> Creating XCFramework"
rm -rf "$XCFRAMEWORK_DIR"
mkdir -p "$(dirname "$XCFRAMEWORK_DIR")"

xcodebuild -create-xcframework \
    -library "$CRATE_DIR/target/aarch64-apple-ios/$PROFILE/lib${CRATE}.a" \
    -headers "$HEADERS_DEVICE" \
    -library "$CRATE_DIR/target/ios-sim-fat/$PROFILE/lib${CRATE}.a" \
    -headers "$HEADERS_SIM" \
    -output "$XCFRAMEWORK_DIR"

echo ""
echo "Done!"
echo "  XCFramework  → $XCFRAMEWORK_DIR"
echo "  Swift bindings → $SWIFT_SOURCES_DIR/${CRATE}.swift"
echo ""
echo "Open allourthings-ios in Xcode. SPM resolves the local package automatically."
