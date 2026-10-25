#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Harfiy Mobile FFI Bridge Test Runner
# Verifies C-ABI, JNI, and FSM logic integrity for mobile consumers.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "==> 🧪 Harfiy Mobile FFI testlari ishga tushirilmoqda..."
cd "$WORKSPACE_ROOT"

# 1. Unit testlar
cargo test -p harfiy-ffi

echo ""
echo "==> ✅ Barcha mobil FFI testlari muvaffaqiyatli o'tdi!"
echo "    Interaktiv simulyatorni ishga tushirish uchun:"
echo "    cargo run --example mobile_simulator -p harfiy-ffi"
