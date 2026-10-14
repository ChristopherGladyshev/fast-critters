//! critters 0.0.23 `processStyle` (первый проход `markOnly`, второй —
//! `applyMarkedSelectors` + отсев keyframes/font-face) для опций,
//! которые использует Next.js: keyframes='critical', allowRules=[], pruneSource=false,
//! fonts=false.

use std::borrow::Cow;
use std::collections::HashSet;

use dom::js;

use crate::list::{comma, set_selectors};
use crate::parser::{Ast, Data, Id, SelState};
use crate::{CssError, ErrorKind, Keyframes};

pub(crate) struct Ctx<'e> {
    pub exists: &'e mut dyn FnMut(&str) -> Result<bool, String>,
    pub failed: Vec<String>,
    pub critical_keyframes: HashSet<String>,
    pub keyframes: Keyframes,
    include_next: bool,
    include_all: bool,
    exclude_next: bool,
    exclude_all: bool,
}

impl<'e> Ctx<'e> {
    pub fn new(
        exists: &'e mut dyn FnMut(&str) -> Result<bool, String>,
        keyframes: Keyframes,
    ) -> Self {
        Ctx {
            exists,
            failed: Vec::new(),
            critical_keyframes: HashSet::new(),
            keyframes,
            include_next: false,
            include_all: false,
            exclude_next: false,
            exclude_all: false,
        }
    }

    /// Предикат `filterSelectors` из processStyle.
    fn check(&mut self, sel: &str) -> bool {
        if sel == ":root" || sel == "html" || sel == "body" || is_before_after(sel) {
            return true;
        }
        let s = transform_selector(sel);
        if s.is_empty() {
            return false;
        }
        match (self.exists)(&s) {
            Ok(b) => b,
            Err(m) => {
                self.failed.push(format!("{s} -> {m}"));
                false
            }
        }
    }

    fn comment_marker(&mut self, text: &str) {
        // text.match(/^(?<!\! )critters:(.*)/) — `.` останавливается на концах строк.
        let Some(rest) = text.strip_prefix("critters:") else {
            return;
        };
        let end = rest
            .find(['\n', '\r', '\u{2028}', '\u{2029}'])
            .unwrap_or(rest.len());
        match &rest[..end] {
            "include" => self.include_next = true,
            "exclude" => self.exclude_next = true,
            "include start" => self.include_all = true,
            "include end" => self.include_all = false,
            "exclude start" => self.exclude_all = true,
            "exclude end" => self.exclude_all = false,
            _ => {}
        }
    }
}

fn is_before_after(sel: &str) -> bool {
    let Some(s) = sel.strip_prefix(':') else {
        return false;
    };
    let s = s.strip_prefix(':').unwrap_or(s);
    s == "before" || s == "after"
}

#[inline]
fn is_az_dash(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'-'
}

/// Длина в байтах серии JS-пробелов в начале `s`.
fn ws_len(s: &str) -> usize {
    s.len() - js::trim_start(s).len()
}

/// `.replace(/(?<!\\)::?[a-z-]+(?![a-z-(])/gi, '')`
fn strip_pseudos(s: &str) -> Cow<'_, str> {
    let b = s.as_bytes();
    let n = b.len();
    let letters_at = |k: usize| -> Option<usize> {
        let mut e = k;
        while e < n && is_az_dash(b[e]) {
            e += 1;
        }
        if e > k && (e == n || b[e] != b'(') {
            Some(e)
        } else {
            None
        }
    };
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut i = 0;
    while i < n {
        if b[i] == b':' && (i == 0 || b[i - 1] != b'\\') {
            let m = if i + 1 < n && b[i + 1] == b':' {
                letters_at(i + 2)
            } else {
                None
            }
            .or_else(|| letters_at(i + 1));
            if let Some(e) = m {
                let o = out.get_or_insert_with(|| String::with_capacity(n));
                o.push_str(&s[copied..i]);
                copied = e;
                i = e;
                continue;
            }
        }
        i += 1;
    }
    match out {
        None => Cow::Borrowed(s),
        Some(mut o) => {
            o.push_str(&s[copied..]);
            Cow::Owned(o)
        }
    }
}

/// `.replace(/::?not\(\s*\)/g, '')`
fn strip_empty_not(s: &str) -> Cow<'_, str> {
    let b = s.as_bytes();
    let n = b.len();
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut i = 0;
    while i < n {
        if b[i] == b':' {
            let k = if i + 1 < n && b[i + 1] == b':' {
                i + 2
            } else {
                i + 1
            };
            if s[k..].starts_with("not(") {
                let p = k + 4;
                let p = p + ws_len(&s[p..]);
                if p < n && b[p] == b')' {
                    let o = out.get_or_insert_with(|| String::with_capacity(n));
                    o.push_str(&s[copied..i]);
                    copied = p + 1;
                    i = p + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    match out {
        None => Cow::Borrowed(s),
        Some(mut o) => {
            o.push_str(&s[copied..]);
            Cow::Owned(o)
        }
    }
}

/// `.replace(/\(\s*,/g, '(')` (open=true) или `.replace(/,\s*\)/g, ')')`.
fn collapse(s: &str, open: bool) -> Cow<'_, str> {
    let (first, second, keep) = if open {
        (b'(', b',', "(")
    } else {
        (b',', b')', ")")
    };
    let b = s.as_bytes();
    let n = b.len();
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut i = 0;
    while i < n {
        if b[i] == first {
            let p = i + 1 + ws_len(&s[i + 1..]);
            if p < n && b[p] == second {
                let o = out.get_or_insert_with(|| String::with_capacity(n));
                o.push_str(&s[copied..i]);
                o.push_str(keep);
                copied = p + 1;
                i = p + 1;
                continue;
            }
        }
        i += 1;
    }
    match out {
        None => Cow::Borrowed(s),
        Some(mut o) => {
            o.push_str(&s[copied..]);
            Cow::Owned(o)
        }
    }
}

fn own<'a>(c: Cow<'_, str>, orig: &'a str) -> Cow<'a, str> {
    match c {
        Cow::Borrowed(_) => Cow::Borrowed(orig),
        Cow::Owned(o) => Cow::Owned(o),
    }
}

/// Переписывание селектора, которое critters делает перед вызовом `exists`.
pub fn transform_selector(sel: &str) -> Cow<'_, str> {
    let b = sel.as_bytes();
    if !b.contains(&b':') && !b.contains(&b'(') {
        return Cow::Borrowed(js::trim(sel));
    }
    let s1 = own(strip_pseudos(sel), sel);
    let s2 = match strip_empty_not(&s1) {
        Cow::Borrowed(_) => s1,
        Cow::Owned(o) => Cow::Owned(o),
    };
    let s3 = match collapse(&s2, true) {
        Cow::Borrowed(_) => s2,
        Cow::Owned(o) => Cow::Owned(o),
    };
    let s4 = match collapse(&s3, false) {
        Cow::Borrowed(_) => s3,
        Cow::Owned(o) => Cow::Owned(o),
    };
    match s4 {
        Cow::Borrowed(x) => Cow::Borrowed(js::trim(x)),
        Cow::Owned(o) => {
            let t = js::trim(&o);
            if t.len() == o.len() {
                Cow::Owned(o)
            } else {
                Cow::Owned(t.to_string())
            }
        }
    }
}

pub(crate) fn has_nested_rules(ast: &Ast<'_>, id: Id) -> bool {
    let node = &ast.nodes[id as usize];
    let Some(body) = node.body() else {
        return false;
    };
    if body.nodes.is_empty() {
        return false;
    }
    if let Data::AtRule(a) = &node.data
        && (a.name == "keyframes" || a.name == "-webkit-keyframes")
    {
        return false;
    }
    body.nodes
        .iter()
        .any(|&c| matches!(ast.nodes[c as usize].data, Data::Rule(_) | Data::AtRule(_)))
}

fn is_removed(ast: &Ast<'_>, id: Id) -> bool {
    match &ast.nodes[id as usize].data {
        Data::Rule(r) => r.remove,
        Data::AtRule(a) => a.remove,
        _ => false,
    }
}

fn needs_slow_path(s: &str) -> bool {
    s.bytes()
        .any(|b| matches!(b, b'\\' | b'"' | b'\'' | b'(' | b')'))
}

/// Первый проход: `walkStyleRules(ast, markOnly(predicate))`.
pub(crate) fn walk1(ast: &mut Ast<'_>, ctx: &mut Ctx<'_>, c: Option<Id>) {
    let n = ast.body(c).nodes.len();
    for k in 0..n {
        let id = ast.body(c).nodes[k];
        if has_nested_rules(ast, id) {
            walk1(ast, ctx, Some(id));
        }
        mark_only(ast, ctx, id);
    }
}

fn mark_only(ast: &mut Ast<'_>, ctx: &mut Ctx<'_>, id: Id) {
    let remove = match &ast.nodes[id as usize].data {
        Data::Comment(cm) => {
            let text = cm.text;
            ctx.comment_marker(text);
            false
        }
        Data::Decl(_) => false,
        Data::AtRule(a) => {
            if a.name == "font-face" {
                false
            } else {
                match &a.body {
                    Some(b) => !b.nodes.iter().any(|&c| !is_removed(ast, c)),
                    None => false,
                }
            }
        }
        Data::Rule(_) => {
            mark_rule(ast, ctx, id);
            return;
        }
    };
    if remove && let Data::AtRule(a) = &mut ast.nodes[id as usize].data {
        a.remove = true;
    }
}

fn mark_rule(ast: &mut Ast<'_>, ctx: &mut Ctx<'_>, id: Id) {
    let Data::Rule(rule) = &ast.nodes[id as usize].data else {
        unreachable!()
    };
    let s0: Cow<'_, str> = match &rule.selector {
        Cow::Borrowed(s) => Cow::Borrowed(*s),
        Cow::Owned(s) => Cow::Owned(s.clone()),
    };
    let between = rule.between;
    let a = comma(&s0);

    // predicate(rule): Some(verdict) и список, из которого берётся `$$markedSelectors`.
    let mut kept: Option<Vec<&str>> = None; // None => путь маркера (S1 == S0)
    let verdict = if ctx.include_next {
        ctx.include_next = false;
        true
    } else if ctx.exclude_next {
        ctx.exclude_next = false;
        false
    } else if ctx.include_all {
        true
    } else if ctx.exclude_all {
        false
    } else {
        let mut f = Vec::with_capacity(a.len());
        for &item in &a {
            if ctx.check(item) {
                f.push(item);
            }
        }
        let empty = f.is_empty();
        kept = Some(f);
        if empty {
            false
        } else {
            let body = &rule.body;
            for &c in &body.nodes {
                if let Data::Decl(d) = &ast.nodes[c as usize].data
                    && (d.prop == "animation" || d.prop == "animation-name")
                {
                    for name in d.value.split(js::is_whitespace) {
                        if !name.is_empty() && !ctx.critical_keyframes.contains(name) {
                            ctx.critical_keyframes.insert(name.to_string());
                        }
                    }
                }
            }
            body.nodes.iter().any(|&c| !is_removed(ast, c))
        }
    };

    let state;
    let mut new_selector: Option<String> = None;
    if !verdict {
        // Удалено: по селектору больше ничего не наблюдаемо.
        state = SelState::Untouched;
    } else if !needs_slow_path(&s0) && (kept.is_some() || !a[..a.len() - 1].contains(&"")) {
        // Без кавычек/экранирования/скобок `list.comma` обратимо проходит через
        // склейку сеттера, кроме пустых элементов перед последним (их
        // судьба зависит от пробелов в разделителе).

        state = SelState::Final(kept.as_ref().unwrap_or(&a).join(","));
    } else {
        let s1 = match &kept {
            Some(f) => set_selectors(&s0, between, f),
            None => s0.to_string(),
        };
        new_selector = Some(set_selectors(&s1, between, &a));
        state = SelState::Marked(s1);
    }
    drop(a);
    let Data::Rule(rule) = &mut ast.nodes[id as usize].data else {
        unreachable!()
    };
    rule.remove = !verdict;
    rule.sel_state = state;
    if let Some(s) = new_selector {
        rule.selector = Cow::Owned(s);
    }
}

/// Второй проход: `walkStyleRulesWithReverseMirror(ast, null, ...)`.
pub(crate) fn walk2(ast: &mut Ast<'_>, ctx: &Ctx<'_>, c: Option<Id>) -> Result<(), CssError> {
    let nodes = std::mem::take(&mut ast.body_mut(c).nodes);
    let mut kept = Vec::with_capacity(nodes.len());
    for &id in &nodes {
        if has_nested_rules(ast, id) {
            walk2(ast, ctx, Some(id))?;
        }
        if second_pass(ast, ctx, id)? {
            kept.push(id);
        }
    }
    ast.body_mut(c).nodes = kept;
    Ok(())
}

fn second_pass(ast: &mut Ast<'_>, ctx: &Ctx<'_>, id: Id) -> Result<bool, CssError> {
    match &mut ast.nodes[id as usize].data {
        Data::Rule(r) => {
            if r.remove {
                return Ok(false);
            }
            if let SelState::Marked(s1) = &r.sel_state {
                let m = comma(s1);
                let s3 = set_selectors(&r.selector, r.between, &m);
                drop(m);
                r.selector = Cow::Owned(s3);
                r.sel_state = SelState::Untouched;
            }
            Ok(true)
        }
        Data::AtRule(a) => {
            if a.remove {
                return Ok(false);
            }
            if a.name == "keyframes" {
                return Ok(match ctx.keyframes {
                    Keyframes::None => false,
                    Keyframes::All => true,
                    Keyframes::Critical => ctx.critical_keyframes.contains(a.params.as_ref()),
                });
            }
            if a.name == "font-face" {
                if a.body.is_none() {
                    return Err(CssError::new(
                        ErrorKind::Runtime,
                        "rule.nodes is not iterable".to_string(),
                    ));
                }
                return Ok(false);
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}
