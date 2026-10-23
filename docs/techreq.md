# Texnik Topshiriq (TZ): Cross-Platform O‘zbek Alifbosi Avto-Konvertatsiya Dvigateli ("Harfiy")

Ushbu hujjat zamonaviy lotin yozuviga asoslangan o'zbek alifbosidagi harf birikmalarini (`sh`, `ch`, `o'`, `g'`) yozish paytida real vaqt rejimida yangi unikal grafemalarga (`ş`, `ç`, `ö`, `ğ`) almashtirib beruvchi tizim darajasidagi fon xizmati (daemon) uchun texnik talablarni belgilaydi. Tizim to'liq **Rust** tilida, nol qo'shimcha xotira yuklamasi (zero-cost abstractions) va qat'iy deterministik kechikish (sub-millisecond latency) talablari asosida ishlab chiqiladi.

---

## 1. Tizim Arxitekturasi va Modullar Tuzilishi

Loyiha Cargo Workspace asosidagi monorepo formatida tuziladi:

```
harfiy/
├── Cargo.toml                  # Workspace konfiguratsiyasi
├── crates/
│   ├── engine/                 # Platformadan mustaqil FSM va qoidalar dvigateli
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── fsm.rs          # Finite State Machine
│   │   │   └── rules.rs        # Konvertatsiya xaritalari
│   │   └── tests/
│   ├── platform/               # OS qatlamlari uchun umumiy trait va implementatsiyalar
│   │   ├── src/
│   │   │   ├── lib.rs          # Interceptor va Emitter trait'lari
│   │   │   ├── windows.rs      # windows-rs (WH_KEYBOARD_LL + SendInput)
│   │   │   ├── macos.rs        # core-graphics (CGEventTap)
│   │   │   └── linux.rs        # evdev + uinput (/dev/uinput)
│   ├── daemon/                 # Asosiy ishchi jarayon (CLI, lifecycle, signal handling)
│   │   └── src/
│   │       └── main.rs
│   └── tray/                   # Tizim paneli (System Tray) va global hotkey boshqaruvi
│       └── src/
│           └── lib.rs          # tray-icon orqali holatni boshqarish
```

---

## 2. Platformadan Mustaqil Asosiy Dvigatel (`harfiy-engine`)

### 2.1. Trait Abstraksiyalari

Dvigatel OS hodisalariga bevosita bog'lanmaydi. Platforma adapterlari quyidagi trait'larni qanoatlantirishi shart:

```rust
pub enum KeyAction {
    PassThrough,
    Suppress,
    Transform {
        backspaces_count: usize,
        replacement: &'static str,
    },
}

pub trait KeyboardInterceptor {
    fn run<F>(&mut self, handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static;
    
    fn stop(&mut self) -> Result<(), PlatformError>;
}

pub trait KeyboardEmitter {
    fn emit_backspace(&mut self, count: usize) -> Result<(), PlatformError>;
    fn emit_unicode(&mut self, text: &str) -> Result<(), PlatformError>;
}
```

### 2.2. Finite State Machine (FSM) Modeli

Holatlar mashinasi har bir kiritilgan `char` va uning modifikatorlarini tahlil qiladi. Dinamik xotira ajratish (heap allocation) bo'lmasligi uchun barcha holatlar stack xotirada boshqariladi.

```
                  ┌──────────────┐
                  │     IDLE     │
                  └──────┬───────┘
                         │
        ┌────────────────┼────────────────┐
        │ 's'/'S'        │ 'c'/'C'        │ 'o'/'O'/'g'/'G'
        ▼                ▼                ▼
   ┌─────────┐      ┌─────────┐      ┌─────────┐
   │ STATE_S │      │ STATE_C │      │ STATE_V │
   └────┬────┘      └────┬────┘      └────┬────┘
        │ 'h'/'H'        │ 'h'/'H'        │ [Apostrof belgilari]
        ▼                ▼                ▼
   ┌───────────────────────────────────────────┐
   │  Transform: Backspace(1) + Emit(Target)   │
   └─────────────────────┬─────────────────────┘
                         │
                         ▼
                  ┌──────────────┐
                  │     IDLE     │
                  └──────────────┘
```

### 2.3. Konvertatsiya Matrisasi va Qoidalari

| Oldingi Holat (`prev`) | Kiruvchi Hodisa (`curr`) | Natijaviy Harakat | Chiqish Simvoli |
| :--- | :--- | :--- | :--- |
| `'s'` | `'h'` | `Transform(1, "ş")` | `ş` (U+015F) |
| `'S'` | `'h'` | `Transform(1, "Ş")` | `Ş` (U+015E) |
| `'S'` | `'H'` | `Transform(1, "Ş")` | `Ş` (U+015E) |
| `'c'` | `'h'` | `Transform(1, "ç")` | `ç` (U+00E7) |
| `'C'` | `'h'` | `Transform(1, "Ç")` | `Ç` (U+00C7) |
| `'C'` | `'H'` | `Transform(1, "Ç")` | `Ç` (U+00C7) |
| `'o'` | `'\' '` / `` '`' `` / `'‘'` / `'’'` | `Transform(1, "ö")` | `ö` (U+00F6) |
| `'O'` | `'\' '` / `` '`' `` / `'‘'` / `'’'` | `Transform(1, "Ö")` | `Ö` (U+00D6) |
| `'g'` | `'\' '` / `` '`' `` / `'‘'` / `'’'` | `Transform(1, "ğ")` | `ğ` (U+011F) |
| `'G'` | `'\' '` / `` '`' `` / `'‘'` / `'’'` | `Transform(1, "Ğ")` | `Ğ` (U+011E) |

*Navigatsiya tugmalari (`ArrowLeft`, `ArrowRight`, `Home`, `End`), `Escape`, `Enter`, `Tab`, `Backspace` bosilganda FSM darhol `IDLE` holatiga qaytariladi va ichki bufer tozalanadi.*

### 2.4. Instant Undo

* **Instant Undo:** Agar foydalanuvchi avto-konvertatsiyadan so'ng darhol `Backspace` tugmasini bossa, `ş` o'chiriladi va o'rniga asl kiritilgan juftlik (`sh`) qayta tiklanadi. Ikkinchi bosilgan `Backspace` odatdagi tarzda bitta harfni o'chiradi.

---

## 3. Platforma Dvigatellari Implementatsiyasi (`harfiy-platform`)

### 3.1. Windows (Target: `x86_64-pc-windows-msvc`)

* **Kutubxona:** `windows = { version = "0.58", features = ["Win32_UI_WindowsAndMessaging", "Win32_UI_Input_KeyboardAndMouse"] }`
* **Interceptor:** `SetWindowsHookExW` yordamida `WH_KEYBOARD_LL`.
  * Callback ichida `KBDLLHOOKSTRUCT` xabarlari o'qiladi.
  * Agar hodisa dasturning o'zi tomonidan yuborilgan bo'lsa (`dwExtraInfo == HARFIY_MAGIC_FLAG`), u filtrlanadi (cheksiz siklning oldini olish).
  * Hodisa almashtirilishi kerak bo'lsa, hook callback'i `LRESULT(1)` qaytaradi (tizimga o'tkazmay yutib qoladi).
* **Emitter:** `SendInput` API orqali:
  * `wVk = VK_BACK` signali.
  * `wScan = unicode_char`, `dwFlags = KEYEVENTF_UNICODE`.

### 3.2. macOS (Target: `aarch64-apple-darwin`, `x86_64-apple-darwin`)

* **Kutubxona:** `core-graphics`, `core-foundation`.
* **Interceptor:** `CGEventTapCreate` orqali `kCGHIDEventTap` nuqtasida tinglash.
  * Tizim ruxsatnomasi: `AXIsProcessTrustedWithOptions` orqali Accessibility API huquqi tekshiriladi; ruxsat bo'lmasa, tizim sozlamalariga yo'naltiruvchi dialog chaqiriladi.
  * Hodisani bekor qilish uchun callback `NULL` qaytaradi.
* **Emitter:** `CGEventCreateKeyboardEvent` orqali virtual hodisa yaratish va `CGEventKeyboardSetUnicodeString` yordamida kiritish, so'ng `CGEventPost` bilan tizim oqimiga yuborish.

### 3.3. Linux (Target: `x86_64-unknown-linux-gnu`)

* **Arxitektura:** Wayland va X11 o'rtasida farqlanmaslik uchun kernel darajasidagi kiritish usuli qo'llaniladi.
* **Kutubxona:** `evdev`, `uinput`.
* **Interceptor:** `/dev/input/event*` orqali apparat klaviatura qurilmalarini aniqlash va `EVIOCGRAB` ioctl orqali hodisalarni eksklyuziv ushlab olish.
* **Emitter:** `/dev/uinput` drayveri orqali yangi virtual apparat klaviaturasi (Virtual Input Device) yaratish. Kiritilgan har bir tugma tahlil qilinadi: o'zgarmaydigan tugmalar to'g'ridan-to'g'ri virtual klaviatura orqali qayta chiqariladi, mos tushgan juftliklar esa `KEY_BACKSPACE` va kerakli simvol kodlari bilan almashtirib yuboriladi.

---

## 4. Boshqaruv va Tizim Integratsiyasi (`harfiy-daemon`, `harfiy-tray`)

1. **System Tray:** `tray-icon` crate yordamida tizim panelida yengil belgi:
   * Holatlar: *Faol (Active)*, *To'xtatilgan (Paused)*.
   * Menyu: *Yoqish/O'chirish*, *Qoidalar sozlamasi*, *Chiqish*.
2. **Global Hotkey:** `global-hotkey` crate yordamida:
   * `Ctrl + Shift + U` (yoki foydalanuvchi tanlagan kombinatsiya) dastur faoliyatini bir zumda to'xtatadi yoki qayta ishga tushiradi.
3. **Konfiguratsiya:** To'liq lokal `config.toml` fayli:
   * `ng` harfini o'zgartirishni yoqish/o'chirish (default: `false`).
   * Istisnolar ro'yxatini foydalanuvchi tomonidan kengaytirish imkoniyati.
   * Dasturiy ta'minot istisnolari (masalan, ma'lum `.exe` yoki jarayon nomlari uchun avtomatik o'chish).

---

## 5. Nofunksional Talablar (NFR)

1. **Latency:** Tugma bosilishidan transformatsiya signali chiqishigacha bo'lgan vaqt **< 1 ms** bo'lishi shart.
2. **Xotira (Footprint):**
   * Static binary hajmi: Release rejimida (LTO yoqilganda) **< 10 MB**.
   * Runtime RAM iste'moli: **< 15 MB**.
   * CPU yuki: Kutish holatida **0.0%**, faol terish rejimida **< 0.1%**.
3. **Xavfsizlik va Maxfiylik:**
   * Dasturda tarmoq kutubxonalari (`reqwest`, `tokio::net` va h.k.) umuman bo'lmasligi lozim (Network Isolated).
   * Kiritilgan matnlar xotirada log qilinmaydi, diskka yozilmaydi.
   * Parol kiritish maydonlarida (Windows TSF `GUID_PROP_INPUTSCOPE` orqali `IS_PASSWORD` aniqlanganda) avtomatik PassThrough rejimiga o'tish.

---

## 6. Ishlab Chiqish Bosqichlari va Tekshirish Mezonlari (Milestones)

### 1-Bosqich: Core Engine va Mock Interceptor
* `engine` crate'ini ishlab chiqish.
* Hech qanday OS chaqiruvlarisiz, sof xotiradagi kiritish oqimini testlovchi Unit testlar to'plami (100% code coverage).
* Barcha case kombinatsiyalari (`sh`, `Sh`, `SH`), apostrof turlari va Backspace holatlarini tasdiqlash.

### 2-Bosqich: Windows Native Adapter
* `windows-rs` orqali hook va emitter integratsiyasi.
* Matn muharrirlarida (Notepad, VS Code, Telegram, Chrome) kechikishsiz ishlashini sinash.
* Tsiklik chaqiruvlar (`infinite loop`) yo'qligini tekshirish.

### 3-Bosqich: Linux (uinput) va macOS (CGEventTap) Adapterlari
* Linux'da `uinput` virtual qurilmasi orqali X11 va Wayland sessiyalarida barqaror kiritishni ta'minlash.
* macOS'da Accessibility huquqlari tizimi va CoreGraphics event oqimini ulash.

### 4-Bosqich: System Tray va CLI Packaging
* Tray belgisi va global hotkey boshqaruvini ulash.
* Release profili uchun LTO va strip sozlamalarini joriy qilish:
  ```toml
  [profile.release]
  opt-level = 3
  lto = true
  codegen-units = 1
  panic = "abort"
  strip = true
  ```
* Windows uchun MSI/InnoSetup, macOS uchun `.dmg`, Linux uchun systemd service + `.deb` paketlarini yig'ish.