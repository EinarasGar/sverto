#!/usr/bin/env bash
# Cross-compiles the Rust core into android/app/src/main/jniLibs and generates the UniFFI Kotlin bindings.
# Usage: build-core.sh [abi...]   (defaults to every ABI the app ships)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORE_DIR="$REPO_ROOT/server/app-core"
TARGET_DIR="$REPO_ROOT/server/target"
JNI_LIBS="$REPO_ROOT/android/app/src/main/jniLibs"
BINDINGS="$REPO_ROOT/android/app/src/main/java"
# Prefer rustup's cargo: Xcode and Gradle run this with a PATH that lacks ~/.cargo/bin. CI images
# can ship rustup elsewhere (Homebrew), so fall back to whatever cargo is on PATH.
CARGO="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
[ -x "$CARGO" ] || CARGO="$(command -v cargo)" || { echo "cargo not found" >&2; exit 127; }

ABIS=("$@")
[ ${#ABIS[@]} -gt 0 ] || ABIS=(arm64-v8a armeabi-v7a x86_64 x86)

rust_target() {
  case "$1" in
    arm64-v8a) echo aarch64-linux-android ;;
    armeabi-v7a) echo armv7-linux-androideabi ;;
    x86_64) echo x86_64-linux-android ;;
    x86) echo i686-linux-android ;;
    *) echo "unknown ABI: $1" >&2; exit 2 ;;
  esac
}

for abi in "${ABIS[@]}"; do
  target="$(rust_target "$abi")"
  echo "Building sverto-core for $target"
  (cd "$CORE_DIR" && "$CARGO" ndk --target "$target" --platform 24 -- rustc --release --lib -- \
    -C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384)
  mkdir -p "$JNI_LIBS/$abi"
  cp "$TARGET_DIR/$target/release/libsverto_core.so" "$JNI_LIBS/$abi/libsverto_core.so"
done

echo "Generating Kotlin bindings"
(cd "$CORE_DIR" && "$CARGO" run --release --bin uniffi-bindgen -- generate \
  --library "$TARGET_DIR/$(rust_target "${ABIS[0]}")/release/libsverto_core.so" \
  --language kotlin --out-dir "$BINDINGS")
