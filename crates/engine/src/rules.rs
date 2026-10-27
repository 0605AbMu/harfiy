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

/// Convert an entire string to the new Uzbek Latin orthography in batch mode.
/// Returns the converted string along with the number of replacements made.
pub fn convert_text(input: &str, enable_ng: bool, smart_apostrophe: bool) -> (String, usize) {
    let mut result = String::with_capacity(input.len());
    let mut count = 0;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let curr = chars[i];
        let next = if i + 1 < len {
            Some(chars[i + 1])
        } else {
            None
        };

        let mut matched = false;
        if let Some(n) = next {
            if let Some(m) = match_rule(curr, n, enable_ng) {
                result.push_str(m.replacement);
                count += 1;
                i += 2;
                matched = true;
            }
        }

        if !matched {
            // Smart apostrophe (hamza / tutuq belgisi)
            if smart_apostrophe && (curr == '\'' || curr == '`') {
                let prev_is_letter = if i > 0 {
                    chars[i - 1].is_alphabetic()
                } else {
                    false
                };
                let next_is_letter = next.map(|c| c.is_alphabetic()).unwrap_or(false);
                if prev_is_letter && next_is_letter {
                    result.push('ʻ'); // U+02BB modifier letter turned comma (official tutuq belgisi)
                    count += 1;
                    i += 1;
                    continue;
                }
            }

            result.push(curr);
            i += 1;
        }
    }

    (result, count)
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
}
