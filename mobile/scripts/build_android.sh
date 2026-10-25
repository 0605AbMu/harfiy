#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Harfiy Android JNI Shared Library Builder
# Compiles harfiy-ffi for arm64-v8a, armeabi-v7a, and x86_64 into jniLibs.
# Requires: ANDROID_NDK_HOME or ANDROID_HOME and cargo-ndk.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
JNILIBS_DIR="$WORKSPACE_ROOT/mobile/android/app/src/main/jniLibs"

echo "==> 🤖 Harfiy Android JNI kutubxonalarini yig'ish..."

# cargo-ndk borligini tekshirish
if ! command -v cargo-ndk &> /dev/null; then
    echo "cargo-ndk topilmadi. O'rnatilmoqda..."
    cargo install cargo-ndk
fi

# Rust targetlarini qo'shish
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

mkdir -p "$JNILIBS_DIR"

echo "==> Android ABI lar uchun kompilyatsiya qilinmoqda..."
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$JNILIBS_DIR" build --release --package harfiy-ffi

echo "==> ✅ Android jniLibs muvaffaqiyatli tayyorlandi:"
ls -la "$JNILIBS_DIR"
