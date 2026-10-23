//! System tray icon and global hotkey controller for Harfiy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::accelerator::Accelerator;
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

// Embedded high-quality official Harfiy icons in 32x32 RGBA
const TRAY_ACTIVE_32: &[u8] = include_bytes!("../../../assets/tray_active_32.rgba");
const TRAY_PAUSED_32: &[u8] = include_bytes!("../../../assets/tray_paused_32.rgba");

#[derive(Debug, thiserror::Error)]
pub enum TrayError {
    #[error("Failed to initialize tray icon: {0}")]
    TrayInit(String),

    #[error("Failed to register global hotkey: {0}")]
    HotKeyInit(String),

    #[error("Icon generation error: {0}")]
    IconError(String),
}

/// Commands triggered from tray menu or hotkeys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleActive(bool),
    CheckUpdates,
    DownloadUpdate,
    Quit,
    PickExcludedAppExplorer,
    AddExcludedApp(String),
    RemoveExcludedApp(String),
}

/// Remove all existing items from a Submenu.
fn clear_submenu(submenu: &Submenu) {
    for it in submenu.items() {
        match it {
            MenuItemKind::MenuItem(i) => {
                let _ = submenu.remove(&i);
            }
            MenuItemKind::Submenu(s) => {
                let _ = submenu.remove(&s);
            }
            MenuItemKind::Predefined(p) => {
                let _ = submenu.remove(&p);
            }
            MenuItemKind::Check(c) => {
                let _ = submenu.remove(&c);
            }
            MenuItemKind::Icon(ic) => {
                let _ = submenu.remove(&ic);
            }
        }
    }
}

/// Populate the excluded apps submenu with explorer picker, running apps, and active exclusions.
pub fn populate_excluded_submenu(
    submenu: &Submenu,
    excluded_apps: &[String],
    running_apps: &[String],
) {
    clear_submenu(submenu);

    // 1. Pick via Native Windows Explorer
    let pick_item = MenuItem::with_id(
        "pick_explorer",
        "📂 Fayldan tanlash (.exe)...",
        true,
        None,
    );
    let _ = submenu.append(&pick_item);

    // 2. Running apps submenu
    let running_sub = Submenu::new("📱 Ishlayotgan dasturlar", true);
    let mut added_any = false;
    for app in running_apps {
        let app_lower = app.to_lowercase();
        if !excluded_apps.iter().any(|e| e.to_lowercase() == app_lower) {
            let item = MenuItem::with_id(
                format!("add_running:{}", app_lower),
                format!("➕ {}", app),
                true,
                None,
            );
            let _ = running_sub.append(&item);
            added_any = true;
        }
    }
    if !added_any {
        let empty_item = MenuItem::new("(Boshqa dastur topilmadi)", false, None);
        let _ = running_sub.append(&empty_item);
    }
    let _ = submenu.append(&running_sub);

    // 3. Separator
    let _ = submenu.append(&PredefinedMenuItem::separator());

    // 4. Current Excluded Apps list
    if excluded_apps.is_empty() {
        let empty_item = MenuItem::new("ℹ️ Istisnolar yo'q (barcha joyda faol)", false, None);
        let _ = submenu.append(&empty_item);
    } else {
        for app in excluded_apps {
            let item = MenuItem::with_id(
                format!("remove_excluded:{}", app.to_lowercase()),
                format!("❌ {} (Olib tashlash)", app),
                true,
                None,
            );
            let _ = submenu.append(&item);
        }
    }
}

/// System tray and hotkey manager.
pub struct TrayController {
    tray_icon: TrayIcon,
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: HotKey,
    active_item: CheckMenuItem,
    excluded_submenu: Submenu,
    _version_item: MenuItem,
    update_item: MenuItem,
    quit_item: MenuItem,
    is_active: Arc<AtomicBool>,
    has_update: Arc<AtomicBool>,
    last_hotkey_press: Arc<Mutex<Instant>>,
}

impl TrayController {
    /// Create and initialize the system tray icon, menu, and hotkey.
    pub fn new(
        initial_active: bool,
        current_version: &str,
        hotkey_str: &str,
        excluded_apps: &[String],
        running_apps: &[String],
    ) -> Result<Self, TrayError> {
        let is_active = Arc::new(AtomicBool::new(initial_active));
        let has_update = Arc::new(AtomicBool::new(false));
        let last_hotkey_press = Arc::new(Mutex::new(
            Instant::now().checked_sub(Duration::from_secs(5)).unwrap_or_else(Instant::now),
        ));

        // Parse accelerator for menu item hint
        let accelerator: Option<Accelerator> = hotkey_str
            .parse()
            .ok()
            .or_else(|| {
                Some(Accelerator::new(
                    Some(tray_icon::menu::accelerator::Modifiers::CONTROL | tray_icon::menu::accelerator::Modifiers::SHIFT),
                    tray_icon::menu::accelerator::Code::KeyU,
                ))
            });

        // Create simplified, elegant tray menu items
        let tray_menu = Menu::new();

        // 1. Active toggle checkbox with shortcut hint
        let active_item = CheckMenuItem::new("Faol (Harfiy)", true, initial_active, accelerator);

        // 2. Excluded apps submenu
        let excluded_submenu = Submenu::new("🚫 Istisno dasturlar", true);
        populate_excluded_submenu(&excluded_submenu, excluded_apps, running_apps);

        // 3. Current version label
        let version_item = MenuItem::new(format!("Harfiy v{}", current_version), false, None);

        // 4. Dynamic single update item
        let update_item = MenuItem::new("Yangilanishlarni tekshirish", true, None);

        // 5. Quit item
        let quit_item = MenuItem::new("Chiqish", true, None);

        tray_menu
            .append_items(&[
                &active_item,
                &excluded_submenu,
                &PredefinedMenuItem::separator(),
                &version_item,
                &update_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Load official logo icon
        let icon = get_app_icon(initial_active)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(format!(
                "Harfiy v{} - O'zbek Alifbosi ({})",
                current_version, hotkey_str
            ))
            .with_icon(icon)
            .build()
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Setup Global Hotkey
        let hotkey_manager = GlobalHotKeyManager::new()
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let hotkey: HotKey = hotkey_str
            .parse()
            .unwrap_or_else(|_| HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyU));

        hotkey_manager
            .register(hotkey)
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        Ok(Self {
            tray_icon,
            _hotkey_manager: hotkey_manager,
            hotkey,
            active_item,
            excluded_submenu,
            _version_item: version_item,
            update_item,
            quit_item,
            is_active,
            has_update,
            last_hotkey_press,
        })
    }

    /// Check for tray menu or hotkey events (non-blocking).
    pub fn poll_events(&self) -> Option<TrayCommand> {
        // Check global hotkey events (debounced 250ms for responsive, single toggle)
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() {
                let should_toggle = match event.state {
                    global_hotkey::HotKeyState::Pressed => {
                        let mut last = self.last_hotkey_press.lock().unwrap();
                        if last.elapsed() >= Duration::from_millis(250) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    }
                    global_hotkey::HotKeyState::Released => {
                        // Fallback in case Pressed was not delivered
                        let mut last = self.last_hotkey_press.lock().unwrap();
                        if last.elapsed() >= Duration::from_millis(250) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    }
                };

                if should_toggle {
                    let new_state = !self.is_active.load(Ordering::SeqCst);
                    self.update_state(new_state);
                    return Some(TrayCommand::ToggleActive(new_state));
                }
            }
        }

        // Check menu item clicks
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.active_item.id() {
                let new_state = !self.is_active.load(Ordering::SeqCst);
                self.update_state(new_state);
                return Some(TrayCommand::ToggleActive(new_state));
            } else if event.id == self.update_item.id() {
                if self.has_update.load(Ordering::SeqCst) {
                    return Some(TrayCommand::DownloadUpdate);
                } else {
                    return Some(TrayCommand::CheckUpdates);
                }
            } else if event.id == self.quit_item.id() {
                return Some(TrayCommand::Quit);
            } else {
                let id_str = event.id.as_ref();
                if id_str == "pick_explorer" {
                    return Some(TrayCommand::PickExcludedAppExplorer);
                } else if let Some(app) = id_str.strip_prefix("remove_excluded:") {
                    return Some(TrayCommand::RemoveExcludedApp(app.to_string()));
                } else if let Some(app) = id_str.strip_prefix("add_running:") {
                    return Some(TrayCommand::AddExcludedApp(app.to_string()));
                }
            }
        }

        None
    }

    /// Update the excluded apps submenu dynamically.
    pub fn update_excluded_apps(&self, excluded_apps: &[String], running_apps: &[String]) {
        populate_excluded_submenu(&self.excluded_submenu, excluded_apps, running_apps);
    }

    /// Update tray menu when a new version is discovered.
    pub fn set_update_available(&self, latest_version: &str) {
        self.has_update.store(true, Ordering::SeqCst);
        self.update_item
            .set_text(format!("⭐ Yangilash (v{})", latest_version));
        self.update_item.set_enabled(true);
        let _ = self.tray_icon.set_tooltip(Some(format!(
            "Harfiy - Yangi versiya mavjud: v{}!",
            latest_version
        )));
    }

    /// Update tray menu when the installed version is already the latest.
    pub fn set_up_to_date(&self) {
        self.has_update.store(false, Ordering::SeqCst);
        self.update_item
            .set_text("✅ Eng so'nggi versiya");
        self.update_item.set_enabled(true);
        let _ = self.tray_icon.set_tooltip(Some("Harfiy - Eng so'nggi versiya o'rnatilgan"));
    }

    /// Update tray menu while an update check is in progress.
    pub fn set_checking_updates(&self) {
        self.update_item
            .set_text("⏳ Tekshirilmoqda...");
        self.update_item.set_enabled(false);
        let _ = self.tray_icon.set_tooltip(Some("Harfiy - Yangilanishlar tekshirilmoqda..."));
    }

    /// Internal helper to update state, check item, and change icon.
    fn update_state(&self, active: bool) {
        self.is_active.store(active, Ordering::SeqCst);
        self.active_item.set_checked(active);
        if let Ok(icon) = get_app_icon(active) {
            let _ = self.tray_icon.set_icon(Some(icon));
        }
    }

    /// Update the current active status externally.
    pub fn set_active(&self, active: bool) {
        self.update_state(active);
    }

    /// Query the current active status.
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }
}

/// Standalone global hotkey listener for headless or non-tray mode.
pub struct StandaloneHotKey {
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: HotKey,
    last_press: Mutex<Instant>,
}

impl StandaloneHotKey {
    pub fn new(hotkey_str: &str) -> Result<Self, TrayError> {
        let hotkey_manager = GlobalHotKeyManager::new()
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let hotkey: HotKey = hotkey_str
            .parse()
            .unwrap_or_else(|_| HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyU));

        hotkey_manager
            .register(hotkey)
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        Ok(Self {
            _hotkey_manager: hotkey_manager,
            hotkey,
            last_press: Mutex::new(
                Instant::now().checked_sub(Duration::from_secs(5)).unwrap_or_else(Instant::now),
            ),
        })
    }

    /// Poll for hotkey press events. Returns true if hotkey was triggered.
    pub fn poll(&self) -> bool {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() {
                let should_toggle = match event.state {
                    global_hotkey::HotKeyState::Pressed => {
                        let mut last = self.last_press.lock().unwrap();
                        if last.elapsed() >= Duration::from_millis(250) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    }
                    global_hotkey::HotKeyState::Released => {
                        let mut last = self.last_press.lock().unwrap();
                        if last.elapsed() >= Duration::from_millis(250) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    }
                };

                if should_toggle {
                    return true;
                }
            }
        }
        false
    }
}

/// Loads the official Harfiy icon in 32x32 RGBA format.
fn get_app_icon(active: bool) -> Result<Icon, TrayError> {
    const SIZE: u32 = 32;
    let bytes = if active {
        TRAY_ACTIVE_32
    } else {
        TRAY_PAUSED_32
    };

    Icon::from_rgba(bytes.to_vec(), SIZE, SIZE).map_err(|e| TrayError::IconError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_populate_excluded_submenu() {
        let sub = Submenu::new("Istisno dasturlar", true);
        let excluded = vec!["code.exe".to_string()];
        let running = vec!["code.exe".to_string(), "telegram.exe".to_string()];

        populate_excluded_submenu(&sub, &excluded, &running);
        assert!(!sub.items().is_empty());

        // Re-populating with empty excluded list
        populate_excluded_submenu(&sub, &[], &running);
        assert!(!sub.items().is_empty());
    }
}
