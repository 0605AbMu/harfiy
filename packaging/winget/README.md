# Harfiy uchun Windows Package Manager (WinGet) Qo‘llanmasi

Ushbu bo‘lim Harfiy dasturini rasmiy **Windows Package Manager (`winget`)** tizimida e’lon qilish (publish) va kelgusidagi yangilanishlarni avtomatlashtirish uchun mo‘ljallangan.

---

## ⚡ Foydalanuvchilar qanday o‘rnatadi?

Foydalanuvchilar Windows terminalida quyidagicha oson o‘rnatishlari mumkin:

```cmd
winget install harfiy
```
yoki:
```cmd
winget install Harfiy
```
*(yoki to‘liq ID orqali: `winget install AbdumannonShamsiyev.Harfiy`)*

> **Izoh:** Manifest ichida `PackageName: Harfiy` va `Moniker: harfiy` deb ko‘rsatilgani tufayli, WinGet qidiruvda darhol Harfiy dasturini topadi va to‘g‘ridan-to‘g‘ri o‘rnatadi. Foydalanuvchi uzun ID yozishi shart emas!

---

## 📋 Qanday ishlaydi? (Arxitektura)

WinGet paketlari Microsoft tomonidan boshqariladigan [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) ochiq repozitoriyasida saqlanadi. Harfiy dasturini WinGet'ga joylash ikki bosqichdan iborat:

- **Paket identifikatori:** `AbdumannonShamsiyev.Harfiy`  
- **1-bosqich: Dastlabki ro‘yxatdan o‘tkazish (Initial PR - Bir martalik ish):**  
  Yangi ilova WinGet bazasida paydo bo‘lishi uchun dastlabki manifest fayllari `microsoft/winget-pkgs` repozitoriyasiga Pull Request (PR) sifatida yuboriladi va Microsoft boti hamda moderatorlari tomonidan tasdiqlanadi.
- **2-bosqich: Avtomatik yangilanishlar (Continuous Releases):**  
  Paket bir marta qabul qilingach, GitHub Actions workflow'i har safar yangi release chiqqanda o‘zi avtomatik ravishda `microsoft/winget-pkgs` ga PR yuboradi.

---

## 🚀 1-BOSQICH: Dastlabki manifestni yuborish (v0.8.0)

Ushbu repozitoriyda tayyor v0.8.0 manifestlari shakllantirilgan:
`packaging/winget/manifests/a/AbdumannonShamsiyev/Harfiy/0.8.0/`

PR ochish uchun quyidagi 2 ta usuldan birini tanlang:

### Usul A: GitHub Web interfeysi orqali (Eng oson, OS talab qilmaydi)

1. Brauzerda [github.com/microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) sahifasiga kiring va yuqori o‘ng burchakdagi **Fork** tugmasini bosing.
2. Fork qilingan repozitoriyingizda quyidagi papka yo‘nalishini oching:  
   `manifests/a/AbdumannonShamsiyev/Harfiy/0.8.0/`
3. Yangi papka ichiga 3 ta manifest faylini joylashtiring:
   - `AbdumannonShamsiyev.Harfiy.yaml`
   - `AbdumannonShamsiyev.Harfiy.installer.yaml`
   - `AbdumannonShamsiyev.Harfiy.locale.en-US.yaml`
   *(Ushbu fayllarning tayyor mazmuni loyihangizdagi `packaging/winget/manifests/a/AbdumannonShamsiyev/Harfiy/0.8.0/` papkasida mavjud)*.
4. O‘zgarishlarni commit qiling:
   - Commit xabari: `New package: AbdumannonShamsiyev.Harfiy version 0.8.0`
5. `microsoft/winget-pkgs` repozitoriyasiga **Pull Request** oching.
6. Microsoft boti bir necha daqiqa ichida avtomatik testlarni yurgizadi va PR tasdiqlangach (merge bo‘lgach), dastur WinGet katalogiga qo‘shiladi!

---

### Usul B: Windows'da `wingetcreate` vositasi orqali (Rasmiy CLI)

Agar Windows tizimida bo‘lsangiz:

1. `wingetcreate` vositasini o‘rnating:
   ```powershell
   winget install Microsoft.WingetCreate
   ```
2. Yangi paket yaratish buyrug‘ini bering:
   ```powershell
   wingetcreate new https://github.com/0605AbMu/harfiy/releases/download/0.8.0/harfiy-v0.8.0-windows-x86_64-setup.exe
   ```
3. Terminaldagi ko‘rsatmalarga asosan:
   - PackageIdentifier: `AbdumannonShamsiyev.Harfiy`
   - PackageName: `Harfiy`
   - Publisher: `Abdumannon Shamsiyev`
   - Moniker: `harfiy`
   - License: `MIT`
4. Oxirida `--submit` opsiyasi orqali to‘g‘ridan-to‘g‘ri PR ochadi (GitHub hisobingiz orqali login so‘raydi).

---

## ⚡ 2-BOSQICH: Avtomatlashtirishni sozlash (Kelgusi yangilanishlar uchun)

Harfiy repozitoriyasiga quyidagi workflow fayllari qo‘shilgan:
- `.github/workflows/winget.yml` (Alohida WinGet releaser workflow)
- `.github/workflows/release.yml` (Asosiy release workflow ichida `publish-winget` qadami)

Ushbu workflow har safar yangi reliz chiqqanda avtomatik `microsoft/winget-pkgs` ga PR ochishi uchun:

### `WINGET_TOKEN` Secret yaratish

1. GitHub profilingizga kiring: [Personal Access Tokens (classic)](https://github.com/settings/tokens/new)
2. Yangi token yarating:
   - **Note**: `Harfiy WinGet Releaser`
   - **Expiration**: `No expiration` (yoki 1 yil)
   - **Scopes**:
     - `[x] public_repo` (Ochiq repozitoriyalar uchun PR ochish huquqi)
3. Token kodini nusxalang (`ghp_...`).
4. **Harfiy** repozitoriyasiga o‘ting:
   - **Settings** -> **Secrets and variables** -> **Actions** -> **New repository secret**.
   - **Name:** `WINGET_TOKEN`
   - **Secret:** *(Nusxalangan tokenni joylang)*
   - **Add secret** tugmasini bosing.
