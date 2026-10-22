#!/usr/bin/env bash
set -e

VERSION="${1:-0.1.0}"
TARGET_DIR="${2:-target/release}"
OUT_DIR="${3:-dist}"
ARCH="amd64"

PKG_DIR="harfiy_${VERSION}_${ARCH}"
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN"
mkdir -p "${PKG_DIR}/usr/bin"
mkdir -p "${PKG_DIR}/usr/share/applications"
mkdir -p "${PKG_DIR}/usr/lib/systemd/user"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/16x16/apps"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/32x32/apps"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/64x64/apps"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/128x128/apps"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/256x256/apps"
mkdir -p "${PKG_DIR}/usr/share/doc/harfiy"

# Binary
cp "${TARGET_DIR}/harfiy" "${PKG_DIR}/usr/bin/"
chmod 755 "${PKG_DIR}/usr/bin/harfiy"

# Desktop file and service
cp packaging/linux/harfiy.desktop "${PKG_DIR}/usr/share/applications/"
cp packaging/linux/harfiy.service "${PKG_DIR}/usr/lib/systemd/user/"

# Icons
cp assets/logo_16.png "${PKG_DIR}/usr/share/icons/hicolor/16x16/apps/harfiy.png"
cp assets/logo_32.png "${PKG_DIR}/usr/share/icons/hicolor/32x32/apps/harfiy.png"
cp assets/logo_64.png "${PKG_DIR}/usr/share/icons/hicolor/64x64/apps/harfiy.png"
cp assets/logo_128.png "${PKG_DIR}/usr/share/icons/hicolor/128x128/apps/harfiy.png"
cp assets/logo_256.png "${PKG_DIR}/usr/share/icons/hicolor/256x256/apps/harfiy.png"

# Documentation
cp README.md "${PKG_DIR}/usr/share/doc/harfiy/"

# DEBIAN/control
cat <<EOF > "${PKG_DIR}/DEBIAN/control"
Package: harfiy
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Abdumannon Shamsiyev <https://t.me/Abdumannon_Shamsiyev>
Description: Real-time Uzbek Latin orthography conversion engine
 Harfiy automatically converts Uzbek Latin letter combinations (sh, ch, o', g')
 into modern unified graphemes in real-time at system level.
EOF

mkdir -p "${OUT_DIR}"
dpkg-deb --build --root-owner-group "${PKG_DIR}" "${OUT_DIR}/harfiy_${VERSION}_${ARCH}.deb"
rm -rf "${PKG_DIR}"
echo "Built ${OUT_DIR}/harfiy_${VERSION}_${ARCH}.deb successfully!"
