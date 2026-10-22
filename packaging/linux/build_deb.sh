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

# 1. Binary
cp "${TARGET_DIR}/harfiy" "${PKG_DIR}/usr/bin/"
chmod 755 "${PKG_DIR}/usr/bin/harfiy"

# 2. Desktop file and systemd service
cp packaging/linux/harfiy.desktop "${PKG_DIR}/usr/share/applications/"
cp packaging/linux/harfiy.service "${PKG_DIR}/usr/lib/systemd/user/"

# 3. Icons
cp assets/logo_16.png "${PKG_DIR}/usr/share/icons/hicolor/16x16/apps/harfiy.png"
cp assets/logo_32.png "${PKG_DIR}/usr/share/icons/hicolor/32x32/apps/harfiy.png"
cp assets/logo_64.png "${PKG_DIR}/usr/share/icons/hicolor/64x64/apps/harfiy.png"
cp assets/logo_128.png "${PKG_DIR}/usr/share/icons/hicolor/128x128/apps/harfiy.png"
cp assets/logo_256.png "${PKG_DIR}/usr/share/icons/hicolor/256x256/apps/harfiy.png"

# 4. Documentation
cp README.md "${PKG_DIR}/usr/share/doc/harfiy/"

# 5. DEBIAN/control with upgrade replacement declarations
cat <<EOF > "${PKG_DIR}/DEBIAN/control"
Package: harfiy
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Abdumannon Shamsiyev <https://t.me/Abdumannon_Shamsiyev>
Replaces: harfiy (<< ${VERSION})
Provides: harfiy
Description: Real-time Uzbek Latin orthography conversion engine
 Harfiy automatically converts Uzbek Latin letter combinations (sh, ch, o', g')
 into modern unified graphemes in real-time at system level.
EOF

# 6. DEBIAN/prerm (executed before upgrade/removal to stop running instances cleanly)
cat <<'EOF' > "${PKG_DIR}/DEBIAN/prerm"
#!/bin/sh
set -e
if [ "$1" = "upgrade" ] || [ "$1" = "remove" ] || [ "$1" = "deconfigure" ]; then
    # Stop running process gracefully
    pkill -TERM -x harfiy || true
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --global stop harfiy.service 2>/dev/null || true
    fi
fi
exit 0
EOF
chmod 755 "${PKG_DIR}/DEBIAN/prerm"

# 7. DEBIAN/postinst (executed after upgrade/install to update caches)
cat <<'EOF' > "${PKG_DIR}/DEBIAN/postinst"
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
    fi
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --global daemon-reload 2>/dev/null || true
    fi
fi
exit 0
EOF
chmod 755 "${PKG_DIR}/DEBIAN/postinst"

# 8. DEBIAN/postrm (executed after package removal or purge)
cat <<'EOF' > "${PKG_DIR}/DEBIAN/postrm"
#!/bin/sh
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
    fi
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --global daemon-reload 2>/dev/null || true
    fi
fi
exit 0
EOF
chmod 755 "${PKG_DIR}/DEBIAN/postrm"

# 9. Build Debian package
mkdir -p "${OUT_DIR}"
dpkg-deb --build --root-owner-group "${PKG_DIR}" "${OUT_DIR}/harfiy_${VERSION}_${ARCH}.deb"
rm -rf "${PKG_DIR}"
echo "Built ${OUT_DIR}/harfiy_${VERSION}_${ARCH}.deb successfully!"
