#!/bin/bash
# Builds Legato's Rust core for iPhone and the simulator, has UniFFI write
# its Swift bindings, and packs both where the app looks for them:
#   ios/LegatoCore/LegatoCoreFFI.xcframework
#   ios/LegatoCore/Sources/LegatoCore/LegatoCore.swift
# Needs a Mac with Xcode and `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PACKAGE="$ROOT/ios/LegatoCore"
WORK="$ROOT/ios/build/core"
TARGETS="aarch64-apple-ios aarch64-apple-ios-sim"
# The C in the core (SQLite, ring) is built for the app's oldest iOS.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"

cd "$ROOT/phone"

# The same crate Android builds as a shared library, here as a static one
# and without speech: transcribe-cpp isn't built for iOS.
for target in $TARGETS; do
  cargo rustc --release -p legato-android --lib --no-default-features \
    --target "$target" --crate-type staticlib -- --print native-static-libs
done

rm -rf "$WORK"
mkdir -p "$WORK/swift" "$WORK/headers"

# UniFFI reads its metadata from the built library. If it can't read the
# static one, a library for this Mac carries the same metadata.
SIM="target/aarch64-apple-ios-sim/release/liblegato_android.a"
if ! cargo run --release -p uniffi-bindgen -- generate --library "$SIM" \
  --language swift --no-format --out-dir "$WORK/swift"; then
  echo "Reading the static library failed; generating from a library built for this Mac."
  cargo build --release -p legato-android --no-default-features
  rm -rf "$WORK/swift" && mkdir -p "$WORK/swift"
  cargo run --release -p uniffi-bindgen -- generate \
    --library target/release/liblegato_android.dylib \
    --language swift --no-format --out-dir "$WORK/swift"
fi
ls -l "$WORK/swift"

cp "$WORK"/swift/*.h "$WORK/headers/"
# A static library's module map has to be called module.modulemap.
cp "$WORK"/swift/*.modulemap "$WORK/headers/module.modulemap"

rm -rf "$PACKAGE/LegatoCoreFFI.xcframework"
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/liblegato_android.a -headers "$WORK/headers" \
  -library target/aarch64-apple-ios-sim/release/liblegato_android.a -headers "$WORK/headers" \
  -output "$PACKAGE/LegatoCoreFFI.xcframework"

cp "$WORK"/swift/*.swift "$PACKAGE/Sources/LegatoCore/LegatoCore.swift"
echo "Built $PACKAGE/LegatoCoreFFI.xcframework and the Swift bindings."
