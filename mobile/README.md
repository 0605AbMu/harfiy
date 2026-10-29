# Harfiy Mobile: iOS va Android Yordamchisi

Ushbu katalog **Harfiy** avto-konvertatsiya tizimining **iOS** (Apple Keyboard Extension) va **Android** (AccessibilityService - tizim klaviaturasi ustida ishlash) platformalari uchun ishlab chiqilgan mobil loyihalarini o'z ichiga oladi.

---

## 📁 Katalog Strukturasi

```text
mobile/
├── scripts/
│   ├── build_ios.sh       # iOS XCFramework (libharfiy_ffi.a) yig'ish skripti
│   └── build_android.sh   # Android jniLibs (libharfiy_ffi.so) yig'ish skripti
├── ios/                   # Swift / SwiftUI iOS loyihasi
│   ├── Frameworks/        # HarfiyCore.xcframework (avtomatik yaratiladi)
│   ├── HarfiyApp/         # Container App (yo'riqnoma, sinov maydoni)
│   └── HarfiyKeyboard/    # UIInputViewController (klaviatura extension)
└── android/               # Kotlin Android Studio loyihasi
    ├── app/
    │   ├── src/main/jniLibs/  # .so kutubxonalari (avtomatik yaratiladi)
    │   └── src/main/kotlin/uz/harfiy/keyboard/
    │       ├── HarfiyEngine.kt                # JNI Bridge
    │       ├── HarfiyAccessibilityService.kt  # Tizim klaviaturasi (Gboard, Samsung) ustida ishlovchi xizmat
    │       └── MainActivity.kt                # Boshqaruv paneli va sinov maydoni
```

---

## 🚀 1. iOS Klaviaturasini Yig'ish va Sinash

### 1-qadam: Rust XCFramework'ni yaratish
Terminal orqali quyidagi buyruqni ishga tushiring:
```bash
./mobile/scripts/build_ios.sh
```
Ushbu skript `aarch64-apple-ios` va `aarch64-apple-ios-sim` targetlarida `harfiy-ffi` ni hajm bo'yicha optimallashtirilgan `ios` profilida (`opt-level = "s"`) kompilyatsiya qiladi va `mobile/ios/Frameworks/HarfiyCore.xcframework` faylini tayyorlaydi.

> **Eslatma:** iOS loyihasida Swift zaxira (fallback) mantiqi yo'q — barcha konversiya Rust engine orqali bajariladi, shuning uchun Xcode'da ochishdan oldin `HarfiyCore.xcframework` yig'ilgan bo'lishi **shart**.

### 2-qadam: Xcode orqali ochish va sinash
1. Xcode dasturida `mobile/ios` loyihasini oching.
2. O'zingizning iPhone telefoningizni (yoki Simulyatorni) tanlang va **Run (Cmd + R)** tugmasini bosing.
3. Telefonda: **Settings -> General -> Keyboard -> Keyboards -> Add New Keyboard -> Harfiy** ni tanlang.
4. Istalgan messenjerda (Telegram, Notes) `shahar`, `choy`, `o'rik`, `g'isht` deb yozib sinab ko'ring.

### iOS klaviaturasini profil qilish (Instruments)

Klaviatura kodida `os_signpost` belgilari bor (`HarfiyKeyboard/HarfiyPerf.swift`). Profiler ulanmaganda ular deyarli bepul, shuning uchun release'da ham qoladi.

1. Xcode'da `HarfiyKeyboard` sxemasini tanlab, host ilova sifatida Notes (yoki Telegram) ni belgilang va **Product -> Profile (Cmd + I)** ni bosing.
2. Instruments shabloni: **Time Profiler** + **Points of Interest** (yoki **os_signpost**).
3. Quyidagi belgilarni qidiring (subsystem `uz.harfiy.keyboard`):

| Belgi | Nimani o'lchaydi |
|-------|------------------|
| `keyboardLaunch` | `viewDidLoad` dan birinchi ko'rinishgacha (cold-start) |
| `insertCharacter` | bitta harf bosilishi: engine + host proxy chaqiruvlari |
| `deleteBackward` | Backspace (Instant Undo bilan) |
| `selectionDidChange` | kursor/tanlov o'zgarishi, host kontekstini o'qish bilan |
| `emojiSections` | emoji klaviatura ma'lumotlarini yig'ish |
| `availableMemoryMB` (event) | extension tizim tomonidan to'xtatilishidan oldin qolgan xotira |

Xotira bo'yicha: `availableMemoryMB` qiymati klaviatura ochilgan sari kamayib borsa, xotira oqishi bor.

---

## 🤖 2. Android Yordamchisini Yig'ish va Sinash

Android'da Harfiy hech qanday qo'shimcha yoki noqulay klaviatura talab qilmaydi. Siz har doimgidek o'zingizning sevimli tizim klaviaturangizda (**Gboard**, **Samsung Keyboard**, **SwiftKey**) yozasiz. Harfiy `AccessibilityService` orqali fonda harflarni avtomatik to'g'rilab beradi!

### 1-qadam: Android JNI kutubxonalarini yig'ish
```bash
./mobile/scripts/build_android.sh
# yoki Windows (PowerShell) da:
# powershell -ExecutionPolicy Bypass -File ./mobile/scripts/build_android.ps1
```
Ushbu skript `cargo-ndk` orqali `arm64-v8a`, `armeabi-v7a` va `x86_64` uchun `libharfiy_ffi.so` fayllarini `mobile/android/app/src/main/jniLibs` ga joylaydi.

### 2-qadam: Android Studio'da ochish va telefonga o'rnatish
1. Android Studio orqali `mobile/android` papkasini oching (*Open existing project*).
2. Telefoningizni USB orqali ulab, **Run (Shift + F10)** tugmasini bosing yoki terminalda:
   ```bash
   cd mobile/android && ./gradlew installDebug
   # Windows da: cd mobile\android && .\gradlew.bat installDebug
   ```
3. Ilova ochilgach, **«Sozlamalardan Harfiy'ni yoqish»** tugmasini bosing va Maxsus imkoniyatlar (Accessibility) bo'limidan Harfiy'ni yoqing.
4. Endi Telegram, WhatsApp, Notes yoki istalgan ilovada sevimli klaviaturangizda bemalol yozavering!

---

## 🔒 Xavfsizlik va Maxfiylik (Zero Network)
* iOS klaviaturasi `RequestsOpenAccess = false` rejimida ishlaydi (Apple tomonidan to'liq ajratilgan).
* Android klaviaturasi hech qanday `INTERNET` ruxsatini talab qilmaydi.
* Barcha hisob-kitoblar va qoidalar telefondagi xotiraning o'zida bir zumda (sub-millisecond) bajariladi.
