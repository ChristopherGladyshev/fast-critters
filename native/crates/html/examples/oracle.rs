//! Oracle-обвязка: читает строки `{"html": "..."}` из stdin, пишет строки
//! `{"out": render(parse_document(html)), "tree": <канонический дамп>}`.
//! С `--bench <file> <iterations>` вместо этого замеряет время парсинга и рендера.

use std::io::{BufRead, BufWriter, Write};
use std::time::Instant;

use dom::{DOCUMENT, Dom, ElementKind, NodeData, NodeId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--bench" {
        bench(
            &args[2],
            args.get(3).and_then(|n| n.parse().ok()).unwrap_or(50),
        );
        return;
    }
    let stdin = std::io::stdin();
    let mut out = BufWriter::new(std::io::stdout());
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let html = read_html_field(&line);
        let dom = html::parse_document(&html);
        let rendered = html::render(&dom, DOCUMENT);
        let mut json = String::from("{\"out\":");
        write_json_string(&mut json, &rendered);
        json.push_str(",\"tree\":");
        dump(&dom, DOCUMENT, &mut json);
        json.push('}');
        writeln!(out, "{json}").expect("stdout");
    }
}

fn bench(path: &str, iterations: usize) {
    let src = std::fs::read_to_string(path).expect("read");
    let mut parse_best = f64::MAX;
    let mut render_best = f64::MAX;
    let mut parse_sum = 0.0;
    let mut render_sum = 0.0;
    let mut len = 0;
    for _ in 0..iterations {
        let t = Instant::now();
        let dom = html::parse_document(&src);
        let parse = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let out = html::render(&dom, DOCUMENT);
        let render = t.elapsed().as_secs_f64() * 1e3;
        len = out.len();
        parse_best = parse_best.min(parse);
        render_best = render_best.min(render);
        parse_sum += parse;
        render_sum += render;
    }
    let n = iterations as f64;
    println!(
        "{path}: {} bytes in, {len} out; parse best {parse_best:.3} ms avg {:.3} ms; render best {render_best:.3} ms avg {:.3} ms",
        src.len(),
        parse_sum / n,
        render_sum / n
    );
}

fn dump(dom: &Dom, id: NodeId, out: &mut String) {
    let node = dom.node(id);
    for &child in &node.children {
        assert_eq!(
            dom.node(child).parent,
            Some(id),
            "parent pointer of {child}"
        );
    }
    match &node.data {
        NodeData::Document => {
            out.push_str("{\"t\":\"root\"");
            children(dom, id, out);
        }
        NodeData::Element(element) => {
            let t = match element.kind {
                ElementKind::Tag => "tag",
                ElementKind::Script => "script",
                ElementKind::Style => "style",
            };
            out.push_str("{\"t\":\"");
            out.push_str(t);
            out.push_str("\",\"n\":");
            write_json_string(out, &element.name);
            out.push_str(",\"a\":[");
            for (i, (key, value)) in element.attrs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('[');
                write_json_string(out, key);
                out.push(',');
                write_json_string(out, value);
                out.push(']');
            }
            out.push(']');
            children(dom, id, out);
        }
        NodeData::Text(data) => {
            out.push_str("{\"t\":\"text\",\"d\":");
            write_json_string(out, data);
        }
        NodeData::Comment(data) => {
            out.push_str("{\"t\":\"comment\",\"d\":");
            write_json_string(out, data);
        }
        NodeData::Directive { name, data } => {
            out.push_str("{\"t\":\"directive\",\"n\":");
            write_json_string(out, name);
            out.push_str(",\"d\":");
            write_json_string(out, data);
        }
        NodeData::CData => {
            out.push_str("{\"t\":\"cdata\"");
            children(dom, id, out);
        }
    }
    out.push('}');
}

fn children(dom: &Dom, id: NodeId, out: &mut String) {
    out.push_str(",\"c\":[");
    for (i, &child) in dom.children(id).iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        dump(dom, child, out);
    }
    out.push(']');
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Извлекает строку `"html"` из JSON-объекта с одним полем.
fn read_html_field(line: &str) -> String {
    let key = "\"html\"";
    let start = line.find(key).expect("html field") + key.len();
    let rest = line[start..].trim_start();
    let rest = rest.strip_prefix(':').expect("colon").trim_start();
    parse_json_string(rest)
}

fn parse_json_string(s: &str) -> String {
    let mut chars = s.chars();
    assert_eq!(chars.next(), Some('"'));
    let mut out = String::new();
    loop {
        match chars.next().expect("unterminated string") {
            '"' => return out,
            '\\' => match chars.next().expect("escape") {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                '/' => out.push('/'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'u' => {
                    let hex = |chars: &mut std::str::Chars| {
                        let h: String = chars.take(4).collect();
                        u32::from_str_radix(&h, 16).expect("hex")
                    };
                    let unit = hex(&mut chars);
                    let code = if (0xD800..0xDC00).contains(&unit) {
                        let rest = chars.as_str();
                        assert!(rest.starts_with("\\u"), "lone surrogate");
                        chars.next();
                        chars.next();
                        let low = hex(&mut chars);
                        0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                    } else {
                        unit
                    };
                    out.push(char::from_u32(code).expect("valid code point"));
                }
                other => panic!("bad escape {other}"),
            },
            c => out.push(c),
        }
    }
}
