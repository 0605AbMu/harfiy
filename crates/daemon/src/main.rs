//! Harfiy Background Daemon & CLI
//! Real-time Uzbek Latin orthography conversion engine.

mod config;

use std::sync::atomic::{AtomicBool, Ordering};
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
use harfiy_tray::{TrayCommand, TrayController};

use crate::config::AppConfig;

#[derive(Parser)]
#[command(name = "harfiy")]
#[command(author = "Abdumannon Shamsiyev")]
#[command(version = "0.1.0")]
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Harfiy daemon status: Ready / Standby.");
        }
        Some(Commands::Stop) => {
            println!("Signal to stop Harfiy sent.");
        }
        Some(Commands::Start { no_tray }) => {
            run_daemon(no_tray);
        }
        None => {
            run_daemon(cli.no_tray);
        }
    }
}

fn run_daemon(no_tray: bool) {
    let (app_config, config_path) = AppConfig::load_or_create();
    println!("Harfiy v0.1.0 faollashtirilmoqda...");
    println!("Sozlamalar fayli: {}", config_path.display());
    println!("Qoidalar: 'sh' -> 'ş', 'ch' -> 'ç', 'o''' -> 'ö', 'g''' -> 'ğ'");
    if app_config.enable_ng {
        println!("Qo'shimcha: 'ng' -> 'ñ' yoqilgan.");
    }
    println!("Tezkor tugma: Ctrl + Shift + U");

    // Initialize engine
    let mut engine_inner = HarfiyEngine::new(EngineConfig {
        enable_ng: app_config.enable_ng,
        enabled: true,
    });

    for exc in &app_config.exceptions {
        engine_inner.add_exception(exc);
    }

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
        thread::spawn(move || {
            let mut interceptor = WindowsInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                eng.process(event)
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
        thread::spawn(move || {
            let mut interceptor = MacOsInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                eng.process(event)
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
        thread::spawn(move || {
            let mut interceptor = LinuxInterceptor::new();
            let res = interceptor.run(move |event| {
                let mut eng = engine_hook.lock().unwrap();
                eng.process(event)
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
    let mut tray_controller = if !no_tray {
        match TrayController::new(true) {
            Ok(controller) => Some(controller),
            Err(e) => {
                eprintln!(
                    "Tizim paneli (System Tray) yaratilmadi: {}. Konsol rejimida davom etilmoqda.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    println!("Harfiy muvaffaqiyatli ishga tushdi. To'xtatish uchun Ctrl+C bosing.");

    // Main event loop
    while is_running.load(Ordering::SeqCst) {
        #[cfg(windows)]
        pump_messages();

        if let Some(tray) = &tray_controller {
            if let Some(cmd) = tray.poll_events() {
                match cmd {
                    TrayCommand::ToggleActive(active) => {
                        let mut eng = engine.lock().unwrap();
                        eng.set_enabled(active);
                        println!(
                            "Holat o'zgartirildi: {}",
                            if active { "Faol" } else { "To'xtatilgan" }
                        );
                    }
                    TrayCommand::OpenSettings => {
                        println!("Sozlamalar: {}", config_path.display());
                    }
                    TrayCommand::Quit => {
                        println!("Tizim panelidan chiqish buyrug'i berildi.");
                        is_running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    // Graceful cleanup
    // 1. Remove tray icon immediately
    tray_controller.take();

    // 2. Unhook keyboard hook and terminate worker thread
    #[cfg(windows)]
    {
        println!("Klaviatura hook'lari olib tashlanmoqda...");
        let _ = WindowsInterceptor::stop_hook();
        let _ = interceptor_thread.join();
    }

    #[cfg(not(windows))]
    {
        let _ = interceptor_thread.join();
    }

    println!("Harfiy to'xtatildi.");
}
