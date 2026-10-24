# Harfiy Loyihasiga Hissa Qo'shish Qo'llanmasi (Contributing Guide)

Harfiy ochiq kodli loyihasiga hissa qo'shishga qiziqish bildirganingiz uchun tashakkur! Ushbu loyiha o'zbek tili uchun qulay, tezkor va xavfsiz tizim darajasidagi vositalarni rivojlantirishga qaratilgan.

Har qanday hissa — yangi funksiyalar, xatoliklarni tuzatish (bug fixes), hujjatlashtirish yoki takliflar — iliq kutib olinadi.

---

## 🧭 Asosiy Prinsiplarimiz

1. **Maxfiylik va 100% Offline (Privacy First):**
   Harfiy foydalanuvchilarning klaviatura bosishlarini (keystrokes) qayta ishlaydi. Shu sababli, dasturda **hech qanday tarmoq so'rovlari (networking), telemetriya, analitika yoki tashqi ma'lumot jo'natish qat'iyan taqiqlanadi**. Kod bazasiga tarmoq kutubxonalari qo'shilmaydi.
2. **Determinizm va Sub-millisecond Latency:**
   FSM va platforma hook'lari kechikishsiz (< 0.05 ms) ishlashi shart. Xotira sarfi minimal (< 10 MB RAM) va binary hajmi imkon qadar ixcham saqlanishi kerak.
3. **Cross-Platform Muvofiqlik:**
   Asosiy mantiq (`harfiy-engine`) har qanday operatsion tizimdan mustaqil bo'lishi, OS darajasidagi kodlar esa `harfiy-platform` ostida qat'iy ajratilgan bo'lishi lozim.

---

## 🛠️ Dasturlash Muhitini Sozlash

### Talablar:
- **Rust:** `1.80.0` yoki undan yuqori (stable toolchain).
- **Git**

### Linux tizimlari uchun tizim kutubxonalari:
```bash
sudo apt-get update
sudo apt-get install -y pkg-config libglib2.0-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libx11-dev
```

### Repozitoriyni klonlash va testlarni tekshirish:
```bash
git clone https://github.com/0605AbMu/harfiy.git
cd harfiy

# Barcha testlarni o'tkazish
cargo test --workspace

# Loyihani release rejimida yig'ish
cargo build --release
```

---

## 🔀 Hissa Qo'shish Jarayoni (Workflow)

1. **Repozitoriyni Fork qiling:**
   GitHub sahifasida **Fork** tugmasini bosing va shaxsiy hisobingizga ko'chiring.

2. **Yangi tarmoq (branch) oching:**
   ```bash
   git checkout -b feature/yangi-imkoniyat
   # yoki
   git checkout -b fix/muammo-tavsifi
   ```

3. **O'zgarishlarni kiriting va testlar yozing:**
   - Dvigatelga yangi qoida yoki o'zgarish qo'shilsa, tegishli unit-test yozing (`crates/engine/tests/`).
   - Mavjud testlar buzilmaganligiga ishonch hosil qiling.

4. **Kodni tekshiring (Sifat nazorati):**
   Pull Request yuborishdan oldin quyidagi 3 ta buyruq xatosiz o'tishi shart:
   ```bash
   # 1. Kod formatini tekshirish
   cargo fmt --check

   # 2. Linter ogohlantirishlarini tekshirish
   cargo clippy --workspace -- -D warnings

   # 3. Barcha testlarni ishga tushirish
   cargo test --workspace
   ```
   *Agar formatlashda farq bo'lsa, `cargo fmt` buyrug'i orqali avtomatik to'g'rilang.*

5. **Commit xabarlari (Conventional Commits):**
   Commit xabarlarini tushunarli va standart asosida yozish tavsiya etiladi:
   - `feat: yangi qoida yoki imkoniyat qo'shildi`
   - `fix: falon tizimdagi klaviatura xatoligi tuzatildi`
   - `docs: README yoki qo'llanmalar yangilandi`
   - `perf: FSM kechikish vaqti optimallashtirildi`
   - `refactor: kod arxitekturasi tozalandi`

6. **Pull Request (PR) yuboring:**
   - O'zgarishlaringizni o'z fork'ingizga push qiling:
     ```bash
     git push origin feature/yangi-imkoniyat
     ```
   - Asosiy repozitoriyga PR oching.
   - PR tavsifida nima o'zgargani, nima uchun kerakligi va qanday test qilinganini batafsil bayon qiling.

---

## 💬 Xatoliklar (Issues) va Takliflar

Agar dasturda xatolik topsangiz yoki taklifingiz bo'lsa:
1. Mavjud [Issues](https://github.com/0605AbMu/harfiy/issues) orasida xuddi shu mavzu yo'qligini tekshiring.
2. Yangi Issue yarating va quyidagilarni ko'rsating:
   - Operatsion tizim va versiyasi (masalan: macOS 15.1, Windows 11 23H2, Ubuntu 24.04).
   - Harfiy versiyasi (masalan: `0.8.0`).
   - Xatolikni takrorlash qadamlari (reproduction steps).
   - Kutilgan natija va haqiqatda nima sodir bo'lgani.

---

## 📜 Litsenziya

Loyiha doirasida taqdim etilgan barcha hissalar [MIT License](LICENSE) shartlari asosida qabul qilinadi.
