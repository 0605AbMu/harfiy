//! Platform-neutral encoding of planned edits for hosts that index text in UTF-16
//! (Java/Kotlin), kept outside the JNI module so it is testable on any target.

use harfiy_engine::plan_edits;

/// Number of `i32`s describing one edit in [`plan_edits_utf16`]'s output.
pub const EDIT_STRIDE: usize = 4;

/// Plans live-typing edits for `text` (no smart apostrophe, as when typing) and flattens
/// them into `[start, end, kind, replacement]` groups:
/// - `start..end` are UTF-16 offsets into `text`;
/// - `kind` is the `RuleKind` discriminant, or -1 for edits without one;
/// - `replacement` is the replacement's single Unicode code point.
pub fn plan_edits_utf16(text: &str, enable_ng: bool) -> Vec<i32> {
    let chars: Vec<char> = text.chars().collect();
    let edits = plan_edits(&chars, enable_ng, false);
    if edits.is_empty() {
        return Vec::new();
    }

    let mut utf16_at = Vec::with_capacity(chars.len() + 1);
    let mut offset = 0i32;
    for c in &chars {
        utf16_at.push(offset);
        offset += c.len_utf16() as i32;
    }
    utf16_at.push(offset);

    let mut out = Vec::with_capacity(edits.len() * EDIT_STRIDE);
    for edit in edits {
        let mut repl = edit.replacement.chars();
        let code_point = repl.next().expect("replacement is never empty");
        debug_assert!(repl.next().is_none(), "replacements are single characters");
        out.extend([
            utf16_at[edit.start],
            utf16_at[edit.end],
            edit.kind.map_or(-1, |k| k as i32),
            code_point as i32,
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use harfiy_engine::RuleKind;

    #[test]
    #[rustfmt::skip] // one row per edit
    fn offsets_are_utf16_across_surrogate_pairs() {
        // '😀' is two UTF-16 units, so "sh" after it starts at 2, not 1.
        let out = plan_edits_utf16("😀sh o'", false);
        assert_eq!(
            out,
            vec![
                2, 4, RuleKind::Sh as i32, 'ş' as i32,
                5, 7, RuleKind::O as i32, 'ö' as i32,
            ]
        );
    }

    #[test]
    fn nothing_to_convert_is_empty() {
        assert!(plan_edits_utf16("salom", false).is_empty());
    }

    #[test]
    fn no_smart_apostrophe_while_typing() {
        assert!(plan_edits_utf16("ma'no", false).is_empty());
    }
}
