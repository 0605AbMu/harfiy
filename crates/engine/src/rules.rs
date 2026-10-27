//! Conversion rules and character classification for Harfiy.

/// Check if a character is any recognized variation of an apostrophe.
#[inline]
pub fn is_apostrophe(c: char) -> bool {
    matches!(
        c,
        '\''        // U+0027 APOSTROPHE
        | '`'       // U+0060 GRAVE ACCENT
        | '‘'       // U+2018 LEFT SINGLE QUOTATION MARK
        | '’'       // U+2019 RIGHT SINGLE QUOTATION MARK
        | 'ʻ'       // U+02BB MODIFIER LETTER TURNED COMMA
        | 'ʼ'       // U+02BC MODIFIER LETTER APOSTROPHE
        | '´'       // U+00B4 ACUTE ACCENT
        | '‛'       // U+201B SINGLE HIGH-REVERSED-9 QUOTATION MARK
        | 'ʽ'       // U+02BD MODIFIER LETTER REVERSED COMMA
        | 'ʹ'       // U+02B9 MODIFIER LETTER PRIME
        | '′'       // U+2032 PRIME
        | '＇'       // U+FF07 FULLWIDTH APOSTROPHE
        | 'ˊ'       // U+02CA MODIFIER LETTER ACUTE ACCENT
        | 'ˋ'       // U+02CB MODIFIER LETTER GRAVE ACCENT
    )
}

/// Check if a character is a delimiter (word boundary) that resets the word buffer.
#[inline]
pub fn is_word_delimiter(c: char) -> bool {
    // In Uzbek, apostrophes are part of letters (o', g') and word syllables (ma'no),
    // so they are never word delimiters.
    if is_apostrophe(c) {
        return false;
    }

    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(c, '—' | '–' | '…' | '«' | '»' | '“' | '”' | '„' | '•' | '№')
}

/// Check if a character can be the first letter in a convertible pair.
#[inline]
pub fn is_first_trigger(c: char, enable_ng: bool) -> bool {
    match c {
        's' | 'S' | 'c' | 'C' | 'o' | 'O' | 'g' | 'G' => true,
        'n' | 'N' if enable_ng => true,
        _ => false,
    }
}

/// Result of a successful rule match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionMatch {
    pub replacement: &'static str,
}

/// Check if `(prev, curr)` matches an auto-conversion rule.
pub fn match_rule(prev: char, curr: char, enable_ng: bool) -> Option<ConversionMatch> {
    match (prev, curr) {
        // sh -> ş
        ('s', 'h') | ('s', 'H') => Some(ConversionMatch { replacement: "ş" }),
        ('S', 'h') | ('S', 'H') => Some(ConversionMatch { replacement: "Ş" }),

        // ch -> ç
        ('c', 'h') | ('c', 'H') => Some(ConversionMatch { replacement: "ç" }),
        ('C', 'h') | ('C', 'H') => Some(ConversionMatch { replacement: "Ç" }),

        // o' -> ö
        ('o', c) if is_apostrophe(c) => Some(ConversionMatch { replacement: "ö" }),
        ('O', c) if is_apostrophe(c) => Some(ConversionMatch { replacement: "Ö" }),

        // g' -> ğ
        ('g', c) if is_apostrophe(c) => Some(ConversionMatch { replacement: "ğ" }),
        ('G', c) if is_apostrophe(c) => Some(ConversionMatch { replacement: "Ğ" }),

        // Optional: ng -> ñ
        ('n', 'g') | ('n', 'G') if enable_ng => Some(ConversionMatch { replacement: "ñ" }),
        ('N', 'g') | ('N', 'G') if enable_ng => Some(ConversionMatch { replacement: "Ñ" }),

        _ => None,
    }
}

/// A single replacement planned by the batch converter: the characters `start..end`
/// (indices into the input `&[char]`) are replaced by `replacement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: &'static str,
}

/// Plan every replacement needed to convert `chars` to the new Uzbek Latin orthography.
///
/// Edits are returned in ascending order and never overlap. Callers that cannot work on a
/// plain string (document formats where one word is split across several runs) apply the
/// edits to their own storage instead of calling `convert_text`.
pub fn plan_edits(chars: &[char], enable_ng: bool, smart_apostrophe: bool) -> Vec<Edit> {
    let len = chars.len();
    let mut edits = Vec::new();
    let mut i = 0;

    while i < len {
        let curr = chars[i];
        let next = chars.get(i + 1).copied();

        if let Some(m) = next.and_then(|n| match_rule(curr, n, enable_ng)) {
            edits.push(Edit {
                start: i,
                end: i + 2,
                replacement: m.replacement,
            });
            i += 2;
            continue;
        }

        // Smart apostrophe (hamza / tutuq belgisi)
        if smart_apostrophe && (curr == '\'' || curr == '`') {
            let prev_is_letter = i > 0 && chars[i - 1].is_alphabetic();
            let next_is_letter = next.map(|c| c.is_alphabetic()).unwrap_or(false);
            if prev_is_letter && next_is_letter {
                edits.push(Edit {
                    start: i,
                    end: i + 1,
                    replacement: "ʻ", // U+02BB modifier letter turned comma (official tutuq belgisi)
                });
            }
        }
        i += 1;
    }

    edits
}

/// Convert an entire string to the new Uzbek Latin orthography in batch mode.
/// Returns the converted string along with the number of replacements made.
pub fn convert_text(input: &str, enable_ng: bool, smart_apostrophe: bool) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let edits = plan_edits(&chars, enable_ng, smart_apostrophe);

    let mut result = String::with_capacity(input.len());
    let mut pos = 0;
    for edit in &edits {
        result.extend(&chars[pos..edit.start]);
        result.push_str(edit.replacement);
        pos = edit.end;
    }
    result.extend(&chars[pos..]);

    (result, edits.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_apostrophe() {
        assert!(is_apostrophe('\''));
        assert!(is_apostrophe('`'));
        assert!(is_apostrophe('‘'));
        assert!(is_apostrophe('’'));
        assert!(is_apostrophe('ʻ'));
        assert!(is_apostrophe('ʼ'));
        assert!(is_apostrophe('´'));
        assert!(!is_apostrophe('a'));
        assert!(!is_apostrophe('"'));
    }

    #[test]
    fn test_sh_rules() {
        assert_eq!(match_rule('s', 'h', false).unwrap().replacement, "ş");
        assert_eq!(match_rule('S', 'h', false).unwrap().replacement, "Ş");
        assert_eq!(match_rule('S', 'H', false).unwrap().replacement, "Ş");
        assert_eq!(match_rule('s', 'H', false).unwrap().replacement, "ş");
    }

    #[test]
    fn test_ch_rules() {
        assert_eq!(match_rule('c', 'h', false).unwrap().replacement, "ç");
        assert_eq!(match_rule('C', 'h', false).unwrap().replacement, "Ç");
        assert_eq!(match_rule('C', 'H', false).unwrap().replacement, "Ç");
    }

    #[test]
    fn test_apostrophe_rules() {
        for apo in ['\'', '`', '‘', '’', 'ʻ', 'ʼ', '´'] {
            assert_eq!(match_rule('o', apo, false).unwrap().replacement, "ö");
            assert_eq!(match_rule('O', apo, false).unwrap().replacement, "Ö");
            assert_eq!(match_rule('g', apo, false).unwrap().replacement, "ğ");
            assert_eq!(match_rule('G', apo, false).unwrap().replacement, "Ğ");
        }
    }

    #[test]
    fn test_ng_rules() {
        assert!(match_rule('n', 'g', false).is_none());
        assert_eq!(match_rule('n', 'g', true).unwrap().replacement, "ñ");
        assert_eq!(match_rule('N', 'g', true).unwrap().replacement, "Ñ");
        assert_eq!(match_rule('N', 'G', true).unwrap().replacement, "Ñ");
    }

    #[test]
    fn test_convert_text() {
        let input = "Shahar ko'chasida choyxona va g'isht bor. A'lo darajada!";
        let (out, count) = convert_text(input, false, true);
        assert_eq!(out, "Şahar köçasida çoyxona va ğişt bor. Aʻlo darajada!");
        assert_eq!(count, 7);
    }

    #[test]
    fn test_plan_edits_positions() {
        let chars: Vec<char> = "bosh o'rin a'lo".chars().collect();
        let edits = plan_edits(&chars, false, true);
        assert_eq!(
            edits,
            vec![
                Edit { start: 2, end: 4, replacement: "ş" },
                Edit { start: 5, end: 7, replacement: "ö" },
                Edit { start: 12, end: 13, replacement: "ʻ" },
            ]
        );
    }

    #[test]
    fn test_plan_edits_empty_when_nothing_to_convert() {
        let chars: Vec<char> = "salom dunyo".chars().collect();
        assert!(plan_edits(&chars, false, true).is_empty());
    }
}
