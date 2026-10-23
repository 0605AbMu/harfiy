//! System tray icon and global hotkey controller for Harfiy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::accelerator::Accelerator;
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

        // 2. Current version label
        let version_item = MenuItem::new(format!("Harfiy v{}", current_version), false, None);

        // 3. Dynamic single update item
        let update_item = MenuItem::new("Yangilanishlarni tekshirish", true, None);

        // 4. Quit item
        let quit_item = MenuItem::new("Chiqish", true, None);

        tray_menu
            .append_items(&[
                &active_item,
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
            }
        }

        None
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
