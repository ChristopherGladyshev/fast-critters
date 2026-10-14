//! Порт `parse` из css-what 6.2.2 (lib/commonjs/parse.js).
//!
//! Селектор обрабатывается как последовательность char. css-what работает с UTF-16
//! code units, но все возможные границы приходятся между целыми char, поэтому
//! результаты совпадают. Единственное отличие в представлении: escape, который
//! декодируется в одиночный суррогат (`\d800` или code point выше U+10FFFF), здесь
//! даёт U+FFFD, т.к. строки Rust не могут хранить одиночные суррогаты.

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectorType {
    Attribute,
    Pseudo,
    PseudoElement,
    Tag,
    Universal,
    Adjacent,
    Child,
    Descendant,
    Parent,
    Sibling,
    ColumnCombinator,
}

impl SelectorType {
    pub fn as_str(self) -> &'static str {
        match self {
            SelectorType::Attribute => "attribute",
            SelectorType::Pseudo => "pseudo",
            SelectorType::PseudoElement => "pseudo-element",
            SelectorType::Tag => "tag",
            SelectorType::Universal => "universal",
            SelectorType::Adjacent => "adjacent",
            SelectorType::Child => "child",
            SelectorType::Descendant => "descendant",
            SelectorType::Parent => "parent",
            SelectorType::Sibling => "sibling",
            SelectorType::ColumnCombinator => "column-combinator",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeAction {
    Any,
    Element,
    End,
    Equals,
    Exists,
    Hyphen,
    Not,
    Start,
}

impl AttributeAction {
    pub fn as_str(self) -> &'static str {
        match self {
            AttributeAction::Any => "any",
            AttributeAction::Element => "element",
            AttributeAction::End => "end",
            AttributeAction::Equals => "equals",
            AttributeAction::Exists => "exists",
            AttributeAction::Hyphen => "hyphen",
            AttributeAction::Not => "not",
            AttributeAction::Start => "start",
        }
    }
}

/// `ignoreCase` из css-what: `null`, `"quirks"`, `true` или `false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IgnoreCase {
    /// `null`: решается по имени атрибута (регистронезависимые атрибуты HTML).
    Unknown,
    /// `"quirks"` (`.class` / `#id`): регистрозависимо, кроме quirks mode.
    Quirks,
    /// Флаг `i`.
    True,
    /// Флаг `s`.
    False,
}

impl IgnoreCase {
    /// Истинность в JS (`"quirks"` и `true`).
    pub fn is_truthy(self) -> bool {
        matches!(self, IgnoreCase::Quirks | IgnoreCase::True)
    }
}

/// `data` токена псевдокласса.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PseudoData {
    /// `null`.
    None,
    /// Строковый аргумент (escape-последовательности уже раскрыты).
    Str(String),
    /// Распарсенный список селекторов для `:is`, `:not`, `:has`, `:matches`, `:where`,
    /// `:host`, `:host-context`.
    Selectors(Vec<Vec<Token>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Attribute {
        name: String,
        action: AttributeAction,
        value: String,
        namespace: Option<String>,
        ignore_case: IgnoreCase,
    },
    Pseudo {
        name: String,
        data: PseudoData,
    },
    PseudoElement {
        name: String,
        data: Option<String>,
    },
    Tag {
        name: String,
        namespace: Option<String>,
    },
    Universal {
        namespace: Option<String>,
    },
    Adjacent,
    Child,
    Descendant,
    Parent,
    Sibling,
    ColumnCombinator,
}

impl Token {
    pub fn selector_type(&self) -> SelectorType {
        match self {
            Token::Attribute { .. } => SelectorType::Attribute,
            Token::Pseudo { .. } => SelectorType::Pseudo,
            Token::PseudoElement { .. } => SelectorType::PseudoElement,
            Token::Tag { .. } => SelectorType::Tag,
            Token::Universal { .. } => SelectorType::Universal,
            Token::Adjacent => SelectorType::Adjacent,
            Token::Child => SelectorType::Child,
            Token::Descendant => SelectorType::Descendant,
            Token::Parent => SelectorType::Parent,
            Token::Sibling => SelectorType::Sibling,
            Token::ColumnCombinator => SelectorType::ColumnCombinator,
        }
    }

    /// `isTraversal` из css-what.
    pub fn is_traversal(&self) -> bool {
        matches!(
            self,
            Token::Adjacent
                | Token::Child
                | Token::Descendant
                | Token::Parent
                | Token::Sibling
                | Token::ColumnCombinator
        )
    }

    /// `JSON.stringify(token)` с порядком ключей css-what.
    pub fn write_json(&self, out: &mut String) {
        let ty = self.selector_type().as_str();
        match self {
            Token::Attribute {
                name,
                action,
                value,
                namespace,
                ignore_case,
            } => {
                let _ = write!(out, "{{\"type\":\"{ty}\",\"name\":");
                json_str(out, name);
                let _ = write!(out, ",\"action\":\"{}\",\"value\":", action.as_str());
                json_str(out, value);
                out.push_str(",\"namespace\":");
                json_opt(out, namespace.as_deref());
                out.push_str(",\"ignoreCase\":");
                out.push_str(match ignore_case {
                    IgnoreCase::Unknown => "null",
                    IgnoreCase::Quirks => "\"quirks\"",
                    IgnoreCase::True => "true",
                    IgnoreCase::False => "false",
                });
                out.push('}');
            }
            Token::Pseudo { name, data } => {
                let _ = write!(out, "{{\"type\":\"{ty}\",\"name\":");
                json_str(out, name);
                out.push_str(",\"data\":");
                match data {
                    PseudoData::None => out.push_str("null"),
                    PseudoData::Str(s) => json_str(out, s),
                    PseudoData::Selectors(list) => write_selectors_json(list, out),
                }
                out.push('}');
            }
            Token::PseudoElement { name, data } => {
                let _ = write!(out, "{{\"type\":\"{ty}\",\"name\":");
                json_str(out, name);
                out.push_str(",\"data\":");
                json_opt(out, data.as_deref());
                out.push('}');
            }
            Token::Tag { name, namespace } => {
                let _ = write!(out, "{{\"type\":\"{ty}\",\"name\":");
                json_str(out, name);
                out.push_str(",\"namespace\":");
                json_opt(out, namespace.as_deref());
                out.push('}');
            }
            Token::Universal { namespace } => {
                let _ = write!(out, "{{\"type\":\"{ty}\",\"namespace\":");
                json_opt(out, namespace.as_deref());
                out.push('}');
            }
            _ => {
                let _ = write!(out, "{{\"type\":\"{ty}\"}}");
            }
        }
    }
}

/// `JSON.stringify(parse(selector))`.
pub fn write_selectors_json(list: &[Vec<Token>], out: &mut String) {
    out.push('[');
    for (i, tokens) in list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        for (j, token) in tokens.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            token.write_json(out);
        }
        out.push(']');
    }
    out.push(']');
}

pub fn selectors_to_json(list: &[Vec<Token>]) -> String {
    let mut out = String::new();
    write_selectors_json(list, &mut out);
    out
}

fn json_opt(out: &mut String, value: Option<&str>) {
    match value {
        Some(s) => json_str(out, s),
        None => out.push_str("null"),
    }
}

/// `JSON.stringify(string)`.
fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `\s` в JS-регэкспе.
fn is_js_space(c: char) -> bool {
    dom::js::is_whitespace(c)
}

/// Что не матчит `.` в JS-регэкспе без dotAll.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `isWhitespace` из css-what.
fn is_ws(c: Option<char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\n' | '\u{0C}' | '\r'))
}

fn is_quote(c: Option<char>) -> bool {
    matches!(c, Some('\'' | '"'))
}

/// `[\w\-°-￿]` (astral-символы: оба суррогата попадают в диапазон).
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c as u32 >= 0xB0
}

/// Один элемент `(?:\\(?:[\da-f]{1,6}\s?|.)|[\w\-°-￿])` в позиции `p`.
fn name_item(s: &[char], p: usize) -> Option<usize> {
    let c = *s.get(p)?;
    if c == '\\' {
        let mut q = p + 1;
        let mut hex = 0;
        while hex < 6 && q < s.len() && matches!(s[q], '0'..='9' | 'a'..='f') {
            hex += 1;
            q += 1;
        }
        if hex > 0 {
            if q < s.len() && is_js_space(s[q]) {
                q += 1;
            }
            return Some(q - p);
        }
        return match s.get(p + 1) {
            Some(&d) if !is_line_terminator(d) => Some(2),
            _ => None,
        };
    }
    is_name_char(c).then_some(1)
}

fn name_items(s: &[char], mut p: usize) -> Option<usize> {
    let start = p;
    while let Some(n) = name_item(s, p) {
        p += n;
    }
    (p > start).then_some(p - start)
}

/// Длина (в char) совпадения `reName` с позиции `start`:
/// `/^[^\\#]?(?:\\(?:[\da-f]{1,6}\s?|.)|[\w\-°-￿])+/`.
fn match_name(s: &[char], start: usize) -> Option<usize> {
    let c = *s.get(start)?;
    let with_prefix = if c != '\\' && c != '#' {
        name_items(s, start + 1).map(|n| n + 1)
    } else {
        None
    };
    with_prefix.or_else(|| name_items(s, start))
}

fn push_code_unit(out: &mut String, unit: u32) {
    let unit = unit & 0xFFFF;
    out.push(char::from_u32(unit).unwrap_or('\u{FFFD}'));
}

/// css-what `unescapeCSS`: `str.replace(/\\([\da-f]{1,6}\s?|(\s)|.)/gi, funescape)`.
pub(crate) fn unescape_css(s: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c != '\\' || i + 1 >= s.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut q = i + 1;
        let mut value: u32 = 0;
        while q - (i + 1) < 6 && q < s.len() && s[q].is_ascii_hexdigit() {
            value = value * 16 + s[q].to_digit(16).unwrap();
            q += 1;
        }
        if q > i + 1 {
            if q < s.len() && is_js_space(s[q]) {
                q += 1;
            }
            // funescape: `parseInt(escaped, 16) - 0x10000`.
            if value < 0x10000 {
                push_code_unit(&mut out, value);
            } else if value <= 0x10FFFF {
                out.push(char::from_u32(value).unwrap_or('\u{FFFD}'));
            } else {
                let high = value - 0x10000;
                push_code_unit(&mut out, (high >> 10) | 0xD800);
                push_code_unit(&mut out, (high & 0x3FF) | 0xDC00);
            }
            i = q;
        } else {
            // `(\s)` возвращает пробельный символ, `.` — сам символ (NaN для parseInt).
            out.push(s[i + 1]);
            i += 2;
        }
    }
    out
}

const UNPACK_PSEUDOS: [&str; 7] = [
    "has",
    "not",
    "matches",
    "is",
    "where",
    "host",
    "host-context",
];

/// `parse(selector)`; `Err` несёт сообщение ошибки JS.
pub fn parse(selector: &str) -> Result<Vec<Vec<Token>>, String> {
    let chars: Vec<char> = selector.chars().collect();
    let mut parser = Parser {
        s: &chars,
        source: selector,
        i: 0,
    };
    let mut subselects = Vec::new();
    let end = parser.parse_selector(&mut subselects, 0)?;
    if end < chars.len() {
        let rest: String = chars[end..].iter().collect();
        return Err(format!("Unmatched selector: {rest}"));
    }
    Ok(subselects)
}

struct Parser<'a> {
    s: &'a [char],
    source: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn at(&self, i: usize) -> Option<char> {
        self.s.get(i).copied()
    }

    fn rest_from(&self, i: usize) -> String {
        self.s[i.min(self.s.len())..].iter().collect()
    }

    fn get_name(&mut self, offset: usize) -> Result<String, String> {
        let start = self.i + offset;
        match match_name(self.s, start) {
            Some(n) => {
                self.i = start + n;
                Ok(unescape_css(&self.s[start..start + n]))
            }
            None => Err(format!("Expected name, found {}", self.rest_from(self.i))),
        }
    }

    fn strip_whitespace(&mut self, offset: usize) {
        self.i += offset;
        while self.i < self.s.len() && is_ws(self.at(self.i)) {
            self.i += 1;
        }
    }

    fn is_escaped(&self, pos: usize) -> bool {
        let mut slashes = 0;
        let mut p = pos;
        while p > 0 && self.s[p - 1] == '\\' {
            slashes += 1;
            p -= 1;
        }
        slashes & 1 == 1
    }

    fn read_value_with_parenthesis(&mut self) -> Result<String, String> {
        self.i += 1;
        let start = self.i;
        let mut counter = 1;
        while counter > 0 && self.i < self.s.len() {
            let c = self.s[self.i];
            if c == '(' && !self.is_escaped(self.i) {
                counter += 1;
            } else if c == ')' && !self.is_escaped(self.i) {
                counter -= 1;
            }
            self.i += 1;
        }
        if counter != 0 {
            return Err("Parenthesis not matched".to_owned());
        }
        Ok(unescape_css(&self.s[start..self.i - 1]))
    }

    fn parse_selector(
        &mut self,
        subselects: &mut Vec<Vec<Token>>,
        index: usize,
    ) -> Result<usize, String> {
        self.i = index;
        let mut tokens: Vec<Token> = Vec::new();

        fn ensure_not_traversal(tokens: &[Token]) -> Result<(), String> {
            match tokens.last() {
                Some(t) if t.is_traversal() => {
                    Err("Did not expect successive traversals.".to_owned())
                }
                _ => Ok(()),
            }
        }
        fn add_traversal(tokens: &mut Vec<Token>, t: Token) -> Result<(), String> {
            if let Some(last @ Token::Descendant) = tokens.last_mut() {
                *last = t;
                return Ok(());
            }
            ensure_not_traversal(tokens)?;
            tokens.push(t);
            Ok(())
        }
        fn finalize(
            tokens: &mut Vec<Token>,
            subselects: &mut Vec<Vec<Token>>,
        ) -> Result<(), String> {
            if let Some(Token::Descendant) = tokens.last() {
                tokens.pop();
            }
            if tokens.is_empty() {
                return Err("Empty sub-selector".to_owned());
            }
            subselects.push(std::mem::take(tokens));
            Ok(())
        }

        self.strip_whitespace(0);
        if self.i == self.s.len() {
            return Ok(self.i);
        }

        'outer: while self.i < self.s.len() {
            let first = self.s[self.i];
            match first {
                ' ' | '\t' | '\n' | '\u{0C}' | '\r' => {
                    if tokens.is_empty() || tokens[0] != Token::Descendant {
                        ensure_not_traversal(&tokens)?;
                        tokens.push(Token::Descendant);
                    }
                    self.strip_whitespace(1);
                }
                '>' => {
                    add_traversal(&mut tokens, Token::Child)?;
                    self.strip_whitespace(1);
                }
                '<' => {
                    add_traversal(&mut tokens, Token::Parent)?;
                    self.strip_whitespace(1);
                }
                '~' => {
                    add_traversal(&mut tokens, Token::Sibling)?;
                    self.strip_whitespace(1);
                }
                '+' => {
                    add_traversal(&mut tokens, Token::Adjacent)?;
                    self.strip_whitespace(1);
                }
                '.' | '#' => {
                    let (name, action) = if first == '.' {
                        ("class", AttributeAction::Element)
                    } else {
                        ("id", AttributeAction::Equals)
                    };
                    let value = self.get_name(1)?;
                    tokens.push(Token::Attribute {
                        name: name.to_owned(),
                        action,
                        value,
                        namespace: None,
                        ignore_case: IgnoreCase::Quirks,
                    });
                }
                '[' => {
                    self.strip_whitespace(1);
                    let mut namespace = None;
                    let name;
                    if self.at(self.i) == Some('|') {
                        name = self.get_name(1)?;
                    } else if self.at(self.i) == Some('*') && self.at(self.i + 1) == Some('|') {
                        namespace = Some("*".to_owned());
                        name = self.get_name(2)?;
                    } else {
                        let n = self.get_name(0)?;
                        if self.at(self.i) == Some('|') && self.at(self.i + 1) != Some('=') {
                            namespace = Some(n);
                            name = self.get_name(1)?;
                        } else {
                            name = n;
                        }
                    }
                    self.strip_whitespace(0);

                    let mut action = AttributeAction::Exists;
                    let possible = match self.at(self.i) {
                        Some('~') => Some(AttributeAction::Element),
                        Some('^') => Some(AttributeAction::Start),
                        Some('$') => Some(AttributeAction::End),
                        Some('*') => Some(AttributeAction::Any),
                        Some('!') => Some(AttributeAction::Not),
                        Some('|') => Some(AttributeAction::Hyphen),
                        _ => None,
                    };
                    if let Some(possible) = possible {
                        action = possible;
                        if self.at(self.i + 1) != Some('=') {
                            return Err("Expected `=`".to_owned());
                        }
                        self.strip_whitespace(2);
                    } else if self.at(self.i) == Some('=') {
                        action = AttributeAction::Equals;
                        self.strip_whitespace(1);
                    }

                    let mut value = String::new();
                    let mut ignore_case = IgnoreCase::Unknown;
                    if action != AttributeAction::Exists {
                        if is_quote(self.at(self.i)) {
                            let quote = self.s[self.i];
                            let mut end = self.i + 1;
                            while end < self.s.len()
                                && (self.s[end] != quote || self.is_escaped(end))
                            {
                                end += 1;
                            }
                            if self.at(end) != Some(quote) {
                                return Err("Attribute value didn't end".to_owned());
                            }
                            value = unescape_css(&self.s[self.i + 1..end]);
                            self.i = end + 1;
                        } else {
                            let start = self.i;
                            while self.i < self.s.len()
                                && ((!is_ws(self.at(self.i)) && self.s[self.i] != ']')
                                    || self.is_escaped(self.i))
                            {
                                self.i += 1;
                            }
                            value = unescape_css(&self.s[start..self.i]);
                        }
                        self.strip_whitespace(0);
                        match self.at(self.i) {
                            Some('s' | 'S') => {
                                ignore_case = IgnoreCase::False;
                                self.strip_whitespace(1);
                            }
                            Some('i' | 'I') => {
                                ignore_case = IgnoreCase::True;
                                self.strip_whitespace(1);
                            }
                            _ => {}
                        }
                    }
                    if self.at(self.i) != Some(']') {
                        return Err("Attribute selector didn't terminate".to_owned());
                    }
                    self.i += 1;
                    tokens.push(Token::Attribute {
                        name,
                        action,
                        value,
                        namespace,
                        ignore_case,
                    });
                }
                ':' => {
                    if self.at(self.i + 1) == Some(':') {
                        let name = self.get_name(2)?.to_lowercase();
                        let data = if self.at(self.i) == Some('(') {
                            Some(self.read_value_with_parenthesis()?)
                        } else {
                            None
                        };
                        tokens.push(Token::PseudoElement { name, data });
                        continue;
                    }
                    let name = self.get_name(1)?.to_lowercase();
                    let mut data = PseudoData::None;
                    if self.at(self.i) == Some('(') {
                        if UNPACK_PSEUDOS.contains(&name.as_str()) {
                            if is_quote(self.at(self.i + 1)) {
                                return Err(format!("Pseudo-selector {name} cannot be quoted"));
                            }
                            let mut list = Vec::new();
                            let end = self.parse_selector(&mut list, self.i + 1)?;
                            self.i = end;
                            if self.at(self.i) != Some(')') {
                                return Err(format!(
                                    "Missing closing parenthesis in :{name} ({})",
                                    self.source
                                ));
                            }
                            self.i += 1;
                            data = PseudoData::Selectors(list);
                        } else {
                            let raw = self.read_value_with_parenthesis()?;
                            let mut chars: Vec<char> = raw.chars().collect();
                            if name == "contains" || name == "icontains" {
                                let first = chars.first().copied();
                                if first.is_some()
                                    && first == chars.last().copied()
                                    && is_quote(first)
                                {
                                    chars = if chars.len() >= 2 {
                                        chars[1..chars.len() - 1].to_vec()
                                    } else {
                                        Vec::new()
                                    };
                                }
                            }
                            data = PseudoData::Str(unescape_css(&chars));
                        }
                    }
                    tokens.push(Token::Pseudo { name, data });
                }
                ',' => {
                    finalize(&mut tokens, subselects)?;
                    self.strip_whitespace(1);
                }
                _ => {
                    if first == '/' && self.at(self.i + 1) == Some('*') {
                        let mut end = None;
                        let mut p = self.i + 2;
                        while p + 1 < self.s.len() {
                            if self.s[p] == '*' && self.s[p + 1] == '/' {
                                end = Some(p);
                                break;
                            }
                            p += 1;
                        }
                        let Some(end) = end else {
                            return Err("Comment was not terminated".to_owned());
                        };
                        self.i = end + 2;
                        if tokens.is_empty() {
                            self.strip_whitespace(0);
                        }
                        continue;
                    }
                    let mut namespace = None;
                    let mut name;
                    if first == '*' {
                        self.i += 1;
                        name = "*".to_owned();
                    } else if first == '|' {
                        name = String::new();
                        if self.at(self.i + 1) == Some('|') {
                            add_traversal(&mut tokens, Token::ColumnCombinator)?;
                            self.strip_whitespace(2);
                            continue;
                        }
                    } else if match_name(self.s, self.i).is_some() {
                        name = self.get_name(0)?;
                    } else {
                        break 'outer;
                    }
                    if self.at(self.i) == Some('|') && self.at(self.i + 1) != Some('|') {
                        namespace = Some(name);
                        if self.at(self.i + 1) == Some('*') {
                            name = "*".to_owned();
                            self.i += 2;
                        } else {
                            name = self.get_name(1)?;
                        }
                    }
                    tokens.push(if name == "*" {
                        Token::Universal { namespace }
                    } else {
                        Token::Tag { name, namespace }
                    });
                }
            }
        }
        finalize(&mut tokens, subselects)?;
        Ok(self.i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(sel: &str) -> String {
        match parse(sel) {
            Ok(list) => selectors_to_json(&list),
            Err(e) => format!("ERR {e}"),
        }
    }

    #[test]
    fn names_and_escapes() {
        assert_eq!(
            json(".a\\31 b"),
            r#"[[{"type":"attribute","name":"class","action":"element","value":"a1b","namespace":null,"ignoreCase":"quirks"}]]"#
        );
        assert_eq!(
            json(":is(a)b"),
            "ERR Missing closing parenthesis in :is (:is(a)b)"
        );
        assert_eq!(json("a)"), "ERR Unmatched selector: )");
        assert_eq!(json(""), "[]");
    }
}
