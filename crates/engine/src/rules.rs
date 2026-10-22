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
        || matches!(
            c,
            '—' | '–' | '…' | '«' | '»' | '“' | '”' | '„' | '•' | '№'
        )
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
}
