use harfiy_engine::{
    is_apostrophe, EngineConfig, HarfiyEngine, InputEvent, KeyAction, SpecialKey,
};

#[test]
fn test_all_sh_permutations() {
    let mut engine = HarfiyEngine::default();

    // lower: s + h -> ş
    assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('h')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "ş"
        }
    );

    // Title: S + h -> Ş
    assert_eq!(engine.process(InputEvent::Char('S')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('h')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "Ş"
        }
    );

    // Upper: S + H -> Ş
    assert_eq!(engine.process(InputEvent::Char('S')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('H')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "Ş"
        }
    );

    // Inverted caps: s + H -> ş
    assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('H')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "ş"
        }
    );
}

#[test]
fn test_all_ch_permutations() {
    let mut engine = HarfiyEngine::default();

    // c + h -> ç
    assert_eq!(engine.process(InputEvent::Char('c')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('h')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "ç"
        }
    );

    // C + h -> Ç
    assert_eq!(engine.process(InputEvent::Char('C')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('h')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "Ç"
        }
    );

    // C + H -> Ç
    assert_eq!(engine.process(InputEvent::Char('C')), KeyAction::PassThrough);
    assert_eq!(
        engine.process(InputEvent::Char('H')),
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "Ç"
        }
    );
}

#[test]
fn test_all_apostrophe_types() {
    let apostrophes = ['\'', '`', '‘', '’', 'ʻ', 'ʼ', '´'];

    for apo in apostrophes {
        assert!(is_apostrophe(apo));
        let mut engine = HarfiyEngine::default();

        // o + apo -> ö
        engine.process(InputEvent::Char('o'));
        assert_eq!(
            engine.process(InputEvent::Char(apo)),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ö"
            }
        );

        // O + apo -> Ö
        engine.process(InputEvent::Char('O'));
        assert_eq!(
            engine.process(InputEvent::Char(apo)),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "Ö"
            }
        );

        // g + apo -> ğ
        engine.process(InputEvent::Char('g'));
        assert_eq!(
            engine.process(InputEvent::Char(apo)),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ğ"
            }
        );

        // G + apo -> Ğ
        engine.process(InputEvent::Char('G'));
        assert_eq!(
            engine.process(InputEvent::Char(apo)),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "Ğ"
            }
        );
    }
}

#[test]
fn test_instant_undo_comprehensive() {
    let mut engine = HarfiyEngine::default();

    // 1. Convert 's' + 'h' -> 'ş'
    engine.process(InputEvent::Char('s'));
    let action = engine.process(InputEvent::Char('h'));
    assert_eq!(
        action,
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "ş"
        }
    );

    // 2. Immediate backspace should undo to "sh"
    let undo = engine.process(InputEvent::Special(SpecialKey::Backspace));
    assert_eq!(
        undo,
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "sh"
        }
    );

    // 3. Second backspace is just normal backspace
    let normal_bs = engine.process(InputEvent::Special(SpecialKey::Backspace));
    assert_eq!(normal_bs, KeyAction::PassThrough);

    // 4. Test instant undo with 'O' + '’' -> 'Ö' -> undo to "O’"
    engine.process(InputEvent::Char('O'));
    engine.process(InputEvent::Char('’'));
    let undo_o = engine.process(InputEvent::Special(SpecialKey::Backspace));
    assert_eq!(
        undo_o,
        KeyAction::Transform {
            backspaces_count: 1,
            replacement: "O’"
        }
    );
}

#[test]
fn test_non_matching_keys_reset_undo() {
    let mut engine = HarfiyEngine::default();

    // Convert 's' + 'h' -> 'ş'
    engine.process(InputEvent::Char('s'));
    engine.process(InputEvent::Char('h'));

    // User continues typing 'a'
    let next_char = engine.process(InputEvent::Char('a'));
    assert_eq!(next_char, KeyAction::PassThrough);

    // Now backspace should NOT trigger undo for "sh", it should be normal backspace!
    let bs = engine.process(InputEvent::Special(SpecialKey::Backspace));
    assert_eq!(bs, KeyAction::PassThrough);
}

#[test]
fn test_exceptions_and_delimiters() {
    let mut engine = HarfiyEngine::default();

    // Type "Salom " then "Ishoq"
    for c in "Salom ".chars() {
        assert_eq!(engine.process(InputEvent::Char(c)), KeyAction::PassThrough);
    }

    // Now type "Is"
    assert_eq!(engine.process(InputEvent::Char('I')), KeyAction::PassThrough);
    assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);

    // Typing 'h' should NOT convert because "Ishoq" is an exception!
    assert_eq!(engine.process(InputEvent::Char('h')), KeyAction::PassThrough);

    // Remaining chars
    assert_eq!(engine.process(InputEvent::Char('o')), KeyAction::PassThrough);
    assert_eq!(engine.process(InputEvent::Char('q')), KeyAction::PassThrough);
}

#[test]
fn test_disabled_engine_passes_everything() {
    let mut engine = HarfiyEngine::new(EngineConfig {
        enable_ng: false,
        enabled: false,
    });

    assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);
    assert_eq!(engine.process(InputEvent::Char('h')), KeyAction::PassThrough);
    assert_eq!(engine.process(InputEvent::Char('o')), KeyAction::PassThrough);
    assert_eq!(engine.process(InputEvent::Char('\'')), KeyAction::PassThrough);
}
