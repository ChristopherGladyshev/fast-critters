//! postcss `Stringifier`, управляемый билдером `serializeStylesheet` из critters
//! в compress-режиме. Все raws, которые читает Stringifier, заданы в распарсенном дереве,
//! поэтому вывод через `raw()` не нужен.

use dom::js;

use crate::list::comma;
use crate::parser::{Ast, Body, Data, Id, SelState};
use crate::{CssError, ErrorKind};

pub(crate) fn serialize(ast: &Ast<'_>) -> Result<String, CssError> {
    let mut out = String::with_capacity(4096);
    body(ast, &ast.root, &mut out)?;
    if !ast.root.after.is_empty() {
        out.push_str(js::trim(&ast.root.after));
    }
    Ok(out)
}

fn body(ast: &Ast<'_>, b: &Body, out: &mut String) -> Result<(), CssError> {
    let n = b.nodes.len();
    if n == 0 {
        return Ok(());
    }
    let mut last = n - 1;
    while last > 0 && ast.nodes[b.nodes[last] as usize].is_comment() {
        last -= 1;
    }
    let semicolon = b.semicolon.unwrap_or(false);
    for (i, &id) in b.nodes.iter().enumerate() {
        let before = &ast.nodes[id as usize].before;
        if !before.is_empty() {
            out.push_str(js::trim(before));
        }
        node(ast, id, last != i || semicolon, out)?;
    }
    Ok(())
}

fn block_end(ast: &Ast<'_>, b: &Body, out: &mut String) -> Result<(), CssError> {
    if !b.nodes.is_empty() {
        body(ast, b, out)?;
    }
    if !b.after.is_empty() {
        out.push_str(js::trim(&b.after));
    }
    if b.semicolon == Some(true) {
        // cssStr.slice(0, -1) отбрасывает одну UTF-16 code unit.
        if let Some(c) = out.pop()
            && c.len_utf16() == 2
        {
            return Err(CssError::new(
                ErrorKind::Unsupported,
                "slice(0, -1) would split a surrogate pair".to_string(),
            ));
        }
    }
    out.push('}');
    Ok(())
}

fn node(ast: &Ast<'_>, id: Id, semicolon: bool, out: &mut String) -> Result<(), CssError> {
    match &ast.nodes[id as usize].data {
        Data::Comment(_) => {}
        Data::Decl(d) => {
            if d.value.contains("</style>") {
                return Ok(());
            }
            let important = if d.important {
                d.important_raw.unwrap_or(" !important")
            } else {
                ""
            };
            let semi = if semicolon { ";" } else { "" };
            // result.replace(prefix, prefix.trim()) — prefix стоит на индексе 0.
            let prefix_len = d.prop.len() + d.between.len();
            let mut prefix = String::with_capacity(prefix_len);
            prefix.push_str(d.prop);
            prefix.push_str(d.between);
            let trimmed = js::trim(&prefix);
            if trimmed.contains('$') {
                let mut after = String::new();
                after.push_str(d.value_raw);
                after.push_str(important);
                after.push_str(semi);
                expand_replacement(trimmed, &prefix, &after, out);
            } else {
                out.push_str(trimmed);
            }
            out.push_str(d.value_raw);
            out.push_str(important);
            out.push_str(semi);
        }
        Data::Rule(r) => {
            match &r.sel_state {
                SelState::Final(s) => out.push_str(s),
                SelState::Untouched => push_joined(&comma(&r.selector), out),
                SelState::Marked(_) => unreachable!("applyMarkedSelectors ran in pass 2"),
            }
            out.push('{');
            block_end(ast, &r.body, out)?;
            if !r.own_semicolon.is_empty() {
                out.push_str(js::trim(&r.own_semicolon));
            }
        }
        Data::AtRule(a) => {
            let start = out.len();
            out.push('@');
            out.push_str(a.name);
            out.push_str(a.after_name);
            if !a.params.is_empty() {
                out.push_str(a.params_raw);
            }
            out.push_str(a.between);
            match &a.body {
                Some(b) => {
                    // result.replace(/\s\{$/, '{')
                    if let Some(c) = out[start..].chars().next_back()
                        && js::is_whitespace(c)
                    {
                        out.truncate(out.len() - c.len_utf8());
                    }
                    out.push('{');
                    block_end(ast, b, out)?;
                }
                None => {
                    if semicolon {
                        out.push(';');
                    }
                    // cssStr += result.trim()
                    let s = &out[start..];
                    let t = js::trim(s);
                    if t.len() != s.len() {
                        let t = t.to_string();
                        out.truncate(start);
                        out.push_str(&t);
                    }
                }
            }
        }
    }
    Ok(())
}

fn push_joined(items: &[&str], out: &mut String) {
    for (i, s) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(s);
    }
}

/// JS `GetSubstitution` для строкового паттерна (без групп), совпавшего на индексе 0.
fn expand_replacement(repl: &str, matched: &str, after: &str, out: &mut String) {
    let b = repl.as_bytes();
    let mut i = 0;
    let mut lit = 0;
    while i < b.len() {
        if b[i] == b'$' && i + 1 < b.len() {
            let sub: Option<&str> = match b[i + 1] {
                b'$' => Some("$"),
                b'&' => Some(matched),
                b'`' => Some(""),
                b'\'' => Some(after),
                _ => None,
            };
            if let Some(s) = sub {
                out.push_str(&repl[lit..i]);
                out.push_str(s);
                i += 2;
                lit = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&repl[lit..]);
}
