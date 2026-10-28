#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Harfiy iOS XCFramework Builder
# Compiles harfiy-ffi for device (aarch64) and simulator (aarch64-sim),
# packaging them into a universal HarfiyCore.xcframework.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$WORKSPACE_ROOT/mobile/ios/Frameworks"
XCFRAMEWORK_PATH="$OUTPUT_DIR/HarfiyCore.xcframework"

echo "==> 🍏 Harfiy iOS XCFramework yig'ish boshlandi..."

# Rust'ning minimal iOS versiyasini Xcode loyihasi bilan moslashtirish
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"

# 1. Targetlarni tekshirish va o'rnatish
echo "==> Rust iOS targetlarini tekshirish..."
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# 2. Qurilma (Device arm64) uchun yig'ish
echo "==> 1/2: aarch64-apple-ios (iPhone jismoniy qurilmasi) uchun kompilyatsiya..."
cargo rustc -p harfiy-ffi --target aarch64-apple-ios --crate-type staticlib --profile ios

# 3. Simulyator (Simulator arm64) uchun yig'ish
echo "==> 2/2: aarch64-apple-ios-sim (Apple Silicon Simulator) uchun kompilyatsiya..."
cargo rustc -p harfiy-ffi --target aarch64-apple-ios-sim --crate-type staticlib --profile ios

# 4. Eski XCFrameworkni tozalash va papkalar yaratish
mkdir -p "$OUTPUT_DIR"
rm -rf "$XCFRAMEWORK_PATH"

mkdir -p "$XCFRAMEWORK_PATH/ios-arm64/Headers"
mkdir -p "$XCFRAMEWORK_PATH/ios-arm64-simulator/Headers"

# 5. Fayllarni ko'chirish
cp "$WORKSPACE_ROOT/target/aarch64-apple-ios/ios/libharfiy_ffi.a" "$XCFRAMEWORK_PATH/ios-arm64/libharfiy_ffi.a"
cp "$WORKSPACE_ROOT/crates/ffi/include/harfiy.h" "$XCFRAMEWORK_PATH/ios-arm64/Headers/"
cp "$WORKSPACE_ROOT/crates/ffi/include/module.modulemap" "$XCFRAMEWORK_PATH/ios-arm64/Headers/"

cp "$WORKSPACE_ROOT/target/aarch64-apple-ios-sim/ios/libharfiy_ffi.a" "$XCFRAMEWORK_PATH/ios-arm64-simulator/libharfiy_ffi.a"
cp "$WORKSPACE_ROOT/crates/ffi/include/harfiy.h" "$XCFRAMEWORK_PATH/ios-arm64-simulator/Headers/"
cp "$WORKSPACE_ROOT/crates/ffi/include/module.modulemap" "$XCFRAMEWORK_PATH/ios-arm64-simulator/Headers/"

# 6. XCFramework Info.plist yaratish
cat << 'EOF' > "$XCFRAMEWORK_PATH/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AvailableLibraries</key>
	<array>
		<dict>
			<key>BinaryPath</key>
			<string>libharfiy_ffi.a</string>
			<key>HeadersPath</key>
			<string>Headers</string>
			<key>LibraryIdentifier</key>
			<string>ios-arm64</string>
			<key>LibraryPath</key>
			<string>libharfiy_ffi.a</string>
			<key>SupportedArchitectures</key>
			<array>
				<string>arm64</string>
			</array>
			<key>SupportedPlatform</key>
			<string>ios</string>
		</dict>
		<dict>
			<key>BinaryPath</key>
			<string>libharfiy_ffi.a</string>
			<key>HeadersPath</key>
			<string>Headers</string>
			<key>LibraryIdentifier</key>
			<string>ios-arm64-simulator</string>
			<key>LibraryPath</key>
			<string>libharfiy_ffi.a</string>
			<key>SupportedArchitectures</key>
			<array>
				<string>arm64</string>
			</array>
			<key>SupportedPlatform</key>
			<string>ios</string>
			<key>SupportedPlatformVariant</key>
			<string>simulator</string>
		</dict>
	</array>
	<key>CFBundlePackageType</key>
	<string>XFWK</string>
	<key>XCFrameworkFormatVersion</key>
	<string>1.0</string>
</dict>
</plist>
EOF

echo "==> ✅ Muvaffaqiyatli yakunlandi: $XCFRAMEWORK_PATH"
ls -la "$XCFRAMEWORK_PATH"
