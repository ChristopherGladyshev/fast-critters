//! Семантика JS-строк для правил атрибутов css-select (длины и срезы в UTF-16,
//! `toLowerCase`, регистронезависимый матчинг регэкспов).

use dom::js::is_whitespace;

pub(crate) fn utf16_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        dom::js::utf16_len(s)
    }
}

/// `String.prototype.toLowerCase` по UTF-16 code units (одиночные суррогаты сохраняются).
pub(crate) fn lower_units(units: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(units.len());
    let mut run = String::new();
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(c) => run.push(c),
            Err(e) => {
                out.extend(run.to_lowercase().encode_utf16());
                run.clear();
                out.push(e.unpaired_surrogate());
            }
        }
    }
    out.extend(run.to_lowercase().encode_utf16());
    out
}

pub(crate) fn lower(s: &str) -> String {
    if s.is_ascii() {
        s.to_ascii_lowercase()
    } else {
        s.to_lowercase()
    }
}

/// `attr.substr(start, len).toLowerCase() === value`, где `value` уже в нижнем регистре.
/// `start`/`len` в единицах UTF-16 (с клампом как в `substr`).
pub(crate) fn substr_lower_eq(attr: &str, start: usize, len: usize, value: &str) -> bool {
    if attr.is_ascii() {
        let start = start.min(attr.len());
        let end = start.saturating_add(len).min(attr.len());
        let slice = &attr.as_bytes()[start..end];
        let value = value.as_bytes();
        return slice.len() == value.len()
            && slice
                .iter()
                .zip(value)
                .all(|(a, b)| a.to_ascii_lowercase() == *b);
    }
    let units: Vec<u16> = attr.encode_utf16().collect();
    let start = start.min(units.len());
    let end = start.saturating_add(len).min(units.len());
    let lowered = lower_units(&units[start..end]);
    lowered.iter().copied().eq(value.encode_utf16())
}

/// `attr.substr(0, len) === value`.
pub(crate) fn prefix_eq(attr: &str, len: usize, value: &str) -> bool {
    if attr.is_ascii() {
        let end = len.min(attr.len());
        return &attr[..end] == value;
    }
    let units: Vec<u16> = attr.encode_utf16().collect();
    let end = len.min(units.len());
    units[..end].iter().copied().eq(value.encode_utf16())
}

/// `attr.charAt(index) === "-"`.
pub(crate) fn char_at_is_hyphen(attr: &str, index: usize) -> bool {
    if attr.is_ascii() {
        return attr.as_bytes().get(index) == Some(&b'-');
    }
    attr.encode_utf16().nth(index) == Some(b'-' as u16)
}

/// Canonicalize (ECMAScript, не-unicode `i`-регэкспы) одной code unit.
fn canonicalize(unit: u16) -> u16 {
    if unit < 128 {
        return (unit as u8).to_ascii_uppercase() as u16;
    }
    let Some(c) = char::from_u32(unit as u32) else {
        return unit;
    };
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) if (u as u32) < 0x10000 => {
            let u = u as u32 as u16;
            if u < 128 { unit } else { u }
        }
        _ => unit,
    }
}

fn is_space_unit(unit: u16) -> bool {
    char::from_u32(unit as u32).is_some_and(is_whitespace)
}

fn units_eq_ci(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x == y || canonicalize(*x) == canonicalize(*y))
}

/// `new RegExp(escapeRegex(value), "i").test(text)`.
pub(crate) fn contains_ci(text: &str, value: &str) -> bool {
    if text.is_ascii() && value.is_ascii() {
        let (t, v) = (text.as_bytes(), value.as_bytes());
        if v.is_empty() {
            return true;
        }
        return t.windows(v.len()).any(|w| w.eq_ignore_ascii_case(v));
    }
    let t: Vec<u16> = text.encode_utf16().collect();
    let v: Vec<u16> = value.encode_utf16().collect();
    if v.is_empty() {
        return true;
    }
    t.windows(v.len()).any(|w| units_eq_ci(w, &v))
}

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;

/// Есть ли в разделённом пробелами ASCII-`text` токен `value`; `None`,
/// если `text` не ASCII (тогда для Unicode-пробелов нужен посимвольный split).
/// Сканирует по 8 байт за раз в поисках чего-либо ≤ 0x20 или не-ASCII.
fn ascii_has_token(t: &[u8], v: &[u8]) -> Option<bool> {
    let mut start = 0;
    let mut i = 0;
    let check = |i: usize, b: u8, start: &mut usize| -> Option<bool> {
        if b >= 0x80 {
            return None;
        }
        if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
            if &t[*start..i] == v {
                return Some(true);
            }
            *start = i + 1;
        }
        Some(false)
    };
    while i + 8 <= t.len() {
        let word = u64::from_le_bytes(t[i..i + 8].try_into().unwrap());
        let special = (word.wrapping_sub(ONES * 0x21) & !word & HIGHS) | (word & HIGHS);
        if special != 0 {
            for (k, &b) in t[i..i + 8].iter().enumerate() {
                if check(i + k, b, &mut start)? {
                    return Some(true);
                }
            }
        }
        i += 8;
    }
    for (k, &b) in t[i..].iter().enumerate() {
        if check(i + k, b, &mut start)? {
            return Some(true);
        }
    }
    Some(&t[start..] == v)
}

/// `new RegExp("(?:^|\\s)" + escapeRegex(value) + "(?:$|\\s)", flags).test(text)`.
pub(crate) fn contains_word(text: &str, value: &str, ignore_case: bool) -> bool {
    if !ignore_case {
        // `value` не содержит пробелов, поэтому ограниченное вхождение — целый токен.
        return match ascii_has_token(text.as_bytes(), value.as_bytes()) {
            Some(found) => found,
            None => text.split(is_whitespace).any(|token| token == value),
        };
    }
    let t: Vec<u16> = text.encode_utf16().collect();
    let v: Vec<u16> = value.encode_utf16().collect();
    if t.len() < v.len() {
        return false;
    }
    (0..=t.len() - v.len()).any(|start| {
        let end = start + v.len();
        (start == 0 || is_space_unit(t[start - 1]))
            && (end == t.len() || is_space_unit(t[end]))
            && units_eq_ci(&t[start..end], &v)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_ignore_case_rules() {
        assert!(contains_ci("ABC", "b"));
        // U+017F (long s) в верхнем регистре — ASCII 'S', что не-unicode регэкспы отвергают.
        assert!(!contains_ci("\u{17F}", "s"));
        assert!(contains_ci("Ä", "ä"));
        assert!(contains_word("a  B c", "b", true));
        assert!(contains_word("x  y", "", false));
        assert!(!contains_word("xy", "", false));
        assert!(substr_lower_eq("ÀB", 0, 1, "à"));
    }
}
