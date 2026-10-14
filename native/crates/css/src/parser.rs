//! Порт postcss 8.5.8 `lib/parser.js` (обычный `Parser`, не safe-вариант),
//! сохраняет ровно те raws, которые видит compress-сериализатор critters.

use std::borrow::Cow;

use dom::js;

use crate::tokenize::{K, Tok, Tokenizer};
use crate::{CssError, ErrorKind};

pub(crate) type Id = u32;

#[derive(Default, Debug)]
pub(crate) struct Body {
    pub nodes: Vec<Id>,
    pub semicolon: Option<bool>,
    pub after: String,
}

#[derive(Debug)]
pub(crate) enum SelState {
    /// critters не трогает: выводится `comma(selector).join(',')`.
    Untouched,
    /// Быстрый путь: итоговый сериализованный список селекторов (уже склеен через ',').
    Final(String),
    /// Медленный путь: `selector` хранит S2, `marked` — S1 (строку, чей
    /// `list.comma` даёт `$$markedSelectors`).
    Marked(String),
}

#[derive(Debug)]
pub(crate) struct Rule<'a> {
    pub selector: Cow<'a, str>,
    pub between: &'a str,
    /// `raws.ownSemicolon` ('' == undefined)
    pub own_semicolon: String,
    pub body: Body,
    pub remove: bool,
    pub sel_state: SelState,
}

#[derive(Debug)]
pub(crate) struct AtRule<'a> {
    pub name: &'a str,
    pub after_name: &'a str,
    pub params: Cow<'a, str>,
    pub params_raw: &'a str,
    pub between: &'a str,
    pub body: Option<Body>,
    pub remove: bool,
}

#[derive(Debug)]
pub(crate) struct Decl<'a> {
    pub prop: &'a str,
    pub between: &'a str,
    pub value: Cow<'a, str>,
    pub value_raw: &'a str,
    pub important: bool,
    /// `raws.important`
    pub important_raw: Option<&'a str>,
}

#[derive(Debug)]
pub(crate) struct Comment<'a> {
    pub text: &'a str,
}

#[derive(Debug)]
pub(crate) enum Data<'a> {
    Rule(Rule<'a>),
    AtRule(AtRule<'a>),
    Decl(Decl<'a>),
    Comment(Comment<'a>),
}

#[derive(Debug)]
pub(crate) struct Node<'a> {
    pub before: String,
    pub data: Data<'a>,
}

impl<'a> Node<'a> {
    pub fn body(&self) -> Option<&Body> {
        match &self.data {
            Data::Rule(r) => Some(&r.body),
            Data::AtRule(a) => a.body.as_ref(),
            _ => None,
        }
    }
    pub fn body_mut(&mut self) -> Option<&mut Body> {
        match &mut self.data {
            Data::Rule(r) => Some(&mut r.body),
            Data::AtRule(a) => a.body.as_mut(),
            _ => None,
        }
    }
    pub fn is_comment(&self) -> bool {
        matches!(self.data, Data::Comment(_))
    }
}

pub(crate) struct Ast<'a> {
    pub nodes: Vec<Node<'a>>,
    pub root: Body,
}

impl<'a> Ast<'a> {
    pub fn body(&self, c: Option<Id>) -> &Body {
        match c {
            None => &self.root,
            Some(id) => self.nodes[id as usize].body().expect("container"),
        }
    }
    pub fn body_mut(&mut self, c: Option<Id>) -> &mut Body {
        match c {
            None => &mut self.root,
            Some(id) => self.nodes[id as usize].body_mut().expect("container"),
        }
    }
}

struct Parser<'a> {
    src: &'a str,
    tk: Tokenizer<'a>,
    ast: Ast<'a>,
    stack: Vec<Id>,
    spaces: String,
    semicolon: bool,
}

fn syntax(msg: impl Into<String>) -> CssError {
    CssError::new(ErrorKind::Syntax, msg.into())
}

#[inline]
fn is_sc(k: K) -> bool {
    k == K::Space || k == K::Comment
}

pub(crate) fn parse(src: &str) -> Result<Ast<'_>, CssError> {
    let mut p = Parser {
        src,
        tk: Tokenizer::new(src),
        ast: Ast {
            nodes: Vec::with_capacity(src.len() / 24),
            root: Body::default(),
        },
        stack: Vec::new(),
        spaces: String::new(),
        semicolon: false,
    };
    p.parse()?;
    Ok(p.ast)
}

impl<'a> Parser<'a> {
    #[inline]
    fn text(&self, t: &Tok) -> &'a str {
        &self.src[t.start as usize..t.end as usize]
    }

    /// Конкатенация подряд идущих токенов.
    #[inline]
    fn span(&self, toks: &[Tok]) -> &'a str {
        match (toks.first(), toks.last()) {
            (Some(f), Some(l)) => &self.src[f.start as usize..l.end as usize],
            _ => "",
        }
    }

    fn next(&mut self) -> Result<Option<Tok>, CssError> {
        self.tk.next_token()
    }

    fn parse(&mut self) -> Result<(), CssError> {
        while !self.tk.end_of_file() {
            let Some(token) = self.next()? else { break };
            match token.kind {
                K::Space => {
                    let t = self.text(&token);
                    self.spaces.push_str(t);
                }
                K::Semicolon => self.free_semicolon(),
                K::CloseCurly => self.end()?,
                K::Comment => self.comment(token),
                K::AtWord => self.atrule(token)?,
                K::OpenCurly => self.empty_rule(),
                _ => self.other(token)?,
            }
        }
        self.end_file()
    }

    fn current(&self) -> Option<Id> {
        self.stack.last().copied()
    }

    fn init(&mut self, data: Data<'a>) -> Id {
        let before = std::mem::take(&mut self.spaces);
        self.init_with(before, data)
    }

    fn init_with(&mut self, before: String, data: Data<'a>) -> Id {
        let is_comment = matches!(data, Data::Comment(_));
        let id = self.ast.nodes.len() as Id;
        self.ast.nodes.push(Node { before, data });
        let cur = self.current();
        self.ast.body_mut(cur).nodes.push(id);
        if !is_comment {
            self.semicolon = false;
        }
        id
    }

    fn free_semicolon(&mut self) {
        self.spaces.push(';');
        let cur = self.current();
        if let Some(&prev) = self.ast.body(cur).nodes.last()
            && let Data::Rule(r) = &mut self.ast.nodes[prev as usize].data
            && r.own_semicolon.is_empty()
        {
            r.own_semicolon = std::mem::take(&mut self.spaces);
        }
    }

    fn end(&mut self) -> Result<(), CssError> {
        let cur = self.current();
        let semicolon = self.semicolon;
        let spaces = std::mem::take(&mut self.spaces);
        let body = self.ast.body_mut(cur);
        if !body.nodes.is_empty() {
            body.semicolon = Some(semicolon);
        }
        self.semicolon = false;
        body.after.push_str(&spaces);
        if self.stack.pop().is_none() {
            return Err(syntax("Unexpected }"));
        }
        Ok(())
    }

    fn end_file(&mut self) -> Result<(), CssError> {
        if !self.stack.is_empty() {
            return Err(syntax("Unclosed block"));
        }
        if !self.ast.root.nodes.is_empty() {
            self.ast.root.semicolon = Some(self.semicolon);
        }
        let spaces = std::mem::take(&mut self.spaces);
        self.ast.root.after.push_str(&spaces);
        Ok(())
    }

    fn comment(&mut self, token: Tok) {
        let t = self.text(&token);
        let inner = &t[2..t.len() - 2];
        let text = js::trim(inner);
        self.init(Data::Comment(Comment { text }));
    }

    fn empty_rule(&mut self) {
        let id = self.init(Data::Rule(Rule {
            selector: Cow::Borrowed(""),
            between: "",
            own_semicolon: String::new(),
            body: Body::default(),
            remove: false,
            sel_state: SelState::Untouched,
        }));
        self.stack.push(id);
    }

    fn atrule(&mut self, token: Tok) -> Result<(), CssError> {
        let name = &self.text(&token)[1..];
        if name.is_empty() {
            return Err(syntax("At-rule without name"));
        }
        let id = self.init(Data::AtRule(AtRule {
            name,
            after_name: "",
            params: Cow::Borrowed(""),
            params_raw: "",
            between: "",
            body: None,
            remove: false,
        }));

        let mut last = false;
        let mut open = false;
        let mut params: Vec<Tok> = Vec::new();
        let mut brackets: Vec<K> = Vec::new();

        while !self.tk.end_of_file() {
            let Some(token) = self.next()? else { break };
            let ty = token.kind;
            if ty == K::OpenParen || ty == K::OpenSquare {
                brackets.push(if ty == K::OpenParen {
                    K::CloseParen
                } else {
                    K::CloseSquare
                });
            } else if ty == K::OpenCurly && !brackets.is_empty() {
                brackets.push(K::CloseCurly);
            } else if brackets.last() == Some(&ty) {
                brackets.pop();
            }

            if brackets.is_empty() {
                if ty == K::Semicolon {
                    self.semicolon = true;
                    break;
                } else if ty == K::OpenCurly {
                    open = true;
                    break;
                } else if ty == K::CloseCurly {
                    self.end()?;
                    break;
                } else {
                    params.push(token);
                }
            } else {
                params.push(token);
            }

            if self.tk.end_of_file() {
                last = true;
                break;
            }
        }

        let between = self.spaces_and_comments_from_end(&mut params);
        let mut a_between = between;
        let mut after_name = "";
        let mut p_val: Cow<'a, str> = Cow::Borrowed("");
        let mut p_raw = "";
        if !params.is_empty() {
            let start = params.iter().take_while(|t| is_sc(t.kind)).count();
            after_name = self.span(&params[..start]);
            let (v, r) = self.raw(&params[start..], false);
            p_val = v;
            p_raw = r;
            if last {
                self.spaces = between.to_string();
                a_between = "";
            }
        }
        if let Data::AtRule(a) = &mut self.ast.nodes[id as usize].data {
            a.after_name = after_name;
            a.params = p_val;
            a.params_raw = p_raw;
            a.between = a_between;
            if open {
                a.body = Some(Body::default());
            }
        }
        if open {
            self.stack.push(id);
        }
        Ok(())
    }

    fn spaces_and_comments_from_end(&self, toks: &mut Vec<Tok>) -> &'a str {
        let n = toks.iter().rev().take_while(|t| is_sc(t.kind)).count();
        let cut = toks.len() - n;
        let s = self.span(&toks[cut..]);
        toks.truncate(cut);
        s
    }

    fn other(&mut self, start: Tok) -> Result<(), CssError> {
        let mut end = false;
        let mut colon = false;
        let mut brackets: Vec<K> = Vec::new();
        let mut has_bracket = false;
        let custom = self.text(&start).starts_with("--");

        let mut tokens: Vec<Tok> = Vec::new();
        let mut token = Some(start);
        while let Some(t) = token {
            let ty = t.kind;
            tokens.push(t);
            if ty == K::OpenParen || ty == K::OpenSquare {
                has_bracket = true;
                brackets.push(if ty == K::OpenParen {
                    K::CloseParen
                } else {
                    K::CloseSquare
                });
            } else if custom && colon && ty == K::OpenCurly {
                has_bracket = true;
                brackets.push(K::CloseCurly);
            } else if brackets.is_empty() {
                if ty == K::Semicolon {
                    if colon {
                        return self.decl(tokens, custom);
                    } else {
                        break;
                    }
                } else if ty == K::OpenCurly {
                    return self.rule(tokens);
                } else if ty == K::CloseCurly {
                    let t = tokens.pop().expect("token");
                    self.tk.back(t);
                    end = true;
                    break;
                } else if ty == K::Colon {
                    colon = true;
                }
            } else if brackets.last() == Some(&ty) {
                brackets.pop();
            }
            token = self.next()?;
        }

        if self.tk.end_of_file() {
            end = true;
        }
        if !brackets.is_empty() {
            debug_assert!(has_bracket);
            return Err(syntax("Unclosed bracket"));
        }

        if end && colon {
            if !custom {
                while let Some(t) = tokens.last() {
                    if !is_sc(t.kind) {
                        break;
                    }
                    let t = tokens.pop().expect("token");
                    self.tk.back(t);
                }
            }
            self.decl(tokens, custom)
        } else {
            Err(self.unknown_word(&tokens[0]))
        }
    }

    fn unknown_word(&self, t: &Tok) -> CssError {
        syntax(format!("Unknown word {}", self.text(t)))
    }

    fn rule(&mut self, mut tokens: Vec<Tok>) -> Result<(), CssError> {
        tokens.pop();
        let between = self.spaces_and_comments_from_end(&mut tokens);
        let (selector, _) = self.raw(&tokens, false);
        let id = self.init(Data::Rule(Rule {
            selector,
            between,
            own_semicolon: String::new(),
            body: Body::default(),
            remove: false,
            sel_state: SelState::Untouched,
        }));
        self.stack.push(id);
        Ok(())
    }

    /// `Parser#raw` — возвращает (обработанное значение, конкатенацию всех токенов).
    fn raw(&self, toks: &[Tok], custom: bool) -> (Cow<'a, str>, &'a str) {
        let raw = self.span(toks);
        let len = toks.len();
        // Быстрая проверка: нет комментариев и хвостовых пробелов для отбрасывания.
        let last_space = !custom && len > 0 && toks[len - 1].kind == K::Space;
        if !last_space && !toks.iter().any(|t| t.kind == K::Comment) {
            return (Cow::Borrowed(raw), raw);
        }
        let mut value = String::with_capacity(raw.len());
        let mut clean = true;
        for i in 0..len {
            let t = &toks[i];
            if t.kind == K::Space && i == len - 1 && !custom {
                clean = false;
            } else if t.kind == K::Comment {
                let prev_safe = i == 0 || toks[i - 1].kind == K::Space;
                let next_safe = i + 1 >= len || toks[i + 1].kind == K::Space;
                if !prev_safe && !next_safe {
                    if value.ends_with(',') {
                        clean = false;
                    } else {
                        value.push_str(self.text(t));
                    }
                } else {
                    clean = false;
                }
            } else {
                value.push_str(self.text(t));
            }
        }
        if clean {
            (Cow::Borrowed(raw), raw)
        } else {
            (Cow::Owned(value), raw)
        }
    }

    fn decl(&mut self, mut tokens: Vec<Tok>, custom: bool) -> Result<(), CssError> {
        let semicolon = tokens.last().is_some_and(|t| t.kind == K::Semicolon);
        if semicolon {
            tokens.pop();
        }

        let mut h = 0usize;
        let mut t = tokens.len();
        let mut before = std::mem::take(&mut self.spaces);
        while tokens[h].kind != K::Word {
            if t - h == 1 {
                return Err(self.unknown_word(&tokens[h]));
            }
            before.push_str(self.text(&tokens[h]));
            h += 1;
        }

        let prop_start = h;
        while h < t {
            let ty = tokens[h].kind;
            if ty == K::Colon || is_sc(ty) {
                break;
            }
            h += 1;
        }
        let mut prop = self.span(&tokens[prop_start..h]);

        let between_start = h;
        while h < t {
            let tok = tokens[h];
            h += 1;
            if tok.kind == K::Colon {
                break;
            } else if tok.kind == K::Word && self.text(&tok).bytes().any(is_word_byte) {
                return Err(self.unknown_word(&tok));
            }
        }
        let mut between_end = h;

        if prop.starts_with('_') || prop.starts_with('*') {
            before.push_str(&prop[..1]);
            prop = &prop[1..];
        }

        let first_spaces_start = h;
        while h < t && is_sc(tokens[h].kind) {
            h += 1;
        }

        let mut important = false;
        let mut important_raw: Option<&'a str> = None;
        let mut i = t;
        while i > h {
            i -= 1;
            let tok = tokens[i];
            let txt = self.text(&tok);
            if txt.eq_ignore_ascii_case("!important") {
                important = true;
                // stringFrom(tokens, i), затем spacesFromEnd(tokens)
                let old_t = t;
                t = i;
                while t > h && tokens[t - 1].kind == K::Space {
                    t -= 1;
                }
                let s = self.span(&tokens[t..old_t]);
                if s != " !important" {
                    important_raw = Some(s);
                }
                break;
            } else if txt.eq_ignore_ascii_case("important") {
                // cache = tokens.slice(0); индексы относительно `h`.
                let mut ct = t;
                let mut str_start = t; // str == span(tokens[str_start..t])
                let mut j = i - h;
                while j > 0 {
                    let ty = tokens[h + j].kind;
                    let s = self.span(&tokens[str_start..t]);
                    if js::trim(s).starts_with('!') && ty != K::Space {
                        break;
                    }
                    ct -= 1;
                    str_start = ct;
                    j -= 1;
                }
                let s = self.span(&tokens[str_start..t]);
                if js::trim(s).starts_with('!') {
                    important = true;
                    important_raw = Some(s);
                    t = ct;
                }
            }
            if !is_sc(tok.kind) {
                break;
            }
        }

        let has_word = tokens[h..t].iter().any(|x| !is_sc(x.kind));
        let value_toks = if has_word {
            between_end = h;
            &tokens[h..t]
        } else {
            &tokens[first_spaces_start..t]
        };
        let between = self.span(&tokens[between_start..between_end]);
        let (value, value_raw) = self.raw(value_toks, custom);

        if value.contains(':') && !custom {
            self.check_missed_semicolon(&tokens[h..t])?;
        }

        self.init_with(
            before,
            Data::Decl(Decl {
                prop,
                between,
                value,
                value_raw,
                important,
                important_raw,
            }),
        );
        self.semicolon = semicolon;
        Ok(())
    }

    fn check_missed_semicolon(&self, toks: &[Tok]) -> Result<(), CssError> {
        let mut brackets: i32 = 0;
        let mut prev: Option<&Tok> = None;
        for tok in toks {
            if tok.kind == K::OpenParen {
                brackets += 1;
            }
            if tok.kind == K::CloseParen {
                brackets -= 1;
            }
            if brackets == 0 && tok.kind == K::Colon {
                match prev {
                    None => return Err(syntax("Double colon")),
                    Some(p) if p.kind == K::Word && self.text(p) == "progid" => continue,
                    Some(_) => return Err(syntax("Missed semicolon")),
                }
            }
            prev = Some(tok);
        }
        Ok(())
    }
}

#[inline]
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
