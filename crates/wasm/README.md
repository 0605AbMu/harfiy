# Harfiy WebAssembly Core (`harfiy-wasm`)

O‘zbek lotin alifbosi yangi qoidalarini (`sh` &rarr; `ş`, `ch` &rarr; `ç`, `o'` &rarr; `ö`, `g'` &rarr; `ğ`, `ng` &rarr; `ñ`) veb muhitida va Single Page Application (SPA) loyihalarida real-vaqtda ishlatish uchun rasmiy **WebAssembly** moduli.

Mazkur modul **Yagona Dvigatel (Single Source of Truth)** tamoyili asosida ishlaydi: hech qanday mantiq TypeScript yoki JavaScript da qayta yozilmagan. U to‘g‘ridan-to‘g‘ri asosiy Rust dvigateli (`crates/engine`) bilan ishlaydi.

---

## ⚡ Xususiyatlari

- 🎯 **100% Yagona Rust Engine:** Desktop, iOS, Android va Web aynan bitta `crates/engine` logikasidan foydalanadi.
- 🪶 **Yengil vazn:** Wasm moduli atigi ~38 KB (brotli/gzip bilan ~10 KB).
- 🔄 **Ikki xil rejim:**
  1. **Batch Convert:** Katta matnlarni bir zumda yangi alifboga o‘girish va statistika olish.
  2. **Real-time Typing (FSM):** Klaviaturada yozish bilan bir onda harflarni almashtirish va **Instant Undo** (Backspace bosilganda asl harflarni tiklash).
- 🧩 **Tayyor DOM Adapter (`attachHarfiy`):** Har qanday `<input>` yoki `<textarea>` ga bitta qator kod bilan ulanadi (React, Vue, Svelte yoki Vanilla JS).

---

## 📦 O‘rnatish (GitHub Packages orqali)

Loyiha GitHub Packages npm registriga (`@0605abmu/harfiy-wasm`) joylanadi.

SPAngiz papkasida `.npmrc` faylini yarating yoki mavjudiga qo‘shing:
```ini
@0605abmu:registry=https://npm.pkg.github.com
```

So‘ngra paketni o‘rnating:
```bash
npm install @0605abmu/harfiy-wasm
# yoki
pnpm add @0605abmu/harfiy-wasm
# yoki
yarn add @0605abmu/harfiy-wasm
```

---

## 🛠️ Mahalliy Yig‘ish (Local Build)

Loyihaning ildiz papkasidan:

```bash
./scripts/build_web.sh
```

Natijada `crates/wasm/pkg` papkasida quyidagilar hosil bo‘ladi:
- `harfiy_wasm_bg.wasm` — WebAssembly binar moduli.
- `harfiy_wasm.js` — ESM yuklovchi skript.
- `harfiy_wasm.d.ts` — To‘liq TypeScript ta’riflari.
- `adapter.js` va `adapter.d.ts` — DOM bilan bog‘lovchi yengil adapter.

---

## 🚀 SPA Loyihalarida Foydalanish

### 1. Modulni yuklash (Init)

```ts
import init, {
  convert_text,
  convert_text_with_stats,
  WasmHarfiyEngine,
} from '@0605abmu/harfiy-wasm';

// Wasm modulini 1 marta ishga tushirish:
await init();
```

---

### 2. Katta Matnni O‘girish (Batch Convert)

```ts
// Oddiy o'girish:
const yangiMatn = convert_text("O'zbekiston shoshilma!", false, true);
console.log(yangiMatn); // "Özbekiston şoşilma!"

// Statistika bilan birga olish:
const stats = convert_text_with_stats("Shahar markazida choyxona bor", false, true);
console.log(stats.text); // "Şahar markazida çoyxona bor"
console.log(stats.conversions_count); // 2 ta harf o'zgartirildi
```

---

### 3. Real-vaqtda Klaviaturaga Ulash (`attachHarfiy`)

React, Vue, Svelte yoki Vanilla HTML maydoniga bitta qatorda ulanadi:

```ts
import init, { WasmHarfiyEngine } from '@0605abmu/harfiy-wasm';
import { attachHarfiy } from '@0605abmu/harfiy-wasm/adapter';

await init();

const textarea = document.querySelector('textarea');

// Ulash:
const detach = attachHarfiy(textarea, {
  enableNg: false, // 'ng' -> 'ñ' yoqish/o'chirish
  onTransform: (replacement, trigger) => {
    console.log(`Almashtirildi: ${trigger} -> ${replacement}`);
  },
});

// Kerak bo'lmaganda (masalan, React unmount paytida):
// detach();
```

---

### 4. React Komponentida Namunasi

```tsx
import React, { useEffect, useRef } from 'react';
import init from '@0605abmu/harfiy-wasm';
import { attachHarfiy } from '@0605abmu/harfiy-wasm/adapter';

export function HarfiyInput() {
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    let detach: (() => void) | undefined;

    init().then(() => {
      if (inputRef.current) {
        detach = attachHarfiy(inputRef.current);
      }
    });

    return () => {
      detach?.();
    };
  }, []);

  return (
    <textarea
      ref={inputRef}
      rows={4}
      placeholder="Yozing: sh, ch, o', g'..."
    />
  );
}
```

---

### 5. Qo‘lda FSM Boshqarish (Low-level WasmHarfiyEngine)

Agar o‘z shaxsiy matn muharriringiz yoki maxsus UI komponentingiz bo‘lsa:

```ts
const engine = new WasmHarfiyEngine(false, true);

// 's' bosilganda:
let action = engine.process_char('s');
// action.is_pass_through() === true;
// engine.state_name() === "waitingForSecondChar";

// 'h' bosilganda:
action = engine.process_char('h');
// action.is_transform() === true;
// action.backspaces_count === 1;
// action.replacement === "ş";

// Instant Undo: Darhol Backspace bosilganda:
let undo = engine.process_backspace();
// undo.is_transform() === true;
// undo.replacement === "sh";
```

---

## 🧪 Sinab Ko‘rish (Interactive Demo)

`crates/wasm/demo/index.html` faylini brauzerda ochib (masalan, `npx serve crates/wasm`), real-vaqtda FSM holatlari va Instant Undo qanday ishlashini darhol sinab ko‘rishingiz mumkin.
