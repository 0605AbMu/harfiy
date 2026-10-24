#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Harfiy WinGet Manifest Generator
# Usage: ./scripts/generate-winget-manifest.sh <version> [installer_url] [sha256]
# ==============================================================================

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "Xatolik: Versiya ko'rsatilmadi!"
  echo "Foydalanish: $0 <version> [installer_url] [sha256]"
  echo "Misol: $0 0.8.0"
  exit 1
fi

CLEAN_VERSION="${VERSION#v}"
REPO="0605AbMu/harfiy"
PACKAGE_ID="AbdumannonShamsiyev.Harfiy"
INSTALLER_URL="${2:-https://github.com/${REPO}/releases/download/${CLEAN_VERSION}/harfiy-v${CLEAN_VERSION}-windows-x86_64-setup.exe}"

echo "==> Harfiy WinGet Manifestlarini yaratish: v${CLEAN_VERSION}"
echo "    PackageIdentifier: ${PACKAGE_ID}"
echo "    Installer URL:     ${INSTALLER_URL}"

# SHA256 heshtini aniqlash
SHA256="${3:-}"
if [ -z "$SHA256" ]; then
  echo "==> Installer faylining SHA256 heshi hisoblanmoqda..."
  SHA256=$(curl -sL "$INSTALLER_URL" | shasum -a 256 | awk '{print toupper($1)}')
  if [ -z "$SHA256" ] || [ ${#SHA256} -ne 64 ]; then
    echo "Xatolik: SHA256 heshini aniqlab bo'lmadi. URL to'g'riligini tekshiring: $INSTALLER_URL"
    exit 1
  fi
fi
echo "    SHA256:            ${SHA256}"

FIRST_LETTER=$(echo "${PACKAGE_ID:0:1}" | tr '[:upper:]' '[:lower:]')
PUBLISHER="${PACKAGE_ID%%.*}"
PACKAGE_NAME="${PACKAGE_ID##*.}"
OUT_DIR="packaging/winget/manifests/${FIRST_LETTER}/${PUBLISHER}/${PACKAGE_NAME}/${CLEAN_VERSION}"

mkdir -p "$OUT_DIR"

# 1. Version manifest
cat <<EOF > "${OUT_DIR}/${PACKAGE_ID}.yaml"
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.9.0.schema.json

PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${CLEAN_VERSION}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.9.0
EOF

# 2. Installer manifest
cat <<EOF > "${OUT_DIR}/${PACKAGE_ID}.installer.yaml"
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.9.0.schema.json

PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${CLEAN_VERSION}
Platform:
  - Windows.Desktop
MinimumOSVersion: 10.0.0.0
InstallerType: inno
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install
ProductCode: '{8B45462D-8D59-4E87-BA3F-C4D8A9F1A4E7}_is1'
Installers:
  - Architecture: x64
    InstallerUrl: ${INSTALLER_URL}
    InstallerSha256: ${SHA256}
ManifestType: installer
ManifestVersion: 1.9.0
EOF

# 3. Locale manifest
cat <<EOF > "${OUT_DIR}/${PACKAGE_ID}.locale.en-US.yaml"
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.1.9.0.schema.json

PackageIdentifier: ${PACKAGE_ID}
PackageVersion: ${CLEAN_VERSION}
PackageLocale: en-US
Publisher: Abdumannon Shamsiyev
PublisherUrl: https://abdumannon-shamsiyev.uz/harfiy
PublisherSupportUrl: https://t.me/Abdumannon_Shamsiyev
Author: Abdumannon Shamsiyev
PackageName: Harfiy
PackageUrl: https://github.com/${REPO}
License: MIT
LicenseUrl: https://raw.githubusercontent.com/${REPO}/master/LICENSE
Copyright: Copyright (c) 2026 Abdumannon Shamsiyev
ShortDescription: Real-time Uzbek Latin alphabet auto-converter background daemon
Description: |-
  Harfiy is an ultra-fast, zero-latency background service written in Rust that converts Uzbek Latin digraphs (sh, ch, o', g') into their official unified graphemes (ş, ç, ö, ğ) in real-time across all desktop applications.
Moniker: harfiy
Tags:
  - uzbek
  - alphabet
  - keyboard
  - typing
  - converter
  - utility
  - rust
ReleaseNotesUrl: https://github.com/${REPO}/releases/tag/${CLEAN_VERSION}
ManifestType: defaultLocale
ManifestVersion: 1.9.0
EOF

echo "==> Muvaffaqiyatli yaratildi: ${OUT_DIR}/"
ls -la "${OUT_DIR}/"
