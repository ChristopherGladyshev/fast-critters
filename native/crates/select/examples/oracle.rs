//! Драйвер оракула для `oracle/select.mjs`.
//!
//! stdin, по одному JSON-объекту на строку:
//!   {"dom": NODE, "contexts": [node index, ...], "selectors": ["...", ...]}
//! NODE (дерево htmlparser2, дети по порядку; индексы узлов — pre-order, document = 0):
//!   ["doc", [NODE...]] | ["e", name, [[k, v]...], [NODE...]] | ["t", data]
//!   | ["c", data] | ["d", name, data] | ["x", [NODE...]]   (CDATA)
//! stdout, по одной строке на каждую входную:
//!   [ по селектору: {"p": tokens | {"err": msg},
//!                    "x": [ по контексту: {"c": msg | null,
//!                                          "one": index | null | {"err": msg},
//!                                          "all": [index...] | {"err": msg}} ]} ]
//! С `--time` также пишет в stderr время, потраченное в `select_one` (после компиляции).

use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use dom::{Dom, Element, ElementKind, NodeData, NodeId};

#[derive(Debug)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or(&J::Null),
            _ => &J::Null,
        }
    }
    fn arr(&self) -> &[J] {
        match self {
            J::Arr(items) => items,
            _ => &[],
        }
    }
    fn str(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => panic!("expected string, got {self:?}"),
        }
    }
    fn num(&self) -> f64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("expected number"),
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }
    fn value(&mut self) -> J {
        self.ws();
        match self.b[self.i] {
            b'n' => {
                self.i += 4;
                J::Null
            }
            b't' => {
                self.i += 4;
                J::Bool(true)
            }
            b'f' => {
                self.i += 5;
                J::Bool(false)
            }
            b'"' => J::Str(self.string()),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b[self.i] == b']' {
                    self.i += 1;
                    return J::Arr(items);
                }
                loop {
                    items.push(self.value());
                    self.ws();
                    let c = self.b[self.i];
                    self.i += 1;
                    if c == b']' {
                        return J::Arr(items);
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut entries = Vec::new();
                self.ws();
                if self.b[self.i] == b'}' {
                    self.i += 1;
                    return J::Obj(entries);
                }
                loop {
                    self.ws();
                    let key = self.string();
                    self.ws();
                    self.i += 1; // ':'
                    entries.push((key, self.value()));
                    self.ws();
                    let c = self.b[self.i];
                    self.i += 1;
                    if c == b'}' {
                        return J::Obj(entries);
                    }
                }
            }
            _ => {
                let start = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                J::Num(
                    std::str::from_utf8(&self.b[start..self.i])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
        }
    }
    fn hex4(&mut self) -> u32 {
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap();
        self.i += 4;
        u32::from_str_radix(s, 16).unwrap()
    }
    fn string(&mut self) -> String {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).unwrap(),
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    let ch = match e {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => {
                            let u = self.hex4();
                            if (0xD800..0xDC00).contains(&u)
                                && self.b.get(self.i) == Some(&b'\\')
                                && self.b.get(self.i + 1) == Some(&b'u')
                            {
                                let save = self.i;
                                self.i += 2;
                                let lo = self.hex4();
                                if (0xDC00..0xE000).contains(&lo) {
                                    char::from_u32(0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00))
                                        .unwrap()
                                } else {
                                    self.i = save;
                                    '\u{FFFD}'
                                }
                            } else {
                                char::from_u32(u).unwrap_or('\u{FFFD}')
                            }
                        }
                        other => other as char,
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }
}

fn parse_json(s: &str) -> J {
    P {
        b: s.as_bytes(),
        i: 0,
    }
    .value()
}

fn json_str(out: &mut String, s: &str) {
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

fn build(dom: &mut Dom, parent: Option<NodeId>, node: &J) {
    let items = node.arr();
    let kind = items[0].str();
    let (id, children) = match kind {
        "doc" => (dom::DOCUMENT, Some(&items[1])),
        "e" => {
            let name = items[1].str();
            let mut attrs = dom::Attrs::new();
            for pair in items[2].arr() {
                let pair = pair.arr();
                attrs.insert_if_absent(pair[0].str().to_owned(), pair[1].str().to_owned());
            }
            let id = dom.create(NodeData::Element(Element {
                name: name.to_owned(),
                attrs,
                kind: ElementKind::for_name(name),
            }));
            (id, Some(&items[3]))
        }
        "t" => (dom.create(NodeData::Text(items[1].str().to_owned())), None),
        "c" => (
            dom.create(NodeData::Comment(items[1].str().to_owned())),
            None,
        ),
        "d" => (
            dom.create(NodeData::Directive {
                name: items[1].str().to_owned(),
                data: items[2].str().to_owned(),
            }),
            None,
        ),
        "x" => (dom.create(NodeData::CData), Some(&items[1])),
        other => panic!("unknown node kind {other}"),
    };
    if let Some(parent) = parent {
        dom.append_child(parent, id);
    }
    if let Some(children) = children {
        for child in children.arr() {
            build(dom, Some(id), child);
        }
    }
}

fn main() {
    let time = std::env::args().any(|a| a == "--time");
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let mut select_one_time = Duration::ZERO;
    let mut compile_time = Duration::ZERO;
    let profile = std::env::var_os("SELECT_PROFILE").is_some();
    let mut slowest: Vec<(Duration, String)> = Vec::new();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let input = parse_json(&line);
        let mut dom = Dom::new();
        build(&mut dom, None, input.get("dom"));
        let contexts: Vec<NodeId> = input
            .get("contexts")
            .arr()
            .iter()
            .map(|c| c.num() as NodeId)
            .collect();
        let only_one = matches!(input.get("onlyOne"), J::Bool(true));
        let mut s = String::from("[");
        for (i, selector) in input.get("selectors").arr().iter().enumerate() {
            let selector = selector.str();
            if i > 0 {
                s.push(',');
            }
            s.push_str("{\"p\":");
            match select::parse(selector) {
                Ok(tokens) => s.push_str(&select::selectors_to_json(&tokens)),
                Err(e) => {
                    s.push_str("{\"err\":");
                    json_str(&mut s, &e);
                    s.push('}');
                }
            }
            s.push_str(",\"x\":[");
            for (k, &context) in contexts.iter().enumerate() {
                if k > 0 {
                    s.push(',');
                }
                let started = Instant::now();
                let compiled = select::compile(&dom, selector, context);
                compile_time += started.elapsed();
                match compiled {
                    Err(e) => {
                        s.push_str("{\"c\":");
                        json_str(&mut s, &e);
                        s.push_str(",\"one\":{\"err\":");
                        json_str(&mut s, &e);
                        s.push_str("},\"all\":{\"err\":");
                        json_str(&mut s, &e);
                        s.push_str("}}");
                    }
                    Ok(query) => {
                        s.push_str("{\"c\":null,\"one\":");
                        let started = Instant::now();
                        let one = select::try_select_one(&dom, &query, context);
                        let spent = started.elapsed();
                        select_one_time += spent;
                        if profile {
                            slowest.push((spent, selector.to_owned()));
                        }
                        match one {
                            Ok(Some(id)) => s.push_str(&id.to_string()),
                            Ok(None) => s.push_str("null"),
                            Err(e) => {
                                s.push_str("{\"err\":");
                                json_str(&mut s, &e);
                                s.push('}');
                            }
                        }
                        s.push_str(",\"all\":");
                        if only_one {
                            s.push_str("null");
                        } else {
                            match select::try_select_all(&dom, &query, context) {
                                Ok(ids) => {
                                    s.push('[');
                                    for (n, id) in ids.iter().enumerate() {
                                        if n > 0 {
                                            s.push(',');
                                        }
                                        s.push_str(&id.to_string());
                                    }
                                    s.push(']');
                                }
                                Err(e) => {
                                    s.push_str("{\"err\":");
                                    json_str(&mut s, &e);
                                    s.push('}');
                                }
                            }
                        }
                        s.push('}');
                    }
                }
            }
            s.push_str("]}");
        }
        s.push(']');
        writeln!(out, "{s}").unwrap();
    }
    if profile {
        slowest.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        for (spent, selector) in slowest.iter().take(25) {
            eprintln!("{:>9.3} ms  {selector}", spent.as_secs_f64() * 1e3);
        }
    }
    if time {
        eprintln!(
            "rust compile: {:.3} ms, select_one: {:.3} ms",
            compile_time.as_secs_f64() * 1e3,
            select_one_time.as_secs_f64() * 1e3
        );
    }
}
