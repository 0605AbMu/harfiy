# Harfiy Mobile: iOS va Android Klaviaturasi

Ushbu katalog **Harfiy** avto-konvertatsiya tizimining **iOS** (Apple Keyboard Extension) va **Android** (InputMethodService) platformalari uchun ishlab chiqilgan mobil loyihalarini o'z ichiga oladi.

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
    │       ├── HarfiyEngineBridge.kt       # JNI Bridge
    │       ├── HarfiyInputMethodService.kt # Klaviatura xizmati
    │       └── MainActivity.kt             # Faollashtirish ustasi
```

---

## 🚀 1. iOS Klaviaturasini Yig'ish va Sinash

### 1-qadam: Rust XCFramework'ni yaratish
Terminal orqali quyidagi buyruqni ishga tushiring:
```bash
./mobile/scripts/build_ios.sh
```
Ushbu skript `aarch64-apple-ios` va `aarch64-apple-ios-sim` targetlarida `harfiy-ffi` ni kompilyatsiya qiladi va `mobile/ios/Frameworks/HarfiyCore.xcframework` faylini tayyorlaydi.

### 2-qadam: Xcode orqali ochish va sinash
1. Xcode dasturida `mobile/ios` loyihasini oching.
2. O'zingizning iPhone telefoningizni (yoki Simulyatorni) tanlang va **Run (Cmd + R)** tugmasini bosing.
3. Telefonda: **Settings -> General -> Keyboard -> Keyboards -> Add New Keyboard -> Harfiy** ni tanlang.
4. Istalgan messenjerda (Telegram, Notes) `shahar`, `choy`, `o'rik`, `g'isht` deb yozib sinab ko'ring.

---

## 🤖 2. Android Klaviaturasini Yig'ish va Sinash

### 1-qadam: Android JNI kutubxonalarini yig'ish
```bash
./mobile/scripts/build_android.sh
```
Ushbu skript `cargo-ndk` orqali `arm64-v8a`, `armeabi-v7a` va `x86_64` uchun `libharfiy_ffi.so` fayllarini `mobile/android/app/src/main/jniLibs` ga joylaydi.

### 2-qadam: Android Studio'da ochish
1. Android Studio orqali `mobile/android` papkasini oching (*Open existing project*).
2. Telefoningizni USB orqali ulab, **Run (Shift + F10)** tugmasini bosing.
3. Ilova ochilgach, **«Sozlamalardan Klaviaturani Yoqish»** tugmasini bosing va Harfiy'ni faollashtiring.

---

## 🔒 Xavfsizlik va Maxfiylik (Zero Network)
* iOS klaviaturasi `RequestsOpenAccess = false` rejimida ishlaydi (Apple tomonidan to'liq ajratilgan).
* Android klaviaturasi hech qanday `INTERNET` ruxsatini talab qilmaydi.
* Barcha hisob-kitoblar va qoidalar telefondagi xotiraning o'zida bir zumda (sub-millisecond) bajariladi.
