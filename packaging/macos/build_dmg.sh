#!/usr/bin/env bash
set -e

VERSION="${1:-0.1.0}"
TARGET_DIR="${2:-target/release}"
OUT_DIR="${3:-dist}"
ARCH="${4:-universal}"

APP_BUNDLE="Harfiy.app"
rm -rf "${APP_BUNDLE}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

cp "${TARGET_DIR}/harfiy" "${APP_BUNDLE}/Contents/MacOS/"
chmod +x "${APP_BUNDLE}/Contents/MacOS/harfiy"

cp packaging/macos/Info.plist "${APP_BUNDLE}/Contents/"
sed -i '' "s/0.1.0/${VERSION}/g" "${APP_BUNDLE}/Contents/Info.plist"

# Generate icns if iconutil is available on macOS
if command -v iconutil >/dev/null 2>&1; then
    ICONSET="AppIcon.iconset"
    mkdir -p "${ICONSET}"
    cp assets/logo_16.png "${ICONSET}/icon_16x16.png"
    cp assets/logo_32.png "${ICONSET}/icon_16x16@2x.png"
    cp assets/logo_32.png "${ICONSET}/icon_32x32.png"
    cp assets/logo_64.png "${ICONSET}/icon_32x32@2x.png"
    cp assets/logo_128.png "${ICONSET}/icon_128x128.png"
    cp assets/logo_256.png "${ICONSET}/icon_128x128@2x.png"
    cp assets/logo_256.png "${ICONSET}/icon_256x256.png"
    iconutil -c icns "${ICONSET}" -o "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
    rm -rf "${ICONSET}"
fi

# Prepare DMG staging folder
DMG_STAGING="dmg_staging"
rm -rf "${DMG_STAGING}"
mkdir -p "${DMG_STAGING}"
cp -r "${APP_BUNDLE}" "${DMG_STAGING}/"
ln -s /Applications "${DMG_STAGING}/Applications"

mkdir -p "${OUT_DIR}"
DMG_NAME="harfiy-v${VERSION}-macos-${ARCH}.dmg"
hdiutil create -volname "Harfiy" -srcfolder "${DMG_STAGING}" -ov -format UDZO "${OUT_DIR}/${DMG_NAME}"
rm -rf "${DMG_STAGING}" "${APP_BUNDLE}"
echo "Built ${OUT_DIR}/${DMG_NAME} successfully!"
