#![windows_subsystem = "windows"]

//! Harfiy Background Daemon & CLI
//! Real-time Uzbek Latin orthography conversion engine.

mod config;
mod updater;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};
use harfiy_engine::{EngineConfig, HarfiyEngine};
#[cfg(windows)]
use harfiy_platform::{pump_messages, KeyboardInterceptor, WindowsInterceptor};
#[cfg(target_os = "macos")]
use harfiy_platform::{KeyboardInterceptor, MacOsInterceptor};
#[cfg(target_os = "linux")]
use harfiy_platform::{KeyboardInterceptor, LinuxInterceptor};
use harfiy_tray::{StandaloneHotKey, TrayCommand, TrayController};

use crate::config::AppConfig;
use crate::updater::UpdateInfo;

#[derive(Parser)]
#[command(name = "harfiy")]
#[command(author = "Abdumannon Shamsiyev")]
#[command(version)]
#[command(about = "Real-time Uzbek Latin orthography auto-conversion daemon", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Run in headless mode without system tray icon
    #[arg(long, default_value_t = false)]
    no_tray: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the conversion daemon (default)
    Start {
        #[arg(long, default_value_t = false)]
        no_tray: bool,
    },
    /// Check daemon status
    Status,
    /// Stop running daemon instance
    Stop,
    /// Check for updates from GitHub Releases
    #[command(alias = "update")]
    CheckUpdates,
}

fn main() {
    harfiy_platform::attach_parent_console();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Harfiy daemon status: Ready / Standby.");
        }
        Some(Commands::Stop) => {
            #[cfg(windows)]
            {
                println!("Fonda ishlayotgan Harfiy xizmatiga to'xtatish signali yuborilmoqda...");
                let _ = harfiy_platform::WindowsInterceptor::stop_hook();
                println!("To'xtatish signali yuborildi.");
            }
            #[cfg(not(windows))]
            {
                println!("Signal yuborildi.");
            }
        }
        Some(Commands::CheckUpdates) => {
            run_check_updates_cli();
        }
        Some(Commands::Start { no_tray }) => {
            run_daemon(no_tray);
        }
        None => {
            run_daemon(cli.no_tray);
        }
    }
}

fn run_check_updates_cli() {
    let (cfg, _) = AppConfig::load_or_create();
    let current_ver = env!("CARGO_PKG_VERSION");
    println!("Harfiy v{} - GitHub Releases orqali yangilanishlar tekshirilmoqda...", current_ver);
    println!("GitHub repozitoriysi: {}", cfg.github_repo);

    match updater::check_for_updates(&cfg.github_repo) {
        Ok(info) => {
            println!("Joriy versiya: v{}", info.current_version);
            println!("Eng so'nggi versiya: v{}", info.latest_version);
            if info.has_update {
                println!("\n⭐ Yangi versiya mavjud: v{}!", info.latest_version);
                println!("Yuklab olish havolasi: {}", info.release_url);
                println!("Dasturni o'z-o'zidan yangilashni xohlaysizmi? (h/y)");
                let mut buf = String::new();
                let _ = std::io::stdin().read_line(&mut buf);
                let choice = buf.trim().to_lowercase();
                if choice == "h" || choice == "ha" || choice == "y" || choice == "yes" {
                    println!("Yangilanish yuklab olinmoqda va o'rnatilmoqda...");
                    match updater::perform_self_update(&cfg.github_repo, true) {
                        Ok(new_ver) => {
                            println!("✅ Harfiy muvaffaqiyatli v{} versiyasiga yangilandi!", new_ver);
                        }
                        Err(e) => {
                            eprintln!("Yangilashda xatolik: {}", e);
                            println!("Brauzerda yuklab olish sahifasi ochilmoqda...");
                            let _ = open::that(&info.release_url);
                        }
                    }
                }
            } else {
                println!("\n✅ Siz eng so'nggi versiyadan foydalanmoqdasiz.");
            }
        }
        Err(e) => {
            eprintln!("Yangilanishni tekshirishda xatolik yuz berdi: {}", e);
        }
    }
}

fn open_telegram(domain: &str) {
    let deep_link = format!("tg://resolve?domain={}", domain);
    // 1. Try native Telegram deep link first (opens app directly if installed)
    if open::that(&deep_link).is_err() {
        // 2. Fallback to universal web link in default browser
        let web_url = format!("https://t.me/{}", domain);
        let _ = open::that(&web_url);
    }
}

fn perform_quick_convert(
    app_config: &mut AppConfig,
    tray_controller: Option<&TrayController>,
    conversions_counter: &Arc<AtomicU64>,
) {
    harfiy_platform::emit_copy();
    thread::sleep(Duration::from_millis(60));

    if let Some(text) = harfiy_platform::get_clipboard_text() {
        if !text.is_empty() {
            let (converted, count) = harfiy_engine::convert_text(
                &text,
                app_config.enable_ng,
                app_config.smart_apostrophe,
            );
            if count > 0 && converted != text {
                harfiy_platform::set_clipboard_text(&converted);
                harfiy_platform::emit_paste();

                let new_total = conversions_counter.fetch_add(count as u64, Ordering::SeqCst) + (count as u64);
                app_config.total_conversions = new_total;
                let _ = app_config.save();

                if let Some(tray) = tray_controller {
                    tray.update_stats(new_total);
                }

                harfiy_platform::show_custom_hud(
                    &format!("Matn o'girildi ({} ta harf)", count),
                    true,
                );
                println!("Tezkor matn o'girish bajarildi: {} ta harf o'zgartirildi.", count);
            } else {
                harfiy_platform::show_custom_hud("O'zgartiriladigan harflar topilmadi", false);
            }
        }
    }
}

fn run_daemon(no_tray: bool) {
    // Single instance protection: prevent duplicate hooks and installer file locks
    let _instance_guard = match harfiy_platform::acquire_single_instance_lock() {
        Some(guard) => guard,
        None => {
            println!("Harfiy allaqachon fonda ishlamoqda. Dasturning faqat bitta nusxasi ishlashi mumkin.");
            return;
        }
    };

    let (mut app_config, config_path) = AppConfig::load_or_create();
    let current_ver = env!("CARGO_PKG_VERSION");

    harfiy_platform::set_excluded_apps(&app_config.excluded_apps);
    harfiy_platform::set_game_mode(app_config.game_mode);
    let mut running_apps = harfiy_platform::scan_running_apps();
    let is_autostart = harfiy_platform::is_autostart_enabled();

    let conversions_counter = Arc::new(AtomicU64::new(app_config.total_conversions));

    println!("Harfiy v{} faollashtirilmoqda...", current_ver);
    println!("Sozlamalar fayli: {}", config_path.display());
    println!("Blog: https://t.me/abdumannon_notes");
    println!("Qo'llab-quvvatlash: https://t.me/abdumannon_shamsiyev");
    println!("Qoidalar: 'sh' -> 'ş', 'ch' -> 'ç', 'o''' -> 'ö', 'g''' -> 'ğ'");
    if app_config.enable_ng {
        println!("Qo'shimcha: 'ng' -> 'ñ' yoqilgan.");
    }
    println!("Tezkor tugma: Ctrl + Shift + U");
    println!("Matnni o'girish: {}", app_config.convert_hotkey);

    // Initialize engine
    let engine_inner = HarfiyEngine::new(EngineConfig {
        enable_ng: app_config.enable_ng,
        enabled: true,
    });

    let engine = Arc::new(Mutex::new(engine_inner));
    let engine_hook = Arc::clone(&engine);

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_ctrlc = Arc::clone(&is_running);

    // Setup Ctrl+C handler
    let _ = ctrlc::set_handler(move || {
        println!("\nChiqish signali qabul qilindi (Ctrl+C). Yakunlanmoqda...");
        is_running_ctrlc.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        {
            let _ = WindowsInterceptor::stop_hook();
        }
    });

    // Start platform interceptor on background worker thread
    #[cfg(windows)]
    let interceptor_thread = {
        let is_running_worker = Arc::clone(&is_running);
        let conversions_worker = Arc::clone(&conversions_counter);
        thread::spawn(move || {
            let mut interceptor = WindowsInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                let action = eng.process(event);
                if let harfiy_engine::KeyAction::Transform { .. } = action {
                    conversions_worker.fetch_add(1, Ordering::Relaxed);
                }
                action
            });

            if let Err(e) = res {
                eprintln!("Klaviatura kiritishini ushlashda xatolik: {}", e);
            }
            is_running_worker.store(false, Ordering::SeqCst);
        })
    };

    #[cfg(target_os = "macos")]
    let interceptor_thread = {
        let is_running_worker = Arc::clone(&is_running);
        let conversions_worker = Arc::clone(&conversions_counter);
        thread::spawn(move || {
            let mut interceptor = MacOsInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                let action = eng.process(event);
                if let harfiy_engine::KeyAction::Transform { .. } = action {
                    conversions_worker.fetch_add(1, Ordering::Relaxed);
                }
                action
            });

            if let Err(e) = res {
                eprintln!("Error running macOS interceptor: {}", e);
            }
            is_running_worker.store(false, Ordering::SeqCst);
        })
    };

    #[cfg(target_os = "linux")]
    let interceptor_thread = {
        let is_running_worker = Arc::clone(&is_running);
        let conversions_worker = Arc::clone(&conversions_counter);
        thread::spawn(move || {
            let mut interceptor = LinuxInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                let action = eng.process(event);
                if let harfiy_engine::KeyAction::Transform { .. } = action {
                    conversions_worker.fetch_add(1, Ordering::Relaxed);
                }
                action
            });

            if let Err(e) = res {
                eprintln!("Error running Linux interceptor: {}", e);
            }
            is_running_worker.store(false, Ordering::SeqCst);
        })
    };

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let interceptor_thread = thread::spawn(|| {});

    // System Tray Controller
    let initial_permission = harfiy_platform::is_permission_granted();
    let (mut tray_controller, standalone_hotkey) = if !no_tray {
        match TrayController::new(
            true,
            initial_permission,
            current_ver,
            &app_config.hotkey,
            &app_config.convert_hotkey,
            app_config.enable_ng,
            is_autostart,
            app_config.game_mode,
            app_config.total_conversions,
            &app_config.excluded_apps,
            &running_apps,
        ) {
            Ok(controller) => (Some(controller), None),
            Err(e) => {
                eprintln!(
                    "Tizim paneli (System Tray) yaratilmadi: {}. Konsol rejimida davom etilmoqda.",
                    e
                );
                (None, StandaloneHotKey::new(&app_config.hotkey, &app_config.convert_hotkey).ok())
            }
        }
    } else {
        (None, StandaloneHotKey::new(&app_config.hotkey, &app_config.convert_hotkey).ok())
    };

    struct UpdateEvent {
        result: Result<UpdateInfo, String>,
        is_manual: bool,
    }

    // Update checker channel & state
    let latest_update: Arc<Mutex<Option<UpdateInfo>>> = Arc::new(Mutex::new(None));
    let (tx_update, rx_update): (Sender<UpdateEvent>, Receiver<UpdateEvent>) = channel();

    // Trigger initial background update check if enabled
    if app_config.check_updates {
        let repo = app_config.github_repo.clone();
        let tx = tx_update.clone();
        thread::spawn(move || {
            let res = updater::check_for_updates(&repo);
            let _ = tx.send(UpdateEvent {
                result: res,
                is_manual: false,
            });
        });
    }

    println!("Harfiy muvaffaqiyatli ishga tushdi. To'xtatish uchun Ctrl+C bosing.");

    let mut last_scan = std::time::Instant::now();
    let mut last_perm_check = std::time::Instant::now();
    let mut last_perm_state = initial_permission;

    // Main event loop
    while is_running.load(Ordering::SeqCst) {
        #[cfg(windows)]
        pump_messages();

        #[cfg(target_os = "macos")]
        harfiy_tray::pump_macos_events();

        // Periodically verify system permission status (e.g. Accessibility in macOS Settings)
        if last_perm_check.elapsed() >= Duration::from_millis(1000) {
            last_perm_check = std::time::Instant::now();
            let perm_now = harfiy_platform::is_permission_granted();
            if perm_now != last_perm_state {
                last_perm_state = perm_now;
                if let Some(tray) = &tray_controller {
                    tray.update_permission_status(perm_now);
                }
                if perm_now {
                    println!("✅ Tizim ruxsati (Accessibility) berildi! Harfiy to'liq faollashdi.");
                    harfiy_platform::show_custom_hud("Harfiy: Tizim ruxsati berildi!", true);
                } else {
                    println!("⚠️ Tizim ruxsati berilmagan. Sozlamalardan ruxsat bering.");
                }
            }
        }

        // Periodically refresh running applications list for tray menu
        if last_scan.elapsed() >= Duration::from_secs(5) {
            last_scan = std::time::Instant::now();
            let new_running = harfiy_platform::scan_running_apps();
            if new_running != running_apps {
                running_apps = new_running;
                if let Some(tray) = &tray_controller {
                    tray.update_excluded_apps(&app_config.excluded_apps, &running_apps);
                }
            }
        }

        // Process asynchronous update notifications
        if let Ok(event) = rx_update.try_recv() {
            if let Some(tray) = &tray_controller {
                match event.result {
                    Ok(info) => {
                        if info.has_update {
                            println!("\n⭐ [YANGILANISH] Yangi versiya mavjud: v{}!", info.latest_version);
                            println!("Yuklab olish: {}", info.release_url);
                            tray.set_update_available(&info.latest_version);

                            if event.is_manual {
                                let msg = format!(
                                    "Harfiy dasturining yangi versiyasi mavjud!\n\nJoriy versiya: v{}\nYangi versiya: v{}\n\nIlovani hoziroq yangilashni xohlaysizmi?",
                                    info.current_version, info.latest_version
                                );
                                let repo_clone = app_config.github_repo.clone();
                                let download_url = info.release_url.clone();
                                thread::spawn(move || {
                                    if harfiy_platform::show_confirm_dialog("Harfiy - Yangilanish mavjud", &msg) {
                                        harfiy_platform::show_custom_hud("Yangilanish yuklab olinmoqda...", true);
                                        match updater::perform_self_update(&repo_clone, false) {
                                            Ok(new_ver) => {
                                                harfiy_platform::show_custom_hud(&format!("Harfiy v{} ga muvaffaqiyatli yangilandi!", new_ver), true);
                                                println!("Harfiy v{} ga muvaffaqiyatli yangilandi!", new_ver);
                                            }
                                            Err(e) => {
                                                eprintln!("Yangilashda xatolik: {}. Sahifa ochilmoqda...", e);
                                                let _ = open::that(&download_url);
                                            }
                                        }
                                    }
                                });
                            }
                        } else {
                            tray.set_up_to_date();
                            if event.is_manual {
                                let msg = format!(
                                    "Siz eng so'nggi versiyadan foydalanmoqdasiz (v{}).\nHech qanday yangilanish talab etilmaydi.",
                                    info.current_version
                                );
                                thread::spawn(move || {
                                    harfiy_platform::show_info_dialog("Harfiy", &msg);
                                });
                            }
                        }
                        *latest_update.lock().unwrap() = Some(info);
                    }
                    Err(e) => {
                        eprintln!("Yangilanishni tekshirishda xatolik: {}", e);
                        tray.set_up_to_date();
                        if event.is_manual {
                            let msg = format!(
                                "Yangilanishlarni tekshirishda xatolik yuz berdi:\n{}\n\nIltimos, internet aloqasini tekshiring.",
                                e
                            );
                            thread::spawn(move || {
                                harfiy_platform::show_info_dialog("Harfiy - Xatolik", &msg);
                            });
                        }
                    }
                }
            }
        }

        // Process standalone hotkey events (when running in headless/no-tray mode)
        if let Some(shk) = &standalone_hotkey {
            if let Some(cmd) = shk.poll_command() {
                match cmd {
                    TrayCommand::ToggleActive(_) => {
                        let mut eng = engine.lock().unwrap();
                        let active = !eng.is_enabled();
                        eng.set_enabled(active);
                        harfiy_platform::show_status_hud(active);
                        println!(
                            "Holat o'zgartirildi (Tezkor tugma): {}",
                            if active { "Faol" } else { "To'xtatilgan" }
                        );
                    }
                    TrayCommand::QuickConvertText => {
                        perform_quick_convert(
                            &mut app_config,
                            tray_controller.as_ref(),
                            &conversions_counter,
                        );
                    }
                    _ => {}
                }
            }
        }

        // Periodically sync real-time conversion stats
        let current_conversions = conversions_counter.load(Ordering::Relaxed);
        if current_conversions != app_config.total_conversions {
            app_config.total_conversions = current_conversions;
            let _ = app_config.save();
            if let Some(tray) = &tray_controller {
                tray.update_stats(current_conversions);
            }
        }

        // Process System Tray events
        if let Some(tray) = &tray_controller {
            if let Some(cmd) = tray.poll_events() {
                match cmd {
                    TrayCommand::ToggleActive(active) => {
                        let mut eng = engine.lock().unwrap();
                        eng.set_enabled(active);
                        harfiy_platform::show_status_hud(active);
                        println!(
                            "Holat o'zgartirildi: {}",
                            if active { "Faol" } else { "To'xtatilgan" }
                        );
                    }
                    TrayCommand::QuickConvertText => {
                        perform_quick_convert(
                            &mut app_config,
                            tray_controller.as_ref(),
                            &conversions_counter,
                        );
                    }
                    TrayCommand::ToggleNg(enabled) => {
                        app_config.enable_ng = enabled;
                        let _ = app_config.save();
                        {
                            let mut eng = engine.lock().unwrap();
                            eng.set_enable_ng(enabled);
                        }
                        harfiy_platform::show_custom_hud(
                            if enabled { "'ng' -> 'ñ' qoidasi yoqildi" } else { "'ng' -> 'ñ' qoidasi o'chirildi" },
                            true,
                        );
                        println!("'ng' -> 'ñ' qoidasi: {}", if enabled { "Yoqildi" } else { "O'chirildi" });
                    }
                    TrayCommand::ToggleAutostart(enabled) => {
                        let _ = harfiy_platform::set_autostart(enabled);
                        let hud_msg = format!("Avtomatik ishga tushish {}", if enabled { "yoqildi" } else { "o'chirildi" });
                        harfiy_platform::show_custom_hud(&hud_msg, true);
                        println!("Avtomatik ishga tushish: {}", if enabled { "Yoqildi" } else { "O'chirildi" });
                    }
                    TrayCommand::ToggleGameMode(enabled) => {
                        app_config.game_mode = enabled;
                        let _ = app_config.save();
                        harfiy_platform::set_game_mode(enabled);
                        harfiy_platform::show_custom_hud(
                            if enabled { "O'yinlarda avtomatik pauza yoqildi" } else { "O'yinlarda avtomatik pauza o'chirildi" },
                            true,
                        );
                        println!("O'yinlarda avtomatik pauza: {}", if enabled { "Yoqildi" } else { "O'chirildi" });
                    }
                    TrayCommand::ClearStats => {
                        conversions_counter.store(0, Ordering::SeqCst);
                        app_config.total_conversions = 0;
                        let _ = app_config.save();
                        if let Some(tray) = &tray_controller {
                            tray.update_stats(0);
                        }
                        harfiy_platform::show_custom_hud("Statistika tozalandi", true);
                        println!("Statistika tozalandi.");
                    }
                    TrayCommand::CheckUpdates => {
                        println!("GitHub Releases orqali yangilanishlar tekshirilmoqda...");
                        tray.set_checking_updates();
                        let repo = app_config.github_repo.clone();
                        let tx = tx_update.clone();
                        thread::spawn(move || {
                            let start = std::time::Instant::now();
                            let res = updater::check_for_updates(&repo);

                            // Human-perceptible feedback: keep "⏳ Tekshirilmoqda..." active for at least 1 second
                            let elapsed = start.elapsed();
                            if elapsed < Duration::from_millis(1000) {
                                thread::sleep(Duration::from_millis(1000) - elapsed);
                            }

                            let _ = tx.send(UpdateEvent {
                                result: res,
                                is_manual: true,
                            });
                        });
                    }
                    TrayCommand::DownloadUpdate => {
                        let repo = app_config.github_repo.clone();
                        let fallback_url = latest_update.lock().unwrap().as_ref().map(|i| i.release_url.clone()).unwrap_or_else(|| format!("https://github.com/{}", repo));
                        println!("Ilova o'z-o'zidan yangilanmoqda: {}", repo);
                        harfiy_platform::show_custom_hud("Yangilanish yuklab olinmoqda...", true);
                        thread::spawn(move || {
                            match updater::perform_self_update(&repo, false) {
                                Ok(new_ver) => {
                                    harfiy_platform::show_custom_hud(&format!("Harfiy v{} ga muvaffaqiyatli yangilandi!", new_ver), true);
                                    println!("Harfiy v{} ga muvaffaqiyatli yangilandi!", new_ver);
                                }
                                Err(e) => {
                                    eprintln!("Yangilashda xatolik: {}. Sahifa ochilmoqda...", e);
                                    let _ = open::that(&fallback_url);
                                }
                            }
                        });
                    }
                    TrayCommand::Quit => {
                        println!("Tizim panelidan chiqish buyrug'i berildi.");
                        is_running.store(false, Ordering::SeqCst);
                        break;
                    }
                    TrayCommand::PickExcludedAppExplorer => {
                        if let Some(picked) = harfiy_platform::pick_executable_dialog() {
                            let picked_lower = picked.to_lowercase();
                            if !app_config
                                .excluded_apps
                                .iter()
                                .any(|a| a.to_lowercase() == picked_lower)
                            {
                                app_config.excluded_apps.push(picked_lower.clone());
                                let _ = app_config.save();
                                harfiy_platform::set_excluded_apps(&app_config.excluded_apps);
                                running_apps = harfiy_platform::scan_running_apps();
                                tray.update_excluded_apps(&app_config.excluded_apps, &running_apps);
                                harfiy_platform::show_custom_hud(
                                    &format!("Istisno qo'shildi: {}", picked_lower),
                                    true,
                                );
                                println!("Istisno dastur qo'shildi: {}", picked_lower);
                            } else {
                                harfiy_platform::show_custom_hud(
                                    &format!("Allaqachon mavjud: {}", picked_lower),
                                    false,
                                );
                            }
                        }
                    }
                    TrayCommand::AddExcludedApp(app) => {
                        let app_lower = app.to_lowercase();
                        if !app_config
                            .excluded_apps
                            .iter()
                            .any(|a| a.to_lowercase() == app_lower)
                        {
                            app_config.excluded_apps.push(app_lower.clone());
                            let _ = app_config.save();
                            harfiy_platform::set_excluded_apps(&app_config.excluded_apps);
                            running_apps = harfiy_platform::scan_running_apps();
                            tray.update_excluded_apps(&app_config.excluded_apps, &running_apps);
                            harfiy_platform::show_custom_hud(
                                &format!("Istisno qo'shildi: {}", app_lower),
                                true,
                            );
                            println!("Istisno dastur qo'shildi: {}", app_lower);
                        }
                    }
                    TrayCommand::RemoveExcludedApp(app) => {
                        let app_lower = app.to_lowercase();
                        if let Some(pos) = app_config
                            .excluded_apps
                            .iter()
                            .position(|a| a.to_lowercase() == app_lower)
                        {
                            app_config.excluded_apps.remove(pos);
                            let _ = app_config.save();
                            harfiy_platform::set_excluded_apps(&app_config.excluded_apps);
                            running_apps = harfiy_platform::scan_running_apps();
                            tray.update_excluded_apps(&app_config.excluded_apps, &running_apps);
                            harfiy_platform::show_custom_hud(
                                &format!("Istisno olib tashlandi: {}", app_lower),
                                true,
                            );
                            println!("Istisno dasturlardan olib tashlandi: {}", app_lower);
                        }
                    }
                    TrayCommand::ClearAllExcludedApps => {
                        app_config.excluded_apps.clear();
                        let _ = app_config.save();
                        harfiy_platform::set_excluded_apps(&[]);
                        running_apps = harfiy_platform::scan_running_apps();
                        tray.update_excluded_apps(&app_config.excluded_apps, &running_apps);
                        harfiy_platform::show_custom_hud("Barcha istisnolar tozalandi", true);
                        println!("Barcha istisno dasturlar tozalandi.");
                    }
                    TrayCommand::OpenBlog => {
                        println!("Telegram blog ochilmoqda: tg://resolve?domain=abdumannon_notes (@abdumannon_notes)");
                        open_telegram("abdumannon_notes");
                    }
                    TrayCommand::OpenSupport => {
                        println!("Telegram support ochilmoqda: tg://resolve?domain=abdumannon_shamsiyev (@abdumannon_shamsiyev)");
                        open_telegram("abdumannon_shamsiyev");
                    }
                    TrayCommand::OpenPermissionSettings => {
                        println!("Tizim sozlamalari (Accessibility) ochilmoqda...");
                        harfiy_platform::open_permission_settings();
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    // Graceful cleanup
    // 0. Persist latest conversions count
    let final_conversions = conversions_counter.load(Ordering::Relaxed);
    if final_conversions != app_config.total_conversions {
        app_config.total_conversions = final_conversions;
        let _ = app_config.save();
    }

    // 1. Remove tray icon immediately
    tray_controller.take();

    // 2. Unhook keyboard hook and terminate worker thread
    println!("Klaviatura hook'lari olib tashlanmoqda...");
    #[cfg(windows)]
    {
        let _ = WindowsInterceptor::stop_hook();
        let _ = interceptor_thread.join();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = MacOsInterceptor::stop_hook();
        let _ = interceptor_thread.join();
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = interceptor_thread.join();
    }

    println!("Harfiy to'xtatildi.");
    std::process::exit(0);
}
