//! Драйвер оракула: читает JSONL `{"css": "...", "seed": n}` (файл из argv[1] или
//! stdin), запускает `css::process_style` с общим детерминированным предикатом
//! и печатает по одному JSON-результату на строку. `oracle/css.mjs` сверяет с JS.
//!
//! `bench <files...>` вместо этого замеряет время process_style по файлам.

use std::io::{BufRead, Read, Write};
use std::time::Instant;

fn fnv(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// seed -1: всегда true; -2: всегда false; иначе FNV-1a ^ seed.
fn predicate(seed: i64, sel: &str) -> Result<bool, String> {
    match seed {
        -1 => Ok(true),
        -2 => Ok(false),
        _ => {
            let h = fnv(sel) ^ (seed as u32);
            match h % 7 {
                0 => Err(format!("boom:{sel}")),
                1..=3 => Ok(false),
                _ => Ok(true),
            }
        }
    }
}

fn parse_json_string(b: &[u8], i: &mut usize) -> String {
    assert_eq!(b[*i], b'"');
    *i += 1;
    let mut units: Vec<u16> = Vec::new();
    let mut out = String::new();
    let flush = |units: &mut Vec<u16>, out: &mut String| {
        if !units.is_empty() {
            out.push_str(&String::from_utf16(units).expect("lone surrogate in input"));
            units.clear();
        }
    };
    loop {
        let c = b[*i];
        if c == b'"' {
            *i += 1;
            break;
        }
        if c == b'\\' {
            let e = b[*i + 1];
            *i += 2;
            let ch = match e {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{8}',
                b'f' => '\u{c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => {
                    let hex = std::str::from_utf8(&b[*i..*i + 4]).unwrap();
                    *i += 4;
                    units.push(u16::from_str_radix(hex, 16).unwrap());
                    continue;
                }
                _ => panic!("bad escape"),
            };
            flush(&mut units, &mut out);
            out.push(ch);
        } else {
            flush(&mut units, &mut out);
            let s = std::str::from_utf8(&b[*i..]).unwrap();
            let ch = s.chars().next().unwrap();
            out.push(ch);
            *i += ch.len_utf8();
        }
    }
    flush(&mut units, &mut out);
    out
}

fn parse_line(line: &str) -> (String, i64) {
    let b = line.as_bytes();
    let mut css = None;
    let mut seed = None;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let key = parse_json_string(b, &mut i);
            while b[i] == b':' || b[i] == b' ' {
                i += 1;
            }
            if key == "css" {
                css = Some(parse_json_string(b, &mut i));
            } else if key == "seed" {
                let start = i;
                while b[i] == b'-' || b[i].is_ascii_digit() {
                    i += 1;
                }
                seed = Some(line[start..i].parse().unwrap());
            }
        } else {
            i += 1;
        }
    }
    (css.expect("css"), seed.expect("seed"))
}

fn js(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn run_case(css_text: &str, seed: i64) -> String {
    let mut calls: Vec<String> = Vec::new();
    let res = css::process_style(css_text, &mut |s: &str| {
        calls.push(s.to_string());
        predicate(seed, s)
    });
    let mut o = String::from("{");
    match res {
        Ok(p) => {
            o.push_str("\"threw\":false,\"skipped\":");
            o.push_str(if p.skipped { "true" } else { "false" });
            o.push_str(",\"css\":");
            js(&p.css, &mut o);
            o.push_str(",\"failed\":[");
            for (i, f) in p.failed_selectors.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                js(f, &mut o);
            }
            o.push(']');
        }
        Err(e) => {
            o.push_str("\"threw\":true,\"kind\":");
            js(&format!("{:?}", e.kind), &mut o);
            o.push_str(",\"message\":");
            js(&e.message, &mut o);
        }
    }
    o.push_str(",\"calls\":[");
    for (i, c) in calls.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        js(c, &mut o);
    }
    o.push_str("]}");
    o
}

fn bench(files: &[String]) {
    let sheets: Vec<String> = files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();
    for (label, seed) in [("always-true", -1i64), ("hash-seed-1", 1)] {
        // прогрев
        for s in &sheets {
            let _ = css::process_style(s, &mut |x| predicate(seed, x));
        }
        let iters = 20;
        let t = Instant::now();
        let mut total_len = 0usize;
        for _ in 0..iters {
            for s in &sheets {
                let r = css::process_style(s, &mut |x| predicate(seed, x)).unwrap();
                total_len += r.css.len();
            }
        }
        let el = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        let max = sheets.iter().max_by_key(|s| s.len()).unwrap();
        let t2 = Instant::now();
        for _ in 0..iters {
            let _ = css::process_style(max, &mut |x| predicate(seed, x)).unwrap();
        }
        let el2 = t2.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        println!(
            "{label}: {} files, {:.3} ms total per pass ({:.3} ms/file); largest ({} B) {:.3} ms [{}]",
            sheets.len(),
            el,
            el / sheets.len() as f64,
            max.len(),
            el2,
            total_len % 7
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("bench") {
        bench(&args[2..]);
        return;
    }
    let mut input = String::new();
    match args.get(1) {
        Some(p) => input = std::fs::read_to_string(p).unwrap(),
        None => {
            std::io::stdin().read_to_string(&mut input).unwrap();
        }
    }
    let stdout = std::io::stdout();
    let mut w = std::io::BufWriter::new(stdout.lock());
    for line in input.as_bytes().lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let (css_text, seed) = parse_line(&line);
        writeln!(w, "{}", run_case(&css_text, seed)).unwrap();
    }
}
