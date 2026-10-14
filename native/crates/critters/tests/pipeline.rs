//! Golden-тесты целого документа: `expected.html` / `logs.json` получены из стокового critters 0.0.23
//! (перегенерировать: `node oracle/pipeline-golden.mjs`).

use std::fs;
use std::path::Path;

use critters::{FileCache, Level, Options, Outcome, process};

/// `logs.json` — это `[["warn" | "info", message], ...]`; минимальный ридер под эту форму.
fn read_logs(json: &str) -> Vec<(String, String)> {
    let mut strings = Vec::new();
    let mut chars = json.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('u') => {
                        let hex: String = chars.by_ref().take(4).collect();
                        s.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                    }
                    Some(other) => s.push(other),
                    None => break,
                },
                other => s.push(other),
            }
        }
        strings.push(s);
    }
    strings
        .chunks(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect()
}

#[test]
fn matches_stock_critters() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut cases = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        let html = fs::read_to_string(dir.join("input.html")).unwrap();
        let expected = fs::read_to_string(dir.join("expected.html")).unwrap();
        let path = dir.to_str().unwrap();
        // Пути в сообщениях хранятся относительно фикстуры.
        let expected_logs: Vec<(String, String)> =
            read_logs(&fs::read_to_string(dir.join("logs.json")).unwrap())
                .into_iter()
                .map(|(level, message)| (level, message.replace("<DIR>", path)))
                .collect();
        let options = Options {
            path,
            public_path: "/_next/",
        };

        let Outcome::Html { html: out, logs } = process(&html, &options, &FileCache::new(false))
        else {
            panic!("{} fell back", dir.display());
        };
        assert_eq!(out, expected, "{}", dir.display());
        let logs: Vec<(String, String)> = logs
            .into_iter()
            .map(|log| {
                let level = match log.level {
                    Level::Info => "info",
                    Level::Warn => "warn",
                };
                (level.to_owned(), log.message)
            })
            .collect();
        assert_eq!(logs, expected_logs, "{}", dir.display());
        cases += 1;
    }
    assert!(cases >= 5);
}

#[test]
fn falls_back_without_html_element() {
    let options = Options {
        path: "/tmp",
        public_path: "/_next/",
    };
    assert!(matches!(
        process("<p>no root</p>", &options, &FileCache::new(false)),
        Outcome::Fallback(_)
    ));
    let relative = Options {
        path: "tmp/../x",
        public_path: "/_next/",
    };
    assert!(matches!(
        process("<html></html>", &relative, &FileCache::new(false)),
        Outcome::Fallback(_)
    ));
}
