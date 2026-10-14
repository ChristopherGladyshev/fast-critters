//! Порт postcss 8.5.8 `lib/tokenize.js`.
//!
//! Смещения — байтовые смещения UTF-8. Все разделители, которые смотрит JS-токенизатор,
//! ASCII, поэтому побайтовое сканирование эквивалентно UTF-16 сканированию в JS, с одним
//! явно обработанным исключением: backslash, экранирующий астральный символ (JS
//! поглощает только high surrogate, а low surrogate начинает новый
//! word-токен). Это эмулируется через `pending_lo`.

use crate::{CssError, ErrorKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum K {
    Space,
    Word,
    AtWord,
    Str,
    Brackets,
    Comment,
    OpenSquare,
    CloseSquare,
    OpenCurly,
    CloseCurly,
    Colon,
    Semicolon,
    OpenParen,
    CloseParen,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tok {
    pub kind: K,
    pub start: u32,
    pub end: u32,
}

const fn table(chars: &[u8]) -> [bool; 256] {
    let mut t = [false; 256];
    let mut i = 0;
    while i < chars.len() {
        t[chars[i] as usize] = true;
        i += 1;
    }
    t
}

/// `RE_WORD_END` без альтернативы `/(?=\*)`.
static WORD_END: [bool; 256] = table(b"\t\n\x0c\r !\"#'():;@[\\]{}");
/// `RE_AT_END`.
static AT_END: [bool; 256] = table(b"\t\n\x0c\r \"#'()/;[\\]{}");
/// Второй класс символов `RE_BAD_BRACKET`.
static BAD_BRACKET: [bool; 256] = table(b"\r\n\"'(/\\");

pub(crate) struct Tokenizer<'a> {
    css: &'a [u8],
    pos: usize,
    returned: Vec<Tok>,
    buffer: Vec<(u32, u32)>,
    pending_lo: bool,
}

fn unclosed(what: &str) -> CssError {
    CssError::new(ErrorKind::Syntax, format!("Unclosed {what}"))
}

impl<'a> Tokenizer<'a> {
    pub fn new(css: &'a str) -> Self {
        Tokenizer {
            css: css.as_bytes(),
            pos: 0,
            returned: Vec::new(),
            buffer: Vec::new(),
            pending_lo: false,
        }
    }

    #[inline]
    fn at(&self, i: usize) -> Option<u8> {
        self.css.get(i).copied()
    }

    #[inline]
    pub fn end_of_file(&self) -> bool {
        self.returned.is_empty() && self.pos >= self.css.len() && !self.pending_lo
    }

    #[inline]
    pub fn back(&mut self, t: Tok) {
        self.returned.push(t);
    }

    fn find_byte(&self, b: u8, from: usize) -> Option<usize> {
        if from >= self.css.len() {
            return None;
        }
        self.css[from..]
            .iter()
            .position(|&c| c == b)
            .map(|p| p + from)
    }

    /// Первый индекс `>= from`, совпадающий с `RE_WORD_END`.
    #[inline]
    fn word_end(&self, from: usize) -> usize {
        let css = self.css;
        let mut i = from;
        while i < css.len() {
            let c = css[i];
            if WORD_END[c as usize] || (c == b'/' && css.get(i + 1) == Some(&b'*')) {
                return i;
            }
            i += 1;
        }
        css.len()
    }

    fn tok(kind: K, start: usize, end: usize) -> Tok {
        Tok {
            kind,
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Tok>, CssError> {
        if let Some(t) = self.returned.pop() {
            return Ok(Some(t));
        }
        let css = self.css;
        let len = css.len();
        let pos = self.pos;
        if self.pending_lo {
            // Low surrogate экранированного астрального символа начинает word-токен.
            self.pending_lo = false;
            let end = self.word_end(pos);
            let t = Self::tok(K::Word, pos, end);
            self.buffer.push((t.start, t.end));
            self.pos = end;
            return Ok(Some(t));
        }
        if pos >= len {
            return Ok(None);
        }
        let code = css[pos];
        let t = match code {
            b'\n' | b' ' | b'\t' | b'\r' | 0x0c => {
                let mut next = pos + 1;
                while next < len && matches!(css[next], b' ' | b'\n' | b'\t' | b'\r' | 0x0c) {
                    next += 1;
                }
                Self::tok(K::Space, pos, next)
            }
            b'[' => Self::tok(K::OpenSquare, pos, pos + 1),
            b']' => Self::tok(K::CloseSquare, pos, pos + 1),
            b'{' => Self::tok(K::OpenCurly, pos, pos + 1),
            b'}' => Self::tok(K::CloseCurly, pos, pos + 1),
            b':' => Self::tok(K::Colon, pos, pos + 1),
            b';' => Self::tok(K::Semicolon, pos, pos + 1),
            b')' => Self::tok(K::CloseParen, pos, pos + 1),
            b'(' => {
                let is_url = match self.buffer.pop() {
                    Some((s, e)) => &css[s as usize..e as usize] == b"url",
                    None => false,
                };
                let n = self.at(pos + 1);
                if is_url && !matches!(n, Some(b'\'' | b'"' | b' ' | b'\n' | b'\t' | 0x0c | b'\r'))
                {
                    let mut next = pos;
                    loop {
                        let mut escaped = false;
                        next = match self.find_byte(b')', next + 1) {
                            Some(n) => n,
                            None => return Err(unclosed("bracket")),
                        };
                        let mut ep = next;
                        while ep > 0 && css[ep - 1] == b'\\' {
                            ep -= 1;
                            escaped = !escaped;
                        }
                        if !escaped {
                            break;
                        }
                    }
                    Self::tok(K::Brackets, pos, next + 1)
                } else {
                    match self.find_byte(b')', pos + 1) {
                        Some(next) if !bad_bracket(&css[pos..=next]) => {
                            Self::tok(K::Brackets, pos, next + 1)
                        }
                        _ => Self::tok(K::OpenParen, pos, pos + 1),
                    }
                }
            }
            b'\'' | b'"' => {
                let mut next = pos;
                loop {
                    let mut escaped = false;
                    next = match self.find_byte(code, next + 1) {
                        Some(n) => n,
                        None => return Err(unclosed("string")),
                    };
                    let mut ep = next;
                    while ep > 0 && css[ep - 1] == b'\\' {
                        ep -= 1;
                        escaped = !escaped;
                    }
                    if !escaped {
                        break;
                    }
                }
                Self::tok(K::Str, pos, next + 1)
            }
            b'@' => {
                let mut i = pos + 1;
                while i < len && !AT_END[css[i] as usize] {
                    i += 1;
                }
                Self::tok(K::AtWord, pos, i)
            }
            b'\\' => {
                let mut next = pos;
                let mut escape = true;
                while self.at(next + 1) == Some(b'\\') {
                    next += 1;
                    escape = !escape;
                }
                let c = self.at(next + 1);
                let mut end = next + 1;
                if escape && !matches!(c, Some(b'/' | b' ' | b'\n' | b'\t' | b'\r' | 0x0c)) {
                    next += 1;
                    match c {
                        None => end = len,
                        Some(c) if c < 0x80 => {
                            if c.is_ascii_hexdigit() {
                                while self.at(next + 1).is_some_and(|c| c.is_ascii_hexdigit()) {
                                    next += 1;
                                }
                                if self.at(next + 1) == Some(b' ') {
                                    next += 1;
                                }
                            }
                            end = next + 1;
                        }
                        Some(c) => {
                            let w = utf8_width(c);
                            end = next + w;
                            if w == 4 {
                                self.pending_lo = true;
                            }
                        }
                    }
                }
                Self::tok(K::Word, pos, end)
            }
            _ => {
                if code == b'/' && self.at(pos + 1) == Some(b'*') {
                    let end = match find_sub(css, b"*/", pos + 2) {
                        Some(i) => i + 2,
                        None => return Err(unclosed("comment")),
                    };
                    Self::tok(K::Comment, pos, end)
                } else {
                    let end = self.word_end(pos + 1);
                    let t = Self::tok(K::Word, pos, end);
                    self.buffer.push((t.start, t.end));
                    t
                }
            }
        };
        self.pos = t.end as usize;
        Ok(Some(t))
    }
}

fn utf8_width(b: u8) -> usize {
    if b >= 0xF0 {
        4
    } else if b >= 0xE0 {
        3
    } else {
        2
    }
}

pub(crate) fn find_sub(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from > h.len() || h.len() - from < n.len() {
        return None;
    }
    h[from..]
        .windows(n.len())
        .position(|w| w == n)
        .map(|p| p + from)
}

/// `/.[\r\n"'(/\\]/.test(content)` — `.` исключает JS-концы строк.
fn bad_bracket(content: &[u8]) -> bool {
    for i in 1..content.len() {
        if BAD_BRACKET[content[i] as usize] {
            let p = content[i - 1];
            let prev_is_lt = p == b'\n'
                || p == b'\r'
                || (i >= 3
                    && content[i - 3] == 0xE2
                    && content[i - 2] == 0x80
                    && (p == 0xA8 || p == 0xA9));
            if !prev_is_lt {
                return true;
            }
        }
    }
    false
}
