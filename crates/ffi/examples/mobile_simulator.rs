//! Interactive Mobile Keyboard Simulator
//! Simulates keystroke events and JNI/C-ABI processing exactly as iOS & Android keyboards do.

use std::ffi::CStr;
use std::io::{self, Write};

use harfiy_ffi::c_api::*;

fn main() {
    println!("============================================================");
    println!("📱 Harfiy Mobile Klaviatura Simulyatori (iOS & Android Test)");
    println!("============================================================");
    println!("Bu vosita iOS (UIInputViewController) va Android (InputMethodService)");
    println!("tomonidan yuboriladigan FFI signallarini real vaqtda tekshiradi.\n");
    println!("Buyruqlar:");
    println!("  <harflar>  - Klaviatura tugmalarini bosish (masalan: shahar, choy, o'rik)");
    println!("  <bs>       - Backspace tugmasini bosish (Instant Undo ni tekshirish)");
    println!("  <reset>    - Buferni tozalash (kursor joyi o'zgarganda)");
    println!("  <exit>     - Chiqish\n");

    let engine = unsafe { harfiy_engine_create(false) };
    let mut current_text = String::new();

    loop {
        print!("Matn: \"{}\" > ", current_text);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let input = line.trim();
        if input == "exit" {
            break;
        } else if input == "reset" {
            unsafe { harfiy_engine_reset(engine) };
            current_text.clear();
            println!("🔄 Engine holati tozalandi (Idle).\n");
            continue;
        } else if input == "bs" {
            let res = unsafe { harfiy_engine_process_backspace(engine) };
            match res.action_type {
                HarfiyActionType::PassThrough | HarfiyActionType::Suppress => {
                    if !current_text.is_empty() {
                        current_text.pop();
                    }
                    println!("  [Backspace] 1 ta harf o'chirildi (Oddiy orqaga qaytish)");
                }
                HarfiyActionType::Transform => {
                    let backspaces = res.backspaces_count as usize;
                    let replacement = unsafe {
                        CStr::from_ptr(res.replacement_utf8)
                            .to_str()
                            .unwrap_or("")
                    };
                    for _ in 0..backspaces {
                        current_text.pop();
                    }
                    current_text.push_str(replacement);
                    println!(
                        "  [↩️ Instant Undo] {} ta harf o'chirilib, asl juftlik tiklandi: \"{}\"",
                        backspaces, replacement
                    );
                }
            }
            continue;
        }

        // Process characters one by one
        for ch in input.chars() {
            let res = unsafe { harfiy_engine_process_char(engine, ch as u32) };
            match res.action_type {
                HarfiyActionType::PassThrough => {
                    current_text.push(ch);
                    println!("  '{}' -> [PassThrough] Matnga qo'shildi", ch);
                }
                HarfiyActionType::Suppress => {
                    println!("  '{}' -> [Suppress] Tugma yutib qolindi", ch);
                }
                HarfiyActionType::Transform => {
                    let backspaces = res.backspaces_count as usize;
                    let replacement = unsafe {
                        CStr::from_ptr(res.replacement_utf8)
                            .to_str()
                            .unwrap_or("")
                    };
                    for _ in 0..backspaces {
                        current_text.pop();
                    }
                    current_text.push_str(replacement);
                    println!(
                        "  '{}' -> [⚡ Transform] {} ta harf o'chirildi va yangi belgi qo'yildi: \"{}\"",
                        ch, backspaces, replacement
                    );
                }
            }
        }
        println!();
    }

    unsafe { harfiy_engine_destroy(engine) };
    println!("\n👋 Simulyator yakunlandi.");
}
