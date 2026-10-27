use harfiy_engine::{HarfiyEngine, InputEvent, KeyAction, SpecialKey};
use harfiy_platform::{
    KeyboardEmitter, KeyboardInterceptor, MockEmittedEvent, MockInterceptor, MockPlatform,
};

#[test]
fn test_mock_platform_end_to_end() {
    let platform = MockPlatform::new();
    let mut engine = HarfiyEngine::default();

    let mut emitter_clone = platform.emitter.clone();

    // Sequence of typed events: 's', 'h'
    let events = vec![InputEvent::Char('s'), InputEvent::Char('h')];
    let mut interceptor = MockInterceptor::new(events);

    interceptor
        .run(move |event| {
            let action = engine.process(event);
            match action {
                KeyAction::PassThrough => {}
                KeyAction::Suppress => {}
                KeyAction::Transform {
                    backspaces_count,
                    replacement,
                } => {
                    let _ = emitter_clone.emit_backspace(backspaces_count);
                    let _ = emitter_clone.emit_unicode(replacement);
                }
            }
            action
        })
        .unwrap();

    let recorded = platform.emitter.events();
    assert_eq!(
        recorded,
        vec![
            MockEmittedEvent::Backspace(1),
            MockEmittedEvent::Unicode("ş".to_string()),
        ]
    );
}

#[test]
fn test_mock_platform_instant_undo() {
    let platform = MockPlatform::new();
    let mut engine = HarfiyEngine::default();

    let mut emitter_clone = platform.emitter.clone();

    // Sequence: 's', 'h', then Backspace
    let events = vec![
        InputEvent::Char('s'),
        InputEvent::Char('h'),
        InputEvent::Special(SpecialKey::Backspace),
    ];
    let mut interceptor = MockInterceptor::new(events);

    interceptor
        .run(move |event| {
            let action = engine.process(event);
            match action {
                KeyAction::PassThrough => {}
                KeyAction::Suppress => {}
                KeyAction::Transform {
                    backspaces_count,
                    replacement,
                } => {
                    let _ = emitter_clone.emit_backspace(backspaces_count);
                    let _ = emitter_clone.emit_unicode(replacement);
                }
            }
            action
        })
        .unwrap();

    let recorded = platform.emitter.events();
    assert_eq!(
        recorded,
        vec![
            MockEmittedEvent::Backspace(1),
            MockEmittedEvent::Unicode("ş".to_string()),
            MockEmittedEvent::Backspace(1),
            MockEmittedEvent::Unicode("sh".to_string()),
        ]
    );
}

#[cfg(windows)]
#[test]
fn test_windows_interceptor_graceful_stop() {
    use harfiy_platform::WindowsInterceptor;

    let handle = std::thread::spawn(|| {
        let mut interceptor = WindowsInterceptor::new();
        let _ = interceptor.run(|_| KeyAction::PassThrough);
    });

    std::thread::sleep(std::time::Duration::from_millis(100));
    let sw = std::time::Instant::now();
    let _ = WindowsInterceptor::stop_hook();
    handle.join().unwrap();
    let elapsed = sw.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "Stop took too long: {:?}",
        elapsed
    );
}

#[cfg(windows)]
#[test]
fn test_excluded_apps_management() {
    let apps = vec!["code.exe".to_string(), "notepad.exe".to_string()];
    harfiy_platform::set_excluded_apps(&apps);
    // Running apps scan returns non-empty list on desktop or at least does not crash
    let running = harfiy_platform::scan_running_apps();
    println!("Scanned running GUI apps count: {}", running.len());
}

#[cfg(windows)]
#[test]
fn test_windows_uzbek_latin_layout_translation() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        ToUnicodeEx, HKL, VK_OEM_7, VK_S,
    };

    let key_state = [0u8; 256];
    let mut buffer = [0u16; 8];

    // Uzbek Latin HKL in Windows composite identifier: 0x04090443
    let uz_hkl = HKL(0x04090443 as *mut _);

    // Direct call with unmapped composite HKL in background process fails:
    let count_direct = unsafe { ToUnicodeEx(VK_S.0 as u32, 0x1F, &key_state, &mut buffer, 4, uz_hkl) };
    assert_eq!(count_direct, 0);

    // But extracting the layout HIWORD (0x0409) resolves to US QWERTY:
    let hi = (uz_hkl.0 as usize >> 16) & 0xFFFF;
    assert_eq!(hi, 0x0409);
    let sub_hkl = HKL(((hi << 16) | hi) as *mut _);
    let count_fallback = unsafe { ToUnicodeEx(VK_S.0 as u32, 0x1F, &key_state, &mut buffer, 4, sub_hkl) };
    assert_eq!(count_fallback, 1);
    assert_eq!(char::from_u32(buffer[0] as u32), Some('s'));

    // Test quote key VK_OEM_7
    let count_quote = unsafe { ToUnicodeEx(VK_OEM_7.0 as u32, 0x28, &key_state, &mut buffer, 4, sub_hkl) };
    assert_eq!(count_quote, 1);
    assert_eq!(char::from_u32(buffer[0] as u32), Some('\''));
}
