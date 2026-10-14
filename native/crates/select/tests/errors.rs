//! Сообщения об ошибках и особенности JS-объектов, сверенные с css-select 5.2.2 в Node.

use dom::{DOCUMENT, Dom};

/// `<html><head></head><body><div id=a class="x y"><p>hi</p><a href=x constructor=1>z</a></div><span>q</span></body></html>`
fn build() -> (Dom, usize) {
    let mut d = Dom::new();
    let html = d.create_element("html");
    d.append_child(DOCUMENT, html);
    let head = d.create_element("head");
    d.append_child(html, head);
    let body = d.create_element("body");
    d.append_child(html, body);
    let div = d.create_element("div");
    d.set_attr(div, "id", "a");
    d.set_attr(div, "class", "x y");
    d.append_child(body, div);
    let p = d.create_element("p");
    d.append_child(div, p);
    let a = d.create_element("a");
    d.set_attr(a, "href", "x");
    d.set_attr(a, "constructor", "1");
    d.append_child(div, a);
    let span = d.create_element("span");
    d.append_child(body, span);
    (d, html)
}

#[test]
fn js_quirks() {
    let (d, html) = build();
    let names = |r: Result<Vec<usize>, String>| -> Result<Vec<String>, String> {
        r.map(|ids| {
            ids.iter()
                .map(|&id| d.name(id).unwrap().to_owned())
                .collect()
        })
    };
    let run = |sel: &str| {
        names(select::compile(&d, sel, html).and_then(|q| select::try_select_all(&d, &q, html)))
    };
    let err = |m: &str| Err::<Vec<String>, String>(m.to_owned());
    let ok = |v: &[&str]| Ok::<Vec<String>, String>(v.iter().map(|s| s.to_string()).collect());

    assert_eq!(
        run("[constructor=a i]"),
        err("attr.toLowerCase is not a function")
    );
    assert_eq!(
        run("[constructor|=a]"),
        err("attr.substr is not a function")
    );
    assert_eq!(
        run("[constructor|=ab]"),
        err("attr.charAt is not a function")
    );
    assert_eq!(run("[__proto__|=a]"), err("attr.charAt is not a function"));
    assert_eq!(
        run("[constructor^=x]"),
        err("_a.startsWith is not a function")
    );
    assert_eq!(
        run("[constructor$=x i]"),
        err("_a.substr is not a function")
    );
    assert_eq!(
        run("[constructor*=x]"),
        err("_a.includes is not a function")
    );
    assert_eq!(
        run("[constructor*={ i]"),
        ok(&["head", "body", "div", "p", "span"])
    );
    assert_eq!(
        run("[constructor~=\"{\"]"),
        ok(&["head", "body", "div", "p", "span"])
    );
    assert_eq!(
        run("[constructor!=\"\"]"),
        ok(&["head", "body", "div", "p", "a", "span"])
    );
    assert_eq!(run("[constructor]"), ok(&["a"]));
    assert_eq!(
        run(":__proto__"),
        err("filters_js_1.filters[name] is not a function")
    );
    assert_eq!(
        run(":constructor"),
        ok(&["head", "body", "div", "p", "a", "span"])
    );
    assert_eq!(
        run(":host(a b, c)"),
        err(
            "Unknown pseudo-class :host([object Object],[object Object],[object Object],[object Object])"
        )
    );
    assert_eq!(
        run(":icontains"),
        err("Cannot read properties of null (reading 'toLowerCase')")
    );
    assert_eq!(
        run(":nth-child"),
        err("Cannot read properties of null (reading 'trim')")
    );
    assert_eq!(
        run(":empty()"),
        err("Pseudo-class :empty doesn't have any arguments")
    );
    assert_eq!(
        run(":link(a)"),
        err("Pseudo link doesn't have any arguments")
    );
    assert_eq!(run(":foo"), err("Unknown pseudo-class :foo"));
    assert_eq!(run(":hover"), ok(&[]));
    assert_eq!(run("div:has(+ span)"), ok(&["div"]));
    assert_eq!(run("a + "), ok(&[]));
    assert_eq!(
        run("::before"),
        err("Pseudo-elements are not supported by css-select")
    );
    assert_eq!(run("[x^=\"\"]::before"), ok(&[]));
    assert_eq!(
        run("|a"),
        err("Namespaced tag names are not yet supported by css-select")
    );
    assert_eq!(
        run("[*|b]"),
        err("Namespaced attributes are not yet supported by css-select")
    );
    assert_eq!(
        run(":nth-child(1000000000000000000000000000n+1)"),
        ok(&["head", "div", "p"])
    );
    assert_eq!(run("> body"), ok(&["body"]));

    // Тот же запрос с поиском от контекста документа.
    let q = select::compile(&d, "span", DOCUMENT).unwrap();
    assert_eq!(select::select_one(&d, &q, DOCUMENT), Some(7));
    assert!(select::is_match(&d, &q, 7));
    assert!(!select::is_match(&d, &q, DOCUMENT));
    assert_eq!(
        select::query_selector(&d, ":is(a", html),
        Err("Missing closing parenthesis in :is (:is(a)".to_owned())
    );
}
