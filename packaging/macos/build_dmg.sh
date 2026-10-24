#!/usr/bin/env bash
set -e

VERSION="${1:-0.1.0}"
TARGET_DIR="${2:-target/release}"
OUT_DIR="${3:-dist}"
ARCH="${4:-universal}"

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. Build macOS Installer Wizard (.pkg)
"${DIR}/build_pkg.sh" "${VERSION}" "${TARGET_DIR}" "${OUT_DIR}" "${ARCH}"
PKG_NAME="harfiy-v${VERSION}-macos-${ARCH}.pkg"

APP_BUNDLE="Harfiy.app"
rm -rf "${APP_BUNDLE}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

cp "${TARGET_DIR}/harfiy" "${APP_BUNDLE}/Contents/MacOS/"
chmod +x "${APP_BUNDLE}/Contents/MacOS/harfiy"

cp packaging/macos/Info.plist "${APP_BUNDLE}/Contents/"
if [ -x "/usr/libexec/PlistBuddy" ]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString ${VERSION}" "${APP_BUNDLE}/Contents/Info.plist" 2>/dev/null || true
else
    sed -i '' -E "s/<string>[0-9]+\.[0-9]+\.[0-9]+<\/string>/<string>${VERSION}<\/string>/g" "${APP_BUNDLE}/Contents/Info.plist"
fi

# Generate icns if iconutil is available on macOS
if command -v iconutil >/dev/null 2>&1; then
    ICONSET="AppIcon.iconset"
    mkdir -p "${ICONSET}"
    sips -z 16 16     assets/app_icon_1024.png --out "${ICONSET}/icon_16x16.png" >/dev/null
    sips -z 32 32     assets/app_icon_1024.png --out "${ICONSET}/icon_16x16@2x.png" >/dev/null
    sips -z 32 32     assets/app_icon_1024.png --out "${ICONSET}/icon_32x32.png" >/dev/null
    sips -z 64 64     assets/app_icon_1024.png --out "${ICONSET}/icon_32x32@2x.png" >/dev/null
    sips -z 128 128   assets/app_icon_1024.png --out "${ICONSET}/icon_128x128.png" >/dev/null
    sips -z 256 256   assets/app_icon_1024.png --out "${ICONSET}/icon_128x128@2x.png" >/dev/null
    sips -z 256 256   assets/app_icon_1024.png --out "${ICONSET}/icon_256x256.png" >/dev/null
    sips -z 512 512   assets/app_icon_1024.png --out "${ICONSET}/icon_256x256@2x.png" >/dev/null
    sips -z 512 512   assets/app_icon_1024.png --out "${ICONSET}/icon_512x512.png" >/dev/null
    cp assets/app_icon_1024.png "${ICONSET}/icon_512x512@2x.png"
    iconutil -c icns "${ICONSET}" -o "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
    rm -rf "${ICONSET}"
fi

# Code-sign the app bundle (ad-hoc signing with stable identifier requirement)
if command -v codesign >/dev/null 2>&1; then
    echo "Signing ${APP_BUNDLE}..."
    codesign --force --deep -s - -r='designated => identifier "uz.harfiy.app"' "${APP_BUNDLE}"
fi

# Prepare DMG staging folder
DMG_STAGING="dmg_staging"
rm -rf "${DMG_STAGING}"
mkdir -p "${DMG_STAGING}"
cp -r "${APP_BUNDLE}" "${DMG_STAGING}/"
ln -s /Applications "${DMG_STAGING}/Applications"

if [ -f "${OUT_DIR}/${PKG_NAME}" ]; then
    cp "${OUT_DIR}/${PKG_NAME}" "${DMG_STAGING}/Harfiy O'rnatish (Installer).pkg"
fi

mkdir -p "${OUT_DIR}"
DMG_NAME="harfiy-v${VERSION}-macos-${ARCH}.dmg"
hdiutil create -volname "Harfiy" -srcfolder "${DMG_STAGING}" -ov -format UDZO "${OUT_DIR}/${DMG_NAME}"

# Code-sign the DMG if codesign is available
if command -v codesign >/dev/null 2>&1; then
    codesign --force -s - "${OUT_DIR}/${DMG_NAME}" 2>/dev/null || true
fi

rm -rf "${DMG_STAGING}" "${APP_BUNDLE}"
echo "Built ${OUT_DIR}/${DMG_NAME} successfully!"
