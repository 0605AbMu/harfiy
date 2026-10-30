$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspaceRoot = (Resolve-Path "$scriptDir\..\..").Path
$jniLibsDir = Join-Path $workspaceRoot "mobile\android\app\src\main\jniLibs"

Write-Host "==> 🤖 Harfiy Android JNI kutubxonalarini yig'ish..." -ForegroundColor Cyan

# cargo-ndk mavjudligini tekshirish
if (-not (Get-Command cargo-ndk -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-ndk topilmadi. O'rnatilmoqda..." -ForegroundColor Yellow
    cargo install cargo-ndk
}

# Rust Android targetlarini tekshirish / qo'shish
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

if (-not (Test-Path $jniLibsDir)) {
    New-Item -ItemType Directory -Force -Path $jniLibsDir | Out-Null
}

Write-Host "==> Android ABI lar uchun kompilyatsiya qilinmoqda (arm64-v8a, armeabi-v7a, x86_64)..." -ForegroundColor Cyan
Push-Location $workspaceRoot
try {
    cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$jniLibsDir" build --release --package harfiy-ffi
} finally {
    Pop-Location
}

Write-Host "==> ✅ Android jniLibs muvaffaqiyatli tayyorlandi:" -ForegroundColor Green
Get-ChildItem -Recurse $jniLibsDir | Select-Object FullName, Length
