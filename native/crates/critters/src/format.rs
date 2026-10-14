//! `formatSize` из critters и процент в статистике, с форматированием чисел как в JS.

/// `formatSize(size)`: `Number#toFixed` округляет точную середину вверх, поэтому округляем в целых.
pub fn format_size(size: usize) -> String {
    if size == 0 {
        return "0 bytes".to_owned();
    }
    const UNITS: [&str; 4] = ["bytes", "kB", "MB", "GB"];
    let mut index = 0;
    while index < UNITS.len() - 1 && (size as u128) >= 1024u128.pow(index as u32 + 1) {
        index += 1;
    }
    if index == 0 {
        return format!("{size} bytes");
    }
    let divisor = 1024u128.pow(index as u32);
    let hundredths = (size as u128 * 100 * 2 + divisor) / (divisor * 2);
    format!(
        "{}.{:02} {}",
        hundredths / 100,
        hundredths % 100,
        UNITS[index]
    )
}

/// `sheet.length / before.length * 100 | 0`.
pub fn percent(part: usize, whole: usize) -> i64 {
    if whole == 0 {
        return 0;
    }
    (part as f64 / whole as f64 * 100.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_js_format_size() {
        assert_eq!(format_size(0), "0 bytes");
        assert_eq!(format_size(1023), "1023 bytes");
        assert_eq!(format_size(1024), "1.00 kB");
        // 1.125 kB — точная середина: toFixed(2) даёт 1.13
        assert_eq!(format_size(1152), "1.13 kB");
        assert_eq!(format_size(150_000), "146.48 kB");
        assert_eq!(format_size(3 * 1024 * 1024 + 5), "3.00 MB");
        assert_eq!(percent(1, 3), 33);
    }
}
