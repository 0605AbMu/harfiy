# Harfiy Privacy Policy

**Effective Date:** October 25, 2026  
**Application Name:** Harfiy  
**Package Name:** uz.harfiy.keyboard  
**Developer:** Abdumannon Shamsiyev  
**Contact:** [@abdumannon_shamsiyev](https://t.me/abdumannon_shamsiyev) | https://abdumannon-shamsiyev.uz/harfiy  

---

### 1. Overview
Harfiy ("we", "our", or "app") is dedicated to protecting user privacy. Harfiy is an offline text input utility designed for Uzbek Latin script. It allows users to automatically transform digraph combinations ("sh", "ch", "o'", "g'") into modern Uzbek Latin letters ("ş", "ç", "ö", "ğ") as they type using their default device keyboard (e.g., Gboard, Samsung Keyboard).

---

### 2. AccessibilityService API Usage & Prominent Disclosure
To deliver its core input transformation feature, Harfiy uses Android's **AccessibilityService API**.

* **Purpose:** Harfiy uses the AccessibilityService solely to detect input events (`TYPE_VIEW_TEXT_CHANGED`, `TYPE_VIEW_FOCUSED`) in text fields and convert specific letter combinations ("sh", "ch", "o'", "g'") into standard Uzbek Latin characters ("ş", "ç", "ö", "ğ").
* **Data Access & Privacy:** The app does **NOT** log, record, harvest, or transmit any sensitive user data, including passwords, credit card numbers, personal messages, or contact lists.
* **100% Offline (No Internet Permission):** Harfiy does **NOT** request or possess the `android.permission.INTERNET` network permission. Technically and architecturaly, the application cannot transmit any data over the internet or communicate with external servers. All processing is executed strictly on-device using a high-performance local Rust engine.

---

### 3. Data Collection, Storage, and Sharing
* **Personal Data:** We collect **zero** personally identifiable information (PII).
* **Usage Statistics:** The app maintains a simple local counter (number of letters converted and estimated time saved) stored strictly on the user's device via `SharedPreferences`. This data never leaves the device and can be cleared at any time using the "Hisoblagichni tozalash" (Clear Counter) button.
* **Data Sharing:** No data is shared with third parties or advertising brokers.

---

### 4. Third-Party Libraries and Advertisements
* Harfiy contains **no advertisements**.
* Harfiy does **not** integrate third-party tracking, analytics, or behavioral profiling SDKs (e.g., Google Analytics, Firebase, Facebook SDK).

---

### 5. Children's Privacy
Harfiy does not collect information from children under the age of 13 and is compliant with COPPA and GDPR.

---

### 6. Control and Uninstallation
Users retain full control at all times:
1. Toggle the master switch inside the Harfiy app;
2. Toggle the Harfiy Quick Settings tile in the system notification shade;
3. Disable or uninstall the service via Android `Settings -> Accessibility`.

---

### 7. Contact
For any privacy questions or inquiries:
* **Telegram:** [@abdumannon_shamsiyev](https://t.me/abdumannon_shamsiyev)
* **Website:** [https://abdumannon-shamsiyev.uz/harfiy](https://abdumannon-shamsiyev.uz/harfiy)
