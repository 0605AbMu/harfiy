//! System tray icon and global hotkey controller for Harfiy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
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
    OpenSettings,
    CheckUpdates,
    DownloadUpdate,
    Quit,
}

/// System tray and hotkey manager.
pub struct TrayController {
    tray_icon: TrayIcon,
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: HotKey,
    active_item: CheckMenuItem,
    update_info_item: MenuItem,
    update_action_item: MenuItem,
    check_updates_item: MenuItem,
    quit_item: MenuItem,
    is_active: Arc<AtomicBool>,
}

impl TrayController {
    /// Create and initialize the system tray icon, menu, and hotkey (Ctrl + Shift + U).
    pub fn new(initial_active: bool, current_version: &str) -> Result<Self, TrayError> {
        let is_active = Arc::new(AtomicBool::new(initial_active));

        // Create tray menu items
        let tray_menu = Menu::new();

        // 1. Version header (displays current version e.g. "Harfiy v0.1.0")
        let version_item = MenuItem::new(format!("Harfiy v{} (Joriy)", current_version), false, None);

        // 2. Update info item (displays update status / new version)
        let update_info_item = MenuItem::new("✅ Eng so'nggi versiya", false, None);

        // 3. Update action item (Download update button, enabled when update is available)
        let update_action_item = MenuItem::new("🚀 Yangilash", false, None);

        // 4. Manual check updates button
        let check_updates_item = MenuItem::new("🔄 Yangilanishlarni tekshirish", true, None);

        // 5. Active toggle checkbox
        let active_item = CheckMenuItem::new("Faol (Harfiy)", true, initial_active, None);

        // 6. Settings and Quit
        let settings_item = MenuItem::new("Sozlamalar (config.toml)", false, None);
        let quit_item = MenuItem::new("Chiqish", true, None);

        tray_menu
            .append_items(&[
                &version_item,
                &update_info_item,
                &update_action_item,
                &check_updates_item,
                &PredefinedMenuItem::separator(),
                &active_item,
                &PredefinedMenuItem::separator(),
                &settings_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Load official logo icon
        let icon = get_app_icon(initial_active)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Harfiy - O'zbek Alifbosi Avto-Konvertatsiya")
            .with_icon(icon)
            .build()
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Setup Global Hotkey: Ctrl + Shift + U
        let hotkey_manager = GlobalHotKeyManager::new()
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyU);
        hotkey_manager
            .register(hotkey)
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        Ok(Self {
            tray_icon,
            _hotkey_manager: hotkey_manager,
            hotkey,
            active_item,
            update_info_item,
            update_action_item,
            check_updates_item,
            quit_item,
            is_active,
        })
    }

    /// Check for tray menu or hotkey events (non-blocking).
    pub fn poll_events(&self) -> Option<TrayCommand> {
        // Check global hotkey events
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() && event.state == global_hotkey::HotKeyState::Released {
                let new_state = !self.is_active.load(Ordering::SeqCst);
                self.update_state(new_state);
                return Some(TrayCommand::ToggleActive(new_state));
            }
        }

        // Check menu item clicks
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.active_item.id() {
                let new_state = !self.is_active.load(Ordering::SeqCst);
                self.update_state(new_state);
                return Some(TrayCommand::ToggleActive(new_state));
            } else if event.id == self.check_updates_item.id() {
                return Some(TrayCommand::CheckUpdates);
            } else if event.id == self.update_action_item.id() {
                return Some(TrayCommand::DownloadUpdate);
            } else if event.id == self.quit_item.id() {
                return Some(TrayCommand::Quit);
            }
        }

        None
    }

    /// Update tray menu when a new version is discovered.
    pub fn set_update_available(&self, latest_version: &str) {
        self.update_info_item
            .set_text(format!("⭐ Yangi versiya: v{}", latest_version));
        self.update_action_item
            .set_text(format!("🚀 Yangilashni yuklab olish (v{})", latest_version));
        self.update_action_item.set_enabled(true);
    }

    /// Update tray menu when the installed version is already the latest.
    pub fn set_up_to_date(&self) {
        self.update_info_item
            .set_text("✅ Eng so'nggi versiya o'rnatilgan");
        self.update_action_item.set_text("🚀 Yangilash");
        self.update_action_item.set_enabled(false);
    }

    /// Update tray menu while an update check is in progress.
    pub fn set_checking_updates(&self) {
        self.update_info_item
            .set_text("⏳ Yangilanishlar tekshirilmoqda...");
        self.update_action_item.set_enabled(false);
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
