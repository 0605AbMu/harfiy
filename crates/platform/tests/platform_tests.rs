use harfiy_engine::{InputEvent, KeyAction, HarfiyEngine, SpecialKey};
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
    assert!(elapsed < std::time::Duration::from_millis(500), "Stop took too long: {:?}", elapsed);
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
