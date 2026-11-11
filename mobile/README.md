# Harfiy Mobile: iOS va Android Yordamchisi

Ushbu katalog **Harfiy** avto-konvertatsiya tizimining **iOS** (Apple Keyboard Extension) va **Android** (AccessibilityService - tizim klaviaturasi ustida ishlash) platformalari uchun ishlab chiqilgan mobil loyihalarini o'z ichiga oladi.

---

## 📁 Katalog Strukturasi

```text
mobile/
├── scripts/
│   ├── build_ios.sh       # iOS XCFramework (libharfiy_ffi.a) yig'ish skripti
│   ├── build_android.sh   # Android jniLibs (libharfiy_ffi.so) yig'ish skripti
│   ├── e2e_android.py     # Haqiqiy qurilmada klaviaturani bosib test qilish
│   └── keyboard_layouts/  # e2e uchun klaviatura tugmalari koordinatalari
├── ios/                   # Swift / SwiftUI iOS loyihasi
│   ├── Frameworks/        # HarfiyCore.xcframework (avtomatik yaratiladi)
│   ├── HarfiyApp/         # Container App (yo'riqnoma, sinov maydoni)
│   └── HarfiyKeyboard/    # UIInputViewController (klaviatura extension)
└── android/               # Kotlin Android Studio loyihasi
    ├── app/
    │   ├── src/main/jniLibs/  # .so kutubxonalari (avtomatik yaratiladi)
    │   ├── src/main/kotlin/uz/harfiy/keyboard/
    │   │   ├── HarfiyEngine.kt                # JNI Bridge (qoidalar)
    │   │   ├── HarfiySession.kt               # JNI Bridge (tahrir sessiyasi, crates/ffi/src/session.rs)
    │   │   ├── HarfiyAccessibilityService.kt  # Tizim klaviaturasi (Gboard, Samsung) ustida ishlovchi xizmat
    │   │   └── MainActivity.kt                # Boshqaruv paneli va sinov maydoni
    │   └── src/debug/.../TestbedActivity.kt   # Faqat debug: e2e test maydonlari
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

### Android xizmati qanday ishlaydi (barqarorlik)

Harfiy va foydalanuvchi klaviaturasi bitta matnga yozadi, lekin accessibility `InputConnection`da atomar almashtirish ham, `beginBatchEdit` ham yo'q. Shuning uchun asosiy qoida: **ishonch bo'lmasa — matnga tegilmaydi**. Barcha qarorlar Rust'dagi `crates/ffi/src/session.rs` holat mashinasida; Kotlin xizmati faqat hodisalarni uzatadi va so'ralgan o'qish/yozishni bajaradi.

1. Klaviatura so'z yozayotganda (composing) kutiladi; composing klaviaturalarda oxirgi o'zgarishdan keyin 100 ms sokinlik kutiladi (tez yozish oqimi ichida yozilmaydi). Oddiy (composing'siz) klaviaturada konversiya darhol va Instant Undo bilan ishlaydi.
2. Yozishdan oldin matn qayta o'qiladi: kursor foydalanuvchi oxirgi yozgan joyda, matn oxirgi hodisadagi bilan bir xil va (offset bor bo'lsa) ikkala kanal kursori mos bo'lishi shart.
3. Matn hech qachon belgilanmaydi (`setSelection` ishlatilmaydi): kursordan oldingi matn `deleteSurroundingText` bilan o'chirilib, `commitText` bilan yoziladi. Samsung Keyboard vaqtinchalik belgilashni ko'rib, tez yozishda keyingi harfni noto'g'ri joyga yozishi qurilmada o'lchangan. Bu `offset == -1` muharrirlarda (Jetpack Compose) ham to'g'ri ishlaydi.
4. Har bir yozuv qayta o'qib tasdiqlanadi; yozuv echo'si klaviatura matnini almashtirgani ko'rinsa ham. Tasdiqlanmasa, shu maydonda Harfiy o'chadi; bitta ilovada 2 marta takrorlansa, xizmat qayta ishga tushguncha o'sha ilova o'tkazib yuboriladi.
5. Echo (o'z tahririmiz hodisasi) vaqt bo'yicha emas, kutilgan o'zgarish shakli bo'yicha aniqlanadi.
6. WebView (Chromium) accessibility'ga `getSurroundingText` bermaydi: u yerda Harfiy hech narsa yozmaydi.
7. Release build'da `Log.d/i/v` R8 orqali olib tashlanadi; xizmat hech qachon yozilgan matnni log qilmaydi. **Sozlamalar → Xato haqida xabar berish** faqat pozitsiya va uzunliklardan iborat diagnostika jurnalini ulashadi.

### Testlar

```bash
# Holat mashinasi + qurilma simulyatori (tasodifiy hodisa tartiblari, composing va oddiy klaviatura, offset -1)
cargo test -p harfiy-ffi
```

```bash
# Haqiqiy qurilmada: debug build o'rnatilgan va Harfiy yoqilgan bo'lishi kerak
python3 mobile/scripts/e2e_android.py --layout mobile/scripts/keyboard_layouts/samsung_sm-a556e.json
```

`e2e_android.py` debug-only `TestbedActivity`dagi uch xil maydonga (`EditText`, offset bermaydigan maydon, WebView) ekrandagi klaviaturani bosib yozadi va natijani kutilgan konversiya bilan solishtiradi. Boshqa qurilma/klaviatura uchun `keyboard_layouts/` ga koordinatalar faylini qo'shing.

---

## 🔒 Xavfsizlik va Maxfiylik (Zero Network)
* iOS klaviaturasi `RequestsOpenAccess = false` rejimida ishlaydi (Apple tomonidan to'liq ajratilgan).
* Android klaviaturasi hech qanday `INTERNET` ruxsatini talab qilmaydi.
* Barcha hisob-kitoblar va qoidalar telefondagi xotiraning o'zida bir zumda (sub-millisecond) bajariladi.
