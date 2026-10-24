#!/usr/bin/env bash
set -e

VERSION="${1:-0.6.0}"
TARGET_DIR="${2:-target/release}"
OUT_DIR="${3:-dist}"
ARCH="${4:-universal}"

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${DIR}/../.." && pwd)"

echo "==> Building macOS Installer Wizard (.pkg) for Harfiy v${VERSION} (${ARCH})..."

# 1. Create Harfiy.app bundle structure
STAGING_DIR="$(mktemp -d /tmp/harfiy_pkg_staging.XXXXXX)"
APP_BUNDLE="${STAGING_DIR}/Harfiy.app"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

cp "${TARGET_DIR}/harfiy" "${APP_BUNDLE}/Contents/MacOS/"
chmod +x "${APP_BUNDLE}/Contents/MacOS/harfiy"
cp "${DIR}/Info.plist" "${APP_BUNDLE}/Contents/"

if [ -x "/usr/libexec/PlistBuddy" ]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString ${VERSION}" "${APP_BUNDLE}/Contents/Info.plist" 2>/dev/null || true
fi

# Generate icns if iconutil is available
if command -v iconutil >/dev/null 2>&1 && [ -f "${PROJECT_ROOT}/assets/app_icon_1024.png" ]; then
    ICONSET="${STAGING_DIR}/AppIcon.iconset"
    mkdir -p "${ICONSET}"
    sips -z 16 16     "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_16x16.png" >/dev/null
    sips -z 32 32     "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_16x16@2x.png" >/dev/null
    sips -z 32 32     "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_32x32.png" >/dev/null
    sips -z 64 64     "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_32x32@2x.png" >/dev/null
    sips -z 128 128   "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_128x128.png" >/dev/null
    sips -z 256 256   "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_128x128@2x.png" >/dev/null
    sips -z 256 256   "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_256x256.png" >/dev/null
    sips -z 512 512   "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_256x256@2x.png" >/dev/null
    sips -z 512 512   "${PROJECT_ROOT}/assets/app_icon_1024.png" --out "${ICONSET}/icon_512x512.png" >/dev/null
    cp "${PROJECT_ROOT}/assets/app_icon_1024.png" "${ICONSET}/icon_512x512@2x.png"
    iconutil -c icns "${ICONSET}" -o "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
fi

# Code-sign Harfiy.app
if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep -s - -r='designated => identifier "uz.harfiy.app"' "${APP_BUNDLE}" 2>/dev/null || true
fi

# 2. Build component package with preinstall and postinstall scripts
COMPONENT_PKG="${STAGING_DIR}/harfiy_component.pkg"
pkgbuild \
    --component "${APP_BUNDLE}" \
    --install-location "/Applications" \
    --scripts "${DIR}/scripts" \
    --identifier "uz.harfiy.app" \
    --version "${VERSION}" \
    "${COMPONENT_PKG}"

# 3. Generate Distribution.xml with Uzbek welcome and conclusion wizard pages
DISTRIBUTION_XML="${STAGING_DIR}/Distribution.xml"
cat <<EOF > "${DISTRIBUTION_XML}"
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
    <title>Harfiy</title>
    <welcome file="welcome.html" mime-type="text/html"/>
    <conclusion file="conclusion.html" mime-type="text/html"/>
    <domains enable_anywhere="false" enable_currentUserHome="false" enable_localSystem="true"/>
    <options customize="never" require-scripts="false" hostArchitectures="x86_64,arm64"/>
    <choices-outline>
        <line choice="default">
            <line choice="uz.harfiy.app"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="uz.harfiy.app" visible="false">
        <pkg-ref id="uz.harfiy.app"/>
    </choice>
    <pkg-ref id="uz.harfiy.app" version="${VERSION}" onConclusion="none">harfiy_component.pkg</pkg-ref>
</installer-gui-script>
EOF

# 4. Synthesize final distribution product package
mkdir -p "${OUT_DIR}"
PKG_NAME="harfiy-v${VERSION}-macos-${ARCH}.pkg"
FINAL_PKG="${OUT_DIR}/${PKG_NAME}"

productbuild \
    --distribution "${DISTRIBUTION_XML}" \
    --package-path "${STAGING_DIR}" \
    --resources "${DIR}/resources" \
    "${FINAL_PKG}"

# Optional ad-hoc codesign for the product package
if command -v codesign >/dev/null 2>&1; then
    codesign --force -s - "${FINAL_PKG}" 2>/dev/null || true
fi

rm -rf "${STAGING_DIR}"
echo "==> Successfully created macOS Installer Wizard: ${FINAL_PKG}"
