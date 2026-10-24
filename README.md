# Harfiy: Cross-Platform O‘zbek Alifbosi Avto-Konvertatsiya Dvigateli

**Harfiy** — zamonaviy o‘zbek lotin alifbosidagi harf birikmalarini (`sh`, `ch`, `o'`, `g'`) real vaqt rejimida yozish paytida yangi unikal grafemalarga (`ş`, `ç`, `ö`, `ğ`) kechikishsiz (sub-millisecond) va uzluksiz almashtirib beruvchi tizim darajasidagi fon xizmati (daemon).

Loyiha to'liq **Rust** tilida, xavfsiz, nol xotira yuklamasi (zero-cost abstractions) va qat'iy determinizm tamoyillari asosida ishlab chiqilgan.

---

## 🏛️ Arxitektura va Tuzilma

Loyiha Cargo Workspace monorepo tuzilishiga ega:

```
harfiy/
├── Cargo.toml                  # Workspace konfiguratsiyasi va LTO release profili
├── crates/
│   ├── engine/                 # Platformadan mustaqil FSM va qoidalar dvigateli (harfiy-engine)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── fsm.rs          # Finite State Machine (stack-based, Instant Undo)
│   │   │   └── rules.rs        # Konvertatsiya xaritalari va apostrof turlari
│   │   └── tests/              # 100% qamrovli xotira testlari
│   ├── platform/               # OS qatlamlari uchun interceptor va emitter trait'lari (harfiy-platform)
│   │   ├── src/
│   │   │   ├── lib.rs          # KeyboardInterceptor va KeyboardEmitter trait'lari
│   │   │   ├── windows.rs      # Windows hook (WH_KEYBOARD_LL + SendInput + Magic loop guard)
│   │   │   ├── macos.rs        # macOS adapteri (CGEventTap + CoreGraphics)
│   │   │   ├── linux.rs        # Linux adapteri (evdev + uinput)
│   │   │   └── mock.rs         # Sof xotiradagi Mock interceptor va emitter
│   ├── tray/                   # Tizim paneli (System Tray) va global hotkey (harfiy-tray)
│   │   └── src/
│   │       └── lib.rs          # tray-icon + global-hotkey (Ctrl+Shift+U)
│   └── daemon/                 # Asosiy CLI va boshqaruvchi fon xizmati (harfiy-daemon)
│       └── src/
│           ├── main.rs         # Lifecycle, signal handling (Ctrl+C), argumentlar
│           └── config.rs       # config.toml yuklash va boshqarish
```

---

## ⚙️ Konvertatsiya Qoidalari va Xususiyatlari

### 1. Asosiy Qoidalar Matrisasi

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

### 2. ⚡ Instant Undo (Tezkor Bekor Qilish)
Agar avto-konvertatsiyadan so'ng (`sh` -> `ş`) foydalanuvchi darhol `Backspace` tugmasini bossa, dvigatel `ş` ni o'chiradi va uning o'rniga asl kiritilgan juftlikni (`sh`) tiklaydi. Ikkinchi bosilgan `Backspace` odatdagi tarzda bitta harfni o'chiradi.

### 3. 🛡️ Klaviaturadagi Boshqa Tugmalar
- `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`, `Escape`, `Enter`, `Tab` bosilganda FSM darhol `IDLE` holatiga qaytadi.
- `Ctrl` yoki `Alt` kombinatsiyalari (`Ctrl+C`, `Ctrl+V`, `Ctrl+S` va h.k.) paytida dvigatel aralashmaydi va tizim buyruqlarini buzmaydi.

---

## 📥 Yuklab Olish va O‘rnatish

Tayyor dasturni **[GitHub Releases](https://github.com/0605AbMu/harfiy-release/releases)** sahifasidan kompyuteringiz turiga mos holda yuklab oling:

### 🍎 macOS (Apple Silicon M1/M2/M3/M4 va Intel)

* O‘rnatish yoki yangilash uchun **`.pkg`** (yoki `.dmg`) faylini yuklab oling.
* O‘rnatish ustasi (Installer Wizard) eski versiya ishlab turgan bo‘lsa uni to‘xtatadi, yangisini o‘rnatadi va avtomatik ishga tushiradi.

> [!IMPORTANT]
> ### ⚠️ macOS Gatekeeper («Noma’lum dasturchi») ogohlantirishini yechish:
> 
> Agar faylni ochganda ***«cannot be opened because it is from an unidentified developer»*** xabari chiqsa:
> 
> 👉 **Eng oson va tezkor yo‘li (2 soniyada):**
> 1. `.pkg` (yoki `.dmg`) fayli ustiga sichqonchaning **O‘ng tugmasini** (yoki klaviaturadagi **`Control`** tugmasini bosib turib sichqonchaning chap tugmasini) bosing.
> 2. Chiqqan menyudan **«Open» (Ochish)** bandini tanlang.
> 3. Ekranga chiqqan yangi oynada **«Open» (Baribir ochish)** tugmasini bosing!
> 
> *(Alternativ yo‘li: **System Settings (Tizim sozlamalari)** ➔ **Privacy & Security (Maxfiylik va xavfsizlik)** bo‘limining eng pastida **«Open Anyway» (Baribir ochish)** tugmasini bosing).*

### 🪟 Windows (10 / 11)
* `harfiy-v...-windows-x86_64-setup.exe` faylini yuklab oling va ishga tushiring.

### 🐧 Linux (Ubuntu / Debian / Mint)
* `harfiy_..._amd64.deb` faylini yuklab olib, o‘rnating:
  ```bash
  sudo dpkg -i harfiy_*_amd64.deb
  ```

---

## 🚀 Dasturchilar uchun: Kompilyatsiya va Boshqaruv

### Talablar
- Rust 1.80+ (stable toolchain)

### Kompilyatsiya va Testlar
```bash
# Barcha testlarni ishga tushirish (Engine + Platform)
cargo test --workspace

# Release rejimida maksimal optimizatsiya va LTO bilan yig'ish
cargo build --release
```

### Dasturni Ishga Tushirish
```bash
# Tizim paneli (Tray) bilan ishga tushirish
cargo run -p harfiy-daemon -- start

# Konsol (headless) rejimida ishga tushirish
cargo run -p harfiy-daemon -- --no-tray

# Holatni tekshirish
cargo run -p harfiy-daemon -- status

# Yangilanishlarni tekshirish (GitHub Releases orqali)
cargo run -p harfiy-daemon -- check-updates
```

### Global Hotkey
- **`Ctrl + Shift + U`**: Harfiy faoliyatini bir zumda yoqish yoki to'xtatib turish (Pause/Resume).

---

## 🛠️ Konfiguratsiya (`config.toml`)

Dastur ishga tushganda avtomatik ravishda `config.toml` yaratiladi yoki yuklanadi:

```toml
# Harfiy Konfiguratsiya Fayli
enable_ng = false
hotkey = "Ctrl+Shift+U"
excluded_apps = []
check_updates = true
github_repo = "0605AbMu/harfiy-release"
```

Barcha rasmiy relizlar va o'rnatuvchi paketlar sahifasi: **[GitHub Releases](https://github.com/0605AbMu/harfiy-release/releases)**

---

## 🚀 Versiyalash va Avtomatlashtirilgan Release (CI/CD)

Loyihada versiyalash va barcha platformalar uchun relizlar chiqarish to‘liq **GitHub Actions** orqali avtomatlashtirilgan.

### 1. Release chiqarish usullari:

* **1-usul: Git Tag orqali (Tavsiya etiladi)**
  ```bash
  # Yangi versiya tegi yaratish va yuborish
  git tag v0.1.0
  git push origin v0.1.0
  ```

* **2-usul: GitHub Actions veb-interfeysi orqali (Bitta tugma bilan)**
  1. GitHub repozitoriyida **Actions** bo‘limiga o‘ting.
  2. Chap tomondan **Bump Version & Trigger Release** workflow'ini tanlang.
  3. **Run workflow** tugmasini bosib, yangi versiyani kiriting (masalan: `0.2.0`).
  4. Workflow avtomatik ravishda `Cargo.toml` fayllarini yangilaydi, commit qiladi, teg qo‘yadi va relizni boshlaydi.

### 2. Avtomatik Yig'iladigan O'rnatuvchi Paketlar (Installers):
* 🪟 **Windows (`.exe` Installer):** `harfiy-vX.Y.Z-windows-x86_64-setup.exe`
  - Inno Setup orqali to'liq o'rnatuvchi: Start Menu va Desktop yorliqlari, Windows bilan avtomatik ishga tushish (Autostart) tanlovi va toza uninstaller.
  - Qo'shimcha: Portativ `harfiy-vX.Y.Z-windows-x86_64.zip`.
* 🍎 **macOS (`.pkg` va `.dmg` Installer):** `harfiy-vX.Y.Z-macos-{x86_64|arm64}.pkg` va `.dmg`
  - Intel va Apple Silicon (M1/M2/M3/M4) uchun rasmiy Apple Installer Wizard (`.pkg`): yangilash paytida eski ilovani avtomatik to‘xtatadi, yangisini o‘rnatadi va avtomatik ishga tushiradi.
  - Apple Disk Image (`.dmg`): ichida `Harfiy.app` va Installer Wizard mavjud.
  - Qo‘shimcha: Portativ `.tar.gz` arxivlari.
* 🐧 **Ubuntu / Debian (`.deb` Installer):** `harfiy_X.Y.Z_amd64.deb`
  - `sudo dpkg -i harfiy_*.deb` orqali tizimga o'rnatish, `/usr/bin/harfiy`, `.desktop` menyu yorlig'i, hicolor piktogrammalari va `systemd --user` xizmat integratsiyasi.
  - Qo'shimcha: Portativ `harfiy-vX.Y.Z-linux-x86_64.tar.gz`.
* 🔐 **Nazorat Yig'indisi:** Har bir reliz uchun barcha fayllarning `SHA256SUMS.txt` raqamli imzolangan xeshlari avtomatik hisoblanadi.

---

## 📊 Nofunksional Ko'rsatkichlar (NFR)

| Ko'rsatkich | Talab (TZ) | Haqiqiy Natija |
| :--- | :--- | :--- |
| **Static Binary Hajmi** | `< 10 MB` | **0.81 MB** (844 KB) *(LTO va strip yoqilgan)* |
| **Kechikish (Latency)** | `< 1 ms` | **< 0.05 ms** (sub-millisecond) |
| **RAM Sarfi** | `< 15 MB` | **< 8 MB** |
| **Xavfsizlik** | Tarmoqsiz (Isolated) | Tarmoq kutubxonalari mavjud emas |

---

## 👨‍💻 Muallif va Aloqa

* **Muallif va Publisher:** Abdumannon Shamsiyev
* **Blog:** [@abdumannon_notes](https://t.me/abdumannon_notes) *(Deep link: `tg://resolve?domain=abdumannon_notes`)*
* **Qo'llab-quvvatlash (Support):** [@abdumannon_shamsiyev](https://t.me/abdumannon_shamsiyev) *(Deep link: `tg://resolve?domain=abdumannon_shamsiyev`)*
* **LinkedIn:** [linkedin.com/in/abdumannon-shamsiyev](https://www.linkedin.com/in/abdumannon-shamsiyev)

