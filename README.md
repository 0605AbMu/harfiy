# Harfiy: Zamonaviy O‘zbek Alifbosi Avto-Konvertatsiya Tizimi

<p align="center">
  <img src="assets/app_icon_1024.png" alt="Harfiy Logo" width="128" height="128" />
</p>

<p align="center">
  <strong>O‘zbek lotin alifbosidagi harf birikmalarini (<code>sh</code>, <code>ch</code>, <code>o'</code>, <code>g'</code>) real vaqtda yangi unikal grafemalarga (<code>ş</code>, <code>ç</code>, <code>ö</code>, <code>ğ</code>) kechikishsiz almashtiruvchi tizim darajasidagi fon xizmati.</strong>
</p>

<p align="center">
  <a href="https://github.com/0605AbMu/harfiy/releases"><img src="https://img.shields.io/github/v/release/0605AbMu/harfiy?color=blue&label=Versiya" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/Litsenziya-MIT-green.svg" alt="License" /></a>
  <img src="https://img.shields.io/badge/Platforma-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg" alt="Platforms" />
  <img src="https://img.shields.io/badge/Rust-1.80%2B-orange.svg" alt="Rust 1.80+" />
  <img src="https://img.shields.io/badge/Tarmoq-100%25%20Offline-success.svg" alt="Zero Network" />
</p>

🌐 **Rasmiy veb-sahifa:** [abdumannon-shamsiyev.uz/harfiy](https://abdumannon-shamsiyev.uz/harfiy)  
📦 **Yuklab olish (Releases):** [github.com/0605AbMu/harfiy/releases](https://github.com/0605AbMu/harfiy/releases)

---

## 📌 Loyiha Haqida va Muammo

O‘zbekiston Respublikasining yangilangan lotin alifbosi islohotiga ko‘ra, qo‘shharflar (`sh`, `ch`) va belgili harflar (`o'`, `g'`) o‘rniga xalqaro standartlarga mos unikal harflar (`ş`, `ç`, `ö`, `ğ`) joriy etilmoqda. Biroq, zamonaviy kompyuter klaviaturalarida bu harflar uchun maxsus tugmalar mavjud emas.

**Harfiy** ushbu muammoni tizim darajasida hal qiladi:
- Foydalanuvchi odatiy tarzda `sh`, `ch`, `o'`, `g'` deb yozaveradi.
- Harfiy ularni real vaqt rejimida (yozish bilan bir onda, sub-millisecond kechikish bilan) yangi harflarga almashtirib beradi.
- Agar foydalanuvchiga eski juftlik kerak bo‘lsa, darhol `Backspace` bosish orqali asl harflar tiklanadi (**Instant Undo**).
- **100% Offline va Maxfiy:** Hech qanday tarmoq so‘rovlari yo‘q, barcha hisob-kitoblar faqat kompyuteringiz xotirasida ishlaydi.

---

## ✨ Asosiy Imkoniyatlar

- ⚡ **Sub-millisecond Tezlik:** Rust tilida yozilgan deterministik FSM (Finite State Machine) har bir tugmani < 0.05 ms ichida qayta ishlaydi.
- ↩️ **Instant Undo (Tezkor Bekor Qilish):** `sh` -> `ş` ga o'zgargach darhol `Backspace` bosilsa, `ş` o'chib asl `sh` tiklanadi.
- 🔤 **Barcha Apostrof Turlari:** To‘g‘ri, qiyshiq, ochiluvchi/yopiluvchi va bir tirnoqli barcha belgilarni taniydi (`'`, `` ` ``, `‘`, `’`, `ʻ`, `ʼ`, `´`).
- 🎮 **O‘yin va To‘liq Ekran Rejimi (Game Mode):** To‘liq ekranli o‘yinlar yoki maxsus dasturlar ochilganda klaviatura ushlab qolinmaydi.
- 🚫 **Istisno Dasturlar (Excluded Apps):** Parol kiritish yoki maxsus kod muharrirlarini konvertatsiyadan istisno qilish imkoniyati.
- 🎛️ **Tezkor Boshqaruv:** `Ctrl + Shift + U` orqali pauza/faollashtirish, `Ctrl + Alt + U` orqali belgilangan matnni bir zumda o‘girish.
- 🪶 **Yengil Vazn:** Xotira sarfi < 8 MB RAM, fayl hajmi atigi ~1.1 MB.

---

## ⚙️ Qoidalar Matrisasi

| Kiritish | Natija | Unicode | Izoh |
| :--- | :--- | :--- | :--- |
| `s` + `h` / `sH` | `ş` | `U+015F` | Kichik harf |
| `S` + `h` / `SH` | `Ş` | `U+015E` | Bosh harf |
| `c` + `h` / `cH` | `ç` | `U+00E7` | Kichik harf |
| `C` + `h` / `CH` | `Ç` | `U+00C7` | Bosh harf |
| `o` + `'` / `` ` `` / `‘` / `’` / `ʻ` / `ʼ` / `´` | `ö` | `U+00F6` | Barcha apostrof turlari |
| `O` + `'` / `` ` `` / `‘` / `’` / `ʻ` / `ʼ` / `´` | `Ö` | `U+00D6` | Barcha apostrof turlari |
| `g` + `'` / `` ` `` / `‘` / `’` / `ʻ` / `ʼ` / `´` | `ğ` | `U+011F` | Barcha apostrof turlari |
| `G` + `'` / `` ` `` / `‘` / `’` / `ʻ` / `ʼ` / `´` | `Ğ` | `U+011E` | Barcha apostrof turlari |
| *(ixtiyoriy)* `n` + `g` | `ñ` / `Ñ` | `U+00F1` / `U+00D1` | Sozlamada yoqilganda (`enable_ng = true`) |

---

## 📥 Foydalanuvchilar uchun: O‘rnatish

Tayyor dasturni [GitHub Releases](https://github.com/0605AbMu/harfiy/releases) sahifasidan operatsion tizimingizga mos holda yuklab oling:

### 🍎 macOS (Apple Silicon M1/M2/M3/M4 va Intel)
1. `.pkg` (o‘rnatuvchi usta) yoki `.dmg` faylini yuklab oling va oching.
2. Agar macOS Gatekeeper ogohlantirishi chiqsa:
   - **Oddiy usul:** **System Settings** -> **Privacy & Security** bo‘limiga tushib, **«Open Anyway»** tugmasini bosing.
   - **Terminal orqali (1 soniya):**
     ```bash
     xattr -cr ~/Downloads/harfiy*
     ```
3. O‘rnatilgach, macOS tizim panelida (Tray) Harfiy belgisi paydo bo‘ladi.
4. Tizim ruxsati so‘ralganda: **System Settings** -> **Privacy & Security** -> **Accessibility** bo‘limida Harfiy ilovasini yoqing.

### 🪟 Windows (10 / 11)
- `harfiy-...-windows-x86_64-setup.exe` faylini yuklab oling va odatiy tarzda o‘rnating.
- O‘rnatuvchi dasturni avtomatik ishga tushirish (Autostart) va bosh menyuga yorliq qo‘shish imkonini beradi.

### 🐧 Linux (Ubuntu / Debian / Mint)
- `.deb` paketini yuklab olib, terminalda o‘rnating:
  ```bash
  sudo dpkg -i harfiy_*_amd64.deb
  ```

---

## 🏛️ Dasturchilar uchun: Arxitektura va Tuzilma

Harfiy mustaqil modullardan iborat **Cargo Workspace (monorepo)** arxitekturasida qurilgan:

```
harfiy/
├── Cargo.toml                  # Workspace konfiguratsiyasi va LTO release profili
├── crates/
│   ├── engine/                 # Platformadan 100% mustaqil FSM dvigateli (harfiy-engine)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── fsm.rs          # Deterministik State Machine (Stack-based Instant Undo)
│   │   │   └── rules.rs        # Konvertatsiya xaritalari va Unicode qoidalari
│   │   └── tests/              # 100% test qamrovi
│   ├── platform/               # OS darajasidagi klaviatura hook va sintetik emitterlar (harfiy-platform)
│   │   ├── src/
│   │   │   ├── lib.rs          # KeyboardInterceptor va KeyboardEmitter trait'lari
│   │   │   ├── macos.rs        # macOS adapteri (CGEventTap + CoreGraphics + Accessibility)
│   │   │   ├── windows.rs      # Windows hook (WH_KEYBOARD_LL + SendInput + LLKHF_INJECTED)
│   │   │   ├── linux.rs        # Linux adapteri (evdev + uinput)
│   │   │   └── mock.rs         # Sof xotiradagi test adapteri (CI/CD uchun)
│   │   └── tests/
│   ├── tray/                   # Tizim paneli (System Tray) va global hotkey (harfiy-tray)
│   │   └── src/
│   │       └── lib.rs          # muda + tray-icon + global-hotkey
│   └── daemon/                 # CLI va fon jarayonini boshqaruvchi (harfiy-daemon)
│       └── src/
│           ├── main.rs         # Lifecycle, signal handling (Ctrl+C), multithreading
│           └── config.rs       # config.toml yuklash va boshqarish
├── packaging/                  # O‘rnatuvchi paketlar skriptlari (.pkg, .dmg, .iss, .deb)
└── .github/workflows/          # Avtomatlashtirilgan release CI workflow
```

---

## 🛠️ Mahalliy Kompilyatsiya va Ishga Tushirish

### Talablar
- **Rust:** `1.80.0` yoki undan yuqori (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Git**

### Linux tizimlari uchun bog‘liqliklar:
```bash
sudo apt-get update
sudo apt-get install -y pkg-config libglib2.0-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libx11-dev
```

### Kodni tekshirish va yig‘ish:
```bash
# 1. Repozitoriyni klonlash
git clone https://github.com/0605AbMu/harfiy.git
cd harfiy

# 2. Barcha testlarni o‘tkazish (Engine, Platform, Tray, Daemon)
cargo test --workspace

# 3. Kod formatini va linterlarni tekshirish
cargo fmt --check
cargo clippy --workspace -- -D warnings

# 4. Release rejimida yig‘ish (LTO va strip bilan)
cargo build --release
```

### Dasturni Sinab Ko‘rish:
```bash
# Tizim paneli (Tray) bilan ishga tushirish
cargo run -p harfiy-daemon -- start

# Konsol (headless) rejimida ishga tushirish
cargo run -p harfiy-daemon -- --no-tray

# Jarayon holatini tekshirish
cargo run -p harfiy-daemon -- status
```

---

## 🤝 Ochiq Kodli Hamkorlik Qoidalari (Collaboration & Contributing)

Harfiy — ochiq kodli loyiha va jamoaviy hissalarni mamnuniyat bilan qabul qiladi!

Loyiha qoidalariga rioya qilgan holda hissa qo‘shish uchun:

1. Batafsil yo‘riqnoma bilan tanishib chiqing: [CONTRIBUTING.md](CONTRIBUTING.md).
2. **Asosiy qoida:** Harfiy foydalanuvchilarning klaviatura hodisalarini ushlaganligi sababli, **har qanday tarmoq kutubxonalari (networking), telemetriya va tashqi so‘rovlar kiritish qat'iyan man etiladi**.
3. Yangi imkoniyatlar yoki tuzatishlar alohida tarmoqda (`feature/...` yoki `fix/...`) ishlab chiqilishi va unit-testlar bilan qamrab olinishi lozim.
4. Pull Request yuborishdan oldin quyidagilar bajarilgan bo‘lishi shart:
   - `cargo fmt --check`
   - `cargo clippy --workspace -- -D warnings`
   - `cargo test --workspace`

---

## 🚀 Reliz va Versiyalash Jarayoni

Relizlar to‘liq avtomatlashtirilgan. Yangi reliz chiqarish uchun faqatgina versiya raqami bilan Git tag qo‘yish kifoya:

```bash
# Yangi versiya tegi yaratish va yuborish
git tag 0.9.0
git push origin 0.9.0
```

GitHub Actions avtomatik tarzda:
1. Windows (`.exe` o‘rnatuvchi + `.zip`),
2. macOS Apple Silicon va Intel (`.pkg` usta + `.dmg` + `.tar.gz`),
3. Linux (`.deb` + `.tar.gz`),
4. `SHA256SUMS.txt` xesh yig‘indilarini

tayyorlab, to‘g‘ridan-to‘g‘ri GitHub Releases sahifasida nashr etadi.

---

## 📊 Samaradorlik Ko‘rsatkichlari (NFR)

| Ko‘rsatkich | Talab | Haqiqiy Natija |
| :--- | :--- | :--- |
| **Binar Hajmi** | `< 10 MB` | **~1.1 MB** *(LTO va strip yoqilgan)* |
| **Kechikish (Latency)** | `< 1 ms` | **< 0.05 ms** (sub-millisecond) |
| **RAM Sarfi** | `< 15 MB` | **< 8 MB** |
| **Tarmoq Xavfsizligi** | Tarmoqsiz (Offline) | 0 ta tarmoq kutubxonasi, to‘liq izolyatsiya |

---

## 📜 Litsenziya

Loyiha [MIT License](LICENSE) asosida tarqatiladi. Ushbu litsenziyaga muvofiq, koddan tijoriy va notijoriy maqsadlarda erkin foydalanish, o‘zgartirish va tarqatish mumkin.

---

## 👨‍💻 Muallif va Aloqa

* **Muallif va Asoschi:** Abdumannon Shamsiyev
* **Veb-sahifa:** [abdumannon-shamsiyev.uz/harfiy](https://abdumannon-shamsiyev.uz/harfiy)
* **Telegram Blog:** [@abdumannon_notes](https://t.me/abdumannon_notes)
* **Telegram Muloqot:** [@abdumannon_shamsiyev](https://t.me/abdumannon_shamsiyev)
* **LinkedIn:** [linkedin.com/in/abdumannon-shamsiyev](https://www.linkedin.com/in/abdumannon-shamsiyev)
