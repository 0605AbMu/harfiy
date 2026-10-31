#!/usr/bin/env bash
set -euo pipefail

# Build WebAssembly package for Harfiy Engine (Web & SPA)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATE_DIR="${REPO_ROOT}/crates/wasm"
PKG_DIR="${CRATE_DIR}/pkg"

echo "=== Building Harfiy WebAssembly Core (@harfiy/wasm) ==="

cd "${REPO_ROOT}"

# Copy LICENSE if available
if [ -f "${REPO_ROOT}/LICENSE" ]; then
  cp "${REPO_ROOT}/LICENSE" "${CRATE_DIR}/LICENSE"
fi

# Build wasm package using wasm-pack
npx -y wasm-pack build "${CRATE_DIR}" --target web --out-dir "${PKG_DIR}"

# Copy high-level DOM adapter and TypeScript definitions into pkg
cp "${CRATE_DIR}/adapter.js" "${PKG_DIR}/adapter.js"
cp "${CRATE_DIR}/adapter.d.ts" "${PKG_DIR}/adapter.d.ts"
cp "${CRATE_DIR}/README.md" "${PKG_DIR}/README.md"

# Configure package.json for GitHub Packages (@0605abmu/harfiy-wasm)
node -e "
const fs = require('fs');
const pkgPath = '${PKG_DIR}/package.json';
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));

pkg.name = '@0605abmu/harfiy-wasm';
pkg.publishConfig = {
  registry: 'https://npm.pkg.github.com'
};
pkg.files = [
  'harfiy_wasm_bg.wasm',
  'harfiy_wasm.js',
  'harfiy_wasm.d.ts',
  'harfiy_wasm_bg.wasm.d.ts',
  'adapter.js',
  'adapter.d.ts',
  'README.md',
  'LICENSE'
];
pkg.exports = {
  '.': {
    types: './harfiy_wasm.d.ts',
    import: './harfiy_wasm.js'
  },
  './adapter': {
    types: './adapter.d.ts',
    import: './adapter.js'
  }
};

fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
"

echo "=== Build Complete ==="
echo "Output files in: ${PKG_DIR}"
ls -lh "${PKG_DIR}"
