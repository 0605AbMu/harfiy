//! System tray icon and global hotkey controller for Harfiy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::accelerator::Accelerator;
use tray_icon::menu::{
    CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem,
    Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const TRAY_ACTIVE_32: &[u8] = include_bytes!("../../../assets/tray_active_32.rgba");
const TRAY_PAUSED_32: &[u8] = include_bytes!("../../../assets/tray_paused_32.rgba");

const MENU_BLOG_16: &[u8] = include_bytes!("../../../assets/menu_blog_16.rgba");
const MENU_WEB_16: &[u8] = include_bytes!("../../../assets/menu_web_16.rgba");
const MENU_SUPPORT_16: &[u8] = include_bytes!("../../../assets/menu_support_16.rgba");
const MENU_FOLDER_16: &[u8] = include_bytes!("../../../assets/menu_folder_16.rgba");
const MENU_REMOVE_16: &[u8] = include_bytes!("../../../assets/menu_remove_16.rgba");
const MENU_ADD_16: &[u8] = include_bytes!("../../../assets/menu_add_16.rgba");
const MENU_TRASH_16: &[u8] = include_bytes!("../../../assets/menu_trash_16.rgba");
const MENU_INFO_16: &[u8] = include_bytes!("../../../assets/menu_info_16.rgba");
const MENU_STATS_16: &[u8] = include_bytes!("../../../assets/menu_stats_16.rgba");
const MENU_MAGIC_16: &[u8] = include_bytes!("../../../assets/menu_magic_16.rgba");

fn make_menu_icon(rgba: &[u8]) -> Option<tray_icon::menu::Icon> {
    tray_icon::menu::Icon::from_rgba(rgba.to_vec(), 16, 16).ok()
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(' ');
        }
        result.push(ch);
    }
    result
}

fn calculate_time_saved(conversions: u64) -> u64 {
    ((conversions as f64 * 0.5) / 60.0).ceil() as u64
}

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
    QuickConvertText,
    ToggleNg(bool),
    ToggleAutostart(bool),
    ToggleGameMode(bool),
    Quit,
    PickExcludedAppExplorer,
    AddExcludedApp(String),
    RemoveExcludedApp(String),
    ClearAllExcludedApps,
    ClearStats,
    OpenWebPage,
    OpenBlog,
    OpenSupport,
    OpenPermissionSettings,
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

#[cfg(target_os = "macos")]
pub fn init_macos_app() {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
    }
}

#[cfg(target_os = "macos")]
pub fn pump_macos_events() {
    use objc2_app_kit::{NSApplication, NSEventMask};
    use objc2_foundation::{MainThreadMarker, NSDate, NSDefaultRunLoopMode};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            let pool = objc2_foundation::NSAutoreleasePool::new();
            while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&NSDate::distantPast()),
                NSDefaultRunLoopMode,
                true,
            ) {
                app.sendEvent(&event);
                app.updateWindows();
            }
            drop(pool);
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

    // 1. Pick via Native File Dialog with folder icon
    #[cfg(target_os = "windows")]
    let pick_label = "Fayldan tanlash (.exe)...";
    #[cfg(target_os = "macos")]
    let pick_label = "Dasturlardan tanlash (.app)...";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let pick_label = "Dasturni tanlash...";

    let pick_item = IconMenuItem::with_id(
        "pick_explorer",
        pick_label,
        true,
        make_menu_icon(MENU_FOLDER_16),
        None,
    );
    let _ = submenu.append(&pick_item);

    // 2. Running apps submenu
    let running_sub = Submenu::new("Ishlayotgan dasturlar", true);
    let mut added_any = false;
    for app in running_apps {
        let app_lower = app.to_lowercase();
        if !excluded_apps.iter().any(|e| e.to_lowercase() == app_lower) {
            let item = IconMenuItem::with_id(
                format!("add_running:{}", app_lower),
                app,
                true,
                make_menu_icon(MENU_ADD_16),
                None,
            );
            let _ = running_sub.append(&item);
            added_any = true;
        }
    }
    if !added_any {
        let empty_item = IconMenuItem::new(
            "(Boshqa dastur topilmadi)",
            false,
            make_menu_icon(MENU_INFO_16),
            None,
        );
        let _ = running_sub.append(&empty_item);
    }
    let _ = submenu.append(&running_sub);

    // 3. Separator
    let _ = submenu.append(&PredefinedMenuItem::separator());

    // 4. Current Excluded Apps list
    if excluded_apps.is_empty() {
        let empty_item = IconMenuItem::new(
            "Istisnolar yo'q (har joyda faol)",
            false,
            make_menu_icon(MENU_INFO_16),
            None,
        );
        let _ = submenu.append(&empty_item);
    } else {
        for app in excluded_apps {
            let item = IconMenuItem::with_id(
                format!("remove_excluded:{}", app.to_lowercase()),
                app,
                true,
                make_menu_icon(MENU_REMOVE_16),
                None,
            );
            let _ = submenu.append(&item);
        }

        // If there are 2 or more exclusions, show "Barcha istisnolarni tozalash"
        if excluded_apps.len() >= 2 {
            let _ = submenu.append(&PredefinedMenuItem::separator());
            let clear_all_item = IconMenuItem::with_id(
                "clear_all_excluded",
                "Barcha istisnolarni tozalash",
                true,
                make_menu_icon(MENU_TRASH_16),
                None,
            );
            let _ = submenu.append(&clear_all_item);
        }
    }
}

/// System tray and hotkey manager.
pub struct TrayController {
    tray_icon: TrayIcon,
    tray_menu: Menu,
    permission_item: IconMenuItem,
    permission_sep: PredefinedMenuItem,
    has_permission: Arc<AtomicBool>,
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: HotKey,
    convert_hotkey: Option<HotKey>,
    active_item: CheckMenuItem,
    convert_item: IconMenuItem,
    ng_item: CheckMenuItem,
    autostart_item: CheckMenuItem,
    game_mode_item: CheckMenuItem,
    excluded_submenu: Submenu,
    stats_count_item: IconMenuItem,
    stats_time_item: MenuItem,
    _version_item: MenuItem,
    web_item: IconMenuItem,
    blog_item: IconMenuItem,
    support_item: IconMenuItem,
    quit_item: MenuItem,
    is_active: Arc<AtomicBool>,
    enable_ng: Arc<AtomicBool>,
    autostart: Arc<AtomicBool>,
    game_mode: Arc<AtomicBool>,
    last_hotkey_press: Arc<Mutex<Instant>>,
    last_convert_hotkey_press: Arc<Mutex<Instant>>,
}

impl TrayController {
    /// Create and initialize the system tray icon, menu, and hotkey.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        initial_active: bool,
        has_permission: bool,
        current_version: &str,
        hotkey_str: &str,
        convert_hotkey_str: &str,
        initial_ng: bool,
        initial_autostart: bool,
        initial_game_mode: bool,
        total_conversions: u64,
        excluded_apps: &[String],
        running_apps: &[String],
    ) -> Result<Self, TrayError> {
        #[cfg(target_os = "macos")]
        init_macos_app();

        let is_active = Arc::new(AtomicBool::new(initial_active));
        let has_permission_arc = Arc::new(AtomicBool::new(has_permission));
        let enable_ng = Arc::new(AtomicBool::new(initial_ng));
        let autostart = Arc::new(AtomicBool::new(initial_autostart));
        let game_mode = Arc::new(AtomicBool::new(initial_game_mode));
        let last_hotkey_press = Arc::new(Mutex::new(
            Instant::now()
                .checked_sub(Duration::from_secs(5))
                .unwrap_or_else(Instant::now),
        ));
        let last_convert_hotkey_press = Arc::new(Mutex::new(
            Instant::now()
                .checked_sub(Duration::from_secs(5))
                .unwrap_or_else(Instant::now),
        ));

        // Parse accelerator for menu item hints
        let accelerator: Option<Accelerator> = hotkey_str.parse().ok().or_else(|| {
            Some(Accelerator::new(
                Some(
                    tray_icon::menu::accelerator::Modifiers::CONTROL
                        | tray_icon::menu::accelerator::Modifiers::SHIFT,
                ),
                tray_icon::menu::accelerator::Code::KeyU,
            ))
        });

        let convert_accelerator: Option<Accelerator> =
            convert_hotkey_str.parse().ok().or_else(|| {
                Some(Accelerator::new(
                    Some(
                        tray_icon::menu::accelerator::Modifiers::CONTROL
                            | tray_icon::menu::accelerator::Modifiers::ALT,
                    ),
                    tray_icon::menu::accelerator::Code::KeyU,
                ))
            });

        // Create simplified, elegant tray menu items
        let tray_menu = Menu::new();

        // 0. Permission warning item (visible if system accessibility permissions are missing)
        let permission_item = IconMenuItem::with_id(
            "grant_permission",
            "⚠️ Tizim ruxsatini berish (Accessibility)...",
            true,
            make_menu_icon(MENU_INFO_16),
            None,
        );
        let permission_sep = PredefinedMenuItem::separator();

        // 1. Active toggle checkbox with shortcut hint
        let active_item = CheckMenuItem::new("Faol (Harfiy)", true, initial_active, accelerator);

        // 2. Quick convert selection item
        let convert_item = IconMenuItem::with_id(
            "quick_convert",
            "Matnni o'girish",
            true,
            make_menu_icon(MENU_MAGIC_16),
            convert_accelerator,
        );

        // 3. Option checkboxes
        let ng_item = CheckMenuItem::new("'ng' -> 'ñ' qoidasi", true, initial_ng, None);
        let autostart_item =
            CheckMenuItem::new("Avtomatik ishga tushish", true, initial_autostart, None);
        let game_mode_item =
            CheckMenuItem::new("O'yinlarda avtomatik pauza", true, initial_game_mode, None);

        // 4. Excluded apps submenu (clean text)
        let excluded_submenu = Submenu::new("Istisno dasturlar", true);
        populate_excluded_submenu(&excluded_submenu, excluded_apps, running_apps);

        // 5. Statistics submenu
        let stats_submenu = Submenu::new("Statistika", true);
        let stats_count_item = IconMenuItem::new(
            format!(
                "Jami o'zgartirishlar: {} ta",
                format_number(total_conversions)
            ),
            false,
            make_menu_icon(MENU_STATS_16),
            None,
        );
        let minutes = calculate_time_saved(total_conversions);
        let stats_time_item =
            MenuItem::new(format!("Tejalgan vaqt: ~{} daqiqa", minutes), false, None);
        let stats_clear_item = IconMenuItem::with_id(
            "clear_stats",
            "Statistikani tozalash",
            true,
            make_menu_icon(MENU_TRASH_16),
            None,
        );
        let _ = stats_submenu.append_items(&[
            &stats_count_item,
            &stats_time_item,
            &PredefinedMenuItem::separator(),
            &stats_clear_item,
        ]);

        // 6. Current version label
        let version_item = MenuItem::new(format!("Harfiy v{}", current_version), false, None);

        // 7. Web & Telegram links: Web, Blog & Support with real icons and short text
        let web_item = IconMenuItem::with_id(
            "open_web",
            "Veb-sahifa",
            true,
            make_menu_icon(MENU_WEB_16),
            None,
        );
        let blog_item = IconMenuItem::with_id(
            "open_blog",
            "Blog",
            true,
            make_menu_icon(MENU_BLOG_16),
            None,
        );
        let support_item = IconMenuItem::with_id(
            "open_support",
            "Support",
            true,
            make_menu_icon(MENU_SUPPORT_16),
            None,
        );

        // 8. Quit item
        let quit_item = MenuItem::new("Chiqish", true, None);

        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();
        let sep3 = PredefinedMenuItem::separator();
        let sep4 = PredefinedMenuItem::separator();
        let sep5 = PredefinedMenuItem::separator();

        if !has_permission {
            tray_menu
                .append_items(&[&permission_item, &permission_sep])
                .map_err(|e| TrayError::TrayInit(e.to_string()))?;
        }

        tray_menu
            .append_items(&[
                &active_item,
                &convert_item,
                &sep1,
                &ng_item,
                &autostart_item,
                &game_mode_item,
                &sep2,
                &excluded_submenu,
                &stats_submenu,
                &sep3,
                &version_item,
                &sep4,
                &web_item,
                &blog_item,
                &support_item,
                &sep5,
                &quit_item,
            ])
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Load official logo icon
        let icon = get_app_icon(initial_active)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu.clone()))
            .with_tooltip(format!(
                "Harfiy v{} - O'zbek Alifbosi ({})",
                current_version, hotkey_str
            ))
            .with_icon(icon)
            .build()
            .map_err(|e| TrayError::TrayInit(e.to_string()))?;

        // Setup Global Hotkeys
        let hotkey_manager =
            GlobalHotKeyManager::new().map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let hotkey: HotKey = hotkey_str.parse().unwrap_or_else(|_| {
            HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyU)
        });

        hotkey_manager
            .register(hotkey)
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let convert_hotkey: Option<HotKey> = convert_hotkey_str.parse().ok().or_else(|| {
            Some(HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::ALT),
                Code::KeyU,
            ))
        });

        if let Some(chk) = convert_hotkey {
            let _ = hotkey_manager.register(chk);
        }

        Ok(Self {
            tray_icon,
            tray_menu,
            permission_item,
            permission_sep,
            has_permission: has_permission_arc,
            _hotkey_manager: hotkey_manager,
            hotkey,
            convert_hotkey,
            active_item,
            convert_item,
            ng_item,
            autostart_item,
            game_mode_item,
            excluded_submenu,
            stats_count_item,
            stats_time_item,
            _version_item: version_item,
            web_item,
            blog_item,
            support_item,
            quit_item,
            is_active,
            enable_ng,
            autostart,
            game_mode,
            last_hotkey_press,
            last_convert_hotkey_press,
        })
    }

    /// Dynamically update system permission status in the tray menu.
    /// If granted, removes the permission warning item from the top; if missing, prepends it.
    pub fn update_permission_status(&self, granted: bool) {
        let was_granted = self.has_permission.swap(granted, Ordering::SeqCst);
        if was_granted != granted {
            if granted {
                let _ = self.tray_menu.remove(&self.permission_item);
                let _ = self.tray_menu.remove(&self.permission_sep);
            } else {
                let _ = self.tray_menu.prepend(&self.permission_sep);
                let _ = self.tray_menu.prepend(&self.permission_item);
            }
        }
    }

    /// Check for tray menu or hotkey events (non-blocking).
    pub fn poll_events(&self) -> Option<TrayCommand> {
        #[cfg(target_os = "macos")]
        pump_macos_events();

        // Check global hotkey events (debounced 250ms for responsive toggle)
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() {
                let should_toggle = match event.state {
                    global_hotkey::HotKeyState::Pressed | global_hotkey::HotKeyState::Released => {
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
            } else if let Some(chk) = self.convert_hotkey {
                if event.id == chk.id() {
                    let should_convert = match event.state {
                        global_hotkey::HotKeyState::Pressed
                        | global_hotkey::HotKeyState::Released => {
                            let mut last = self.last_convert_hotkey_press.lock().unwrap();
                            if last.elapsed() >= Duration::from_millis(300) {
                                *last = Instant::now();
                                true
                            } else {
                                false
                            }
                        }
                    };

                    if should_convert {
                        return Some(TrayCommand::QuickConvertText);
                    }
                }
            }
        }

        // Check menu item clicks
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.permission_item.id() {
                return Some(TrayCommand::OpenPermissionSettings);
            } else if event.id == self.active_item.id() {
                let new_state = !self.is_active.load(Ordering::SeqCst);
                self.update_state(new_state);
                return Some(TrayCommand::ToggleActive(new_state));
            } else if event.id == self.convert_item.id() {
                return Some(TrayCommand::QuickConvertText);
            } else if event.id == self.ng_item.id() {
                let new_state = !self.enable_ng.load(Ordering::SeqCst);
                self.enable_ng.store(new_state, Ordering::SeqCst);
                self.ng_item.set_checked(new_state);
                return Some(TrayCommand::ToggleNg(new_state));
            } else if event.id == self.autostart_item.id() {
                let new_state = !self.autostart.load(Ordering::SeqCst);
                self.autostart.store(new_state, Ordering::SeqCst);
                self.autostart_item.set_checked(new_state);
                return Some(TrayCommand::ToggleAutostart(new_state));
            } else if event.id == self.game_mode_item.id() {
                let new_state = !self.game_mode.load(Ordering::SeqCst);
                self.game_mode.store(new_state, Ordering::SeqCst);
                self.game_mode_item.set_checked(new_state);
                return Some(TrayCommand::ToggleGameMode(new_state));
            } else if event.id == self.web_item.id() {
                return Some(TrayCommand::OpenWebPage);
            } else if event.id == self.blog_item.id() {
                return Some(TrayCommand::OpenBlog);
            } else if event.id == self.support_item.id() {
                return Some(TrayCommand::OpenSupport);
            } else if event.id == self.quit_item.id() {
                return Some(TrayCommand::Quit);
            } else {
                let id_str = event.id.as_ref();
                if id_str == "pick_explorer" {
                    return Some(TrayCommand::PickExcludedAppExplorer);
                } else if id_str == "clear_all_excluded" {
                    return Some(TrayCommand::ClearAllExcludedApps);
                } else if id_str == "clear_stats" {
                    return Some(TrayCommand::ClearStats);
                } else if let Some(app) = id_str.strip_prefix("remove_excluded:") {
                    return Some(TrayCommand::RemoveExcludedApp(app.to_string()));
                } else if let Some(app) = id_str.strip_prefix("add_running:") {
                    return Some(TrayCommand::AddExcludedApp(app.to_string()));
                } else if id_str == "open_web" {
                    return Some(TrayCommand::OpenWebPage);
                } else if id_str == "open_blog" {
                    return Some(TrayCommand::OpenBlog);
                } else if id_str == "open_support" {
                    return Some(TrayCommand::OpenSupport);
                }
            }
        }

        None
    }

    /// Update the excluded apps submenu dynamically.
    pub fn update_excluded_apps(&self, excluded_apps: &[String], running_apps: &[String]) {
        populate_excluded_submenu(&self.excluded_submenu, excluded_apps, running_apps);
    }

    /// Update stats display dynamically in the menu.
    pub fn update_stats(&self, conversions: u64) {
        self.stats_count_item.set_text(format!(
            "Jami o'zgartirishlar: {} ta",
            format_number(conversions)
        ));
        let minutes = calculate_time_saved(conversions);
        self.stats_time_item
            .set_text(format!("Tejalgan vaqt: ~{} daqiqa", minutes));
    }

    /// Update 'ng' rule toggle state.
    pub fn set_ng_checked(&self, enabled: bool) {
        self.enable_ng.store(enabled, Ordering::SeqCst);
        self.ng_item.set_checked(enabled);
    }

    /// Update autostart checkbox state.
    pub fn set_autostart_checked(&self, enabled: bool) {
        self.autostart.store(enabled, Ordering::SeqCst);
        self.autostart_item.set_checked(enabled);
    }

    /// Update game mode checkbox state.
    pub fn set_game_mode_checked(&self, enabled: bool) {
        self.game_mode.store(enabled, Ordering::SeqCst);
        self.game_mode_item.set_checked(enabled);
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
    convert_hotkey: Option<HotKey>,
    last_press: Mutex<Instant>,
    last_convert_press: Mutex<Instant>,
}

impl StandaloneHotKey {
    pub fn new(hotkey_str: &str, convert_hotkey_str: &str) -> Result<Self, TrayError> {
        let hotkey_manager =
            GlobalHotKeyManager::new().map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let hotkey: HotKey = hotkey_str.parse().unwrap_or_else(|_| {
            HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyU)
        });

        hotkey_manager
            .register(hotkey)
            .map_err(|e| TrayError::HotKeyInit(e.to_string()))?;

        let convert_hotkey: Option<HotKey> = convert_hotkey_str.parse().ok();
        if let Some(chk) = convert_hotkey {
            let _ = hotkey_manager.register(chk);
        }

        Ok(Self {
            _hotkey_manager: hotkey_manager,
            hotkey,
            convert_hotkey,
            last_press: Mutex::new(
                Instant::now()
                    .checked_sub(Duration::from_secs(5))
                    .unwrap_or_else(Instant::now),
            ),
            last_convert_press: Mutex::new(
                Instant::now()
                    .checked_sub(Duration::from_secs(5))
                    .unwrap_or_else(Instant::now),
            ),
        })
    }

    /// Poll for hotkey press events.
    pub fn poll_command(&self) -> Option<TrayCommand> {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() {
                let should_toggle = match event.state {
                    global_hotkey::HotKeyState::Pressed | global_hotkey::HotKeyState::Released => {
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
                    return Some(TrayCommand::ToggleActive(false));
                }
            } else if let Some(chk) = self.convert_hotkey {
                if event.id == chk.id() {
                    let should_convert = match event.state {
                        global_hotkey::HotKeyState::Pressed
                        | global_hotkey::HotKeyState::Released => {
                            let mut last = self.last_convert_press.lock().unwrap();
                            if last.elapsed() >= Duration::from_millis(300) {
                                *last = Instant::now();
                                true
                            } else {
                                false
                            }
                        }
                    };

                    if should_convert {
                        return Some(TrayCommand::QuickConvertText);
                    }
                }
            }
        }
        None
    }

    /// Poll for active toggle hotkey.
    pub fn poll(&self) -> bool {
        matches!(self.poll_command(), Some(TrayCommand::ToggleActive(_)))
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
    #[cfg_attr(
        target_os = "macos",
        ignore = "muda MenuChild requires macOS main thread"
    )]
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
