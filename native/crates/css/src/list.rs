//! postcss `list.comma` и сеттер `Rule#selectors`.

use dom::js;

/// `list.comma(string)` — элементы — обрезанные подсрезы `s`.
pub fn comma(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut cur = 0usize;
    let mut func = 0u32;
    let mut in_quote = false;
    let mut prev_quote = '\0';
    let mut escape = false;
    for (i, ch) in s.char_indices() {
        let mut split = false;
        if escape {
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if in_quote {
            if ch == prev_quote {
                in_quote = false;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            prev_quote = ch;
        } else if ch == '(' {
            func += 1;
        } else if ch == ')' {
            func = func.saturating_sub(1);
        } else if func == 0 && ch == ',' {
            split = true;
        }
        if split {
            if i > cur {
                out.push(js::trim(&s[cur..i]));
            }
            cur = i + 1;
        }
    }
    out.push(js::trim(&s[cur..]));
    out
}

/// Разделитель, выбираемый сеттером `Rule#selectors`: `selector.match(/,\s*/)`
/// или `',' + raws.between`.
pub fn setter_sep(selector: &str, between: &str) -> String {
    if !selector.is_empty()
        && let Some(i) = selector.find(',')
    {
        let rest = &selector[i + 1..];
        let ws = rest.len() - js::trim_start(rest).len();
        return selector[i..i + 1 + ws].to_string();
    }
    let mut s = String::with_capacity(1 + between.len());
    s.push(',');
    s.push_str(between);
    s
}

/// `rule.selectors = values` при текущем селекторе.
pub fn set_selectors(selector: &str, between: &str, values: &[&str]) -> String {
    let sep = setter_sep(selector, between);
    values.join(sep.as_str())
}
