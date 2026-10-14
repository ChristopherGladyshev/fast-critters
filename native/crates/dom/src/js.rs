//! Семантика строк JavaScript, на которую опираются портированные библиотеки.

/// `\s` в JS-регэкспе / то, что срезает `String.prototype.trim`.
pub fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `String.prototype.trim`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_whitespace)
}

pub fn trim_start(s: &str) -> &str {
    s.trim_start_matches(is_whitespace)
}

pub fn trim_end(s: &str) -> &str {
    s.trim_end_matches(is_whitespace)
}

/// `str.length` в JS (кодовые единицы UTF-16).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_js_whitespace_only() {
        assert_eq!(trim("\u{FEFF}\u{A0} a\u{2028}\n"), "a");
        assert_eq!(trim("\u{200B}a"), "\u{200B}a");
        assert_eq!(utf16_len("a😀"), 3);
    }
}
