//! Golden-кейсы, сгенерированные из css-select 5.2.2 (`node oracle/select.mjs golden`).
//! html: "<!doctype html><html lang=\"en\"><head><title>T</title></head><body class=\"Main x\"><div id=\"a\" class=\"foo bar\" data-x=\"en-US\"><p class=\"foo\">Hello <b>World</b></p><p></p><span lang=\"EN\" title=\"a b\">x</span><a href=\"/x\" rel=\"Stylesheet\">l</a></div><ul><li>1</li><li class=\"c\">2</li><li>3</li><!--c--><li>4</li></ul><input type=\"Checkbox\" checked><select><option>o1</option><option selected>o2</option></select><section data-critters-container><div class=\"in\"><p>deep</p></div></section></body></html>"

use dom::{DOCUMENT, Dom, NodeData, NodeId};

fn add(d: &mut Dom, parent: NodeId, data: NodeData) -> NodeId {
    let id = d.create(data);
    d.append_child(parent, id);
    id
}

fn el(d: &mut Dom, parent: NodeId, name: &str, attrs: &[(&str, &str)]) -> NodeId {
    let id = d.create_element(name);
    for (k, v) in attrs {
        d.set_attr(id, k, v);
    }
    d.append_child(parent, id);
    id
}

#[allow(unused_variables)]
fn build() -> Dom {
    let mut d = Dom::new();
    add(
        &mut d,
        DOCUMENT,
        NodeData::Directive {
            name: "!doctype".into(),
            data: "!doctype html".into(),
        },
    );
    let n2 = el(&mut d, DOCUMENT, "html", &[("lang", "en")]);
    let n3 = el(&mut d, n2, "head", &[]);
    let n4 = el(&mut d, n3, "title", &[]);
    add(&mut d, n4, NodeData::Text("T".into()));
    let n6 = el(&mut d, n2, "body", &[("class", "Main x")]);
    let n7 = el(
        &mut d,
        n6,
        "div",
        &[("id", "a"), ("class", "foo bar"), ("data-x", "en-US")],
    );
    let n8 = el(&mut d, n7, "p", &[("class", "foo")]);
    add(&mut d, n8, NodeData::Text("Hello ".into()));
    let n10 = el(&mut d, n8, "b", &[]);
    add(&mut d, n10, NodeData::Text("World".into()));
    let n12 = el(&mut d, n7, "p", &[]);
    let n13 = el(&mut d, n7, "span", &[("lang", "EN"), ("title", "a b")]);
    add(&mut d, n13, NodeData::Text("x".into()));
    let n15 = el(&mut d, n7, "a", &[("href", "/x"), ("rel", "Stylesheet")]);
    add(&mut d, n15, NodeData::Text("l".into()));
    let n17 = el(&mut d, n6, "ul", &[]);
    let n18 = el(&mut d, n17, "li", &[]);
    add(&mut d, n18, NodeData::Text("1".into()));
    let n20 = el(&mut d, n17, "li", &[("class", "c")]);
    add(&mut d, n20, NodeData::Text("2".into()));
    let n22 = el(&mut d, n17, "li", &[]);
    add(&mut d, n22, NodeData::Text("3".into()));
    add(&mut d, n17, NodeData::Comment("c".into()));
    let n25 = el(&mut d, n17, "li", &[]);
    add(&mut d, n25, NodeData::Text("4".into()));
    let n27 = el(
        &mut d,
        n6,
        "input",
        &[("type", "Checkbox"), ("checked", "")],
    );
    let n28 = el(&mut d, n6, "select", &[]);
    let n29 = el(&mut d, n28, "option", &[]);
    add(&mut d, n29, NodeData::Text("o1".into()));
    let n31 = el(&mut d, n28, "option", &[("selected", "")]);
    add(&mut d, n31, NodeData::Text("o2".into()));
    let n33 = el(&mut d, n6, "section", &[("data-critters-container", "")]);
    let n34 = el(&mut d, n33, "div", &[("class", "in")]);
    let n35 = el(&mut d, n34, "p", &[]);
    add(&mut d, n35, NodeData::Text("deep".into()));
    d
}

/// (селектор, контекст, ошибка компиляции, selectOne, selectAll)
type Case = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<NodeId>,
    &'static [NodeId],
);

const CASES: &[Case] = &[
    ("div", "doc", None, Some(7), &[7, 34]),
    ("div", "html", None, Some(7), &[7, 34]),
    ("div", "container", None, Some(34), &[34]),
    ("DIV", "doc", None, Some(7), &[7, 34]),
    ("DIV", "html", None, Some(7), &[7, 34]),
    ("DIV", "container", None, Some(34), &[34]),
    (".foo", "doc", None, Some(7), &[7, 8]),
    (".foo", "html", None, Some(7), &[7, 8]),
    (".foo", "container", None, None, &[]),
    (".Foo", "doc", None, None, &[]),
    (".Foo", "html", None, None, &[]),
    (".Foo", "container", None, None, &[]),
    ("#a", "doc", None, Some(7), &[7]),
    ("#a", "html", None, Some(7), &[7]),
    ("#a", "container", None, None, &[]),
    ("p.foo b", "doc", None, Some(10), &[10]),
    ("p.foo b", "html", None, Some(10), &[10]),
    ("p.foo b", "container", None, None, &[]),
    ("div > p", "doc", None, Some(8), &[8, 12, 35]),
    ("div > p", "html", None, Some(8), &[8, 12, 35]),
    ("div > p", "container", None, Some(35), &[35]),
    ("p + span", "doc", None, Some(13), &[13]),
    ("p + span", "html", None, Some(13), &[13]),
    ("p + span", "container", None, None, &[]),
    ("p ~ a", "doc", None, Some(15), &[15]),
    ("p ~ a", "html", None, Some(15), &[15]),
    ("p ~ a", "container", None, None, &[]),
    ("li:nth-child(2)", "doc", None, Some(20), &[20]),
    ("li:nth-child(2)", "html", None, Some(20), &[20]),
    ("li:nth-child(2)", "container", None, None, &[]),
    ("li:nth-child(odd)", "doc", None, Some(18), &[18, 22]),
    ("li:nth-child(odd)", "html", None, Some(18), &[18, 22]),
    ("li:nth-child(odd)", "container", None, None, &[]),
    ("li:nth-last-child(1)", "doc", None, Some(25), &[25]),
    ("li:nth-last-child(1)", "html", None, Some(25), &[25]),
    ("li:nth-last-child(1)", "container", None, None, &[]),
    ("li:nth-of-type(2n)", "doc", None, Some(20), &[20, 25]),
    ("li:nth-of-type(2n)", "html", None, Some(20), &[20, 25]),
    ("li:nth-of-type(2n)", "container", None, None, &[]),
    ("li:first-child", "doc", None, Some(18), &[18]),
    ("li:first-child", "html", None, Some(18), &[18]),
    ("li:first-child", "container", None, None, &[]),
    ("li:last-child", "doc", None, Some(25), &[25]),
    ("li:last-child", "html", None, Some(25), &[25]),
    ("li:last-child", "container", None, None, &[]),
    ("p:empty", "doc", None, Some(12), &[12]),
    ("p:empty", "html", None, Some(12), &[12]),
    ("p:empty", "container", None, None, &[]),
    ("p:only-of-type", "doc", None, Some(35), &[35]),
    ("p:only-of-type", "html", None, Some(35), &[35]),
    ("p:only-of-type", "container", None, Some(35), &[35]),
    (":contains(World)", "doc", None, Some(2), &[2, 6, 7, 8, 10]),
    (":contains(World)", "html", None, Some(6), &[6, 7, 8, 10]),
    (":contains(World)", "container", None, None, &[]),
    (":icontains(world)", "doc", None, Some(2), &[2, 6, 7, 8, 10]),
    (":icontains(world)", "html", None, Some(6), &[6, 7, 8, 10]),
    (":icontains(world)", "container", None, None, &[]),
    ("p:contains(\"Hello\")", "doc", None, Some(8), &[8]),
    ("p:contains(\"Hello\")", "html", None, Some(8), &[8]),
    ("p:contains(\"Hello\")", "container", None, None, &[]),
    ("[lang|=en]", "doc", None, Some(2), &[2, 13]),
    ("[lang|=en]", "html", None, Some(13), &[13]),
    ("[lang|=en]", "container", None, None, &[]),
    ("[lang|=EN s]", "doc", None, Some(13), &[13]),
    ("[lang|=EN s]", "html", None, Some(13), &[13]),
    ("[lang|=EN s]", "container", None, None, &[]),
    ("[data-x|=en]", "doc", None, Some(7), &[7]),
    ("[data-x|=en]", "html", None, Some(7), &[7]),
    ("[data-x|=en]", "container", None, None, &[]),
    ("[rel=stylesheet]", "doc", None, Some(15), &[15]),
    ("[rel=stylesheet]", "html", None, Some(15), &[15]),
    ("[rel=stylesheet]", "container", None, None, &[]),
    ("[rel=stylesheet s]", "doc", None, None, &[]),
    ("[rel=stylesheet s]", "html", None, None, &[]),
    ("[rel=stylesheet s]", "container", None, None, &[]),
    ("[title~=b]", "doc", None, Some(13), &[13]),
    ("[title~=b]", "html", None, Some(13), &[13]),
    ("[title~=b]", "container", None, None, &[]),
    ("[title~=\"a b\"]", "doc", None, None, &[]),
    ("[title~=\"a b\"]", "html", None, None, &[]),
    ("[title~=\"a b\"]", "container", None, None, &[]),
    ("[href^=\"/\"]", "doc", None, Some(15), &[15]),
    ("[href^=\"/\"]", "html", None, Some(15), &[15]),
    ("[href^=\"/\"]", "container", None, None, &[]),
    ("[href$=x]", "doc", None, Some(15), &[15]),
    ("[href$=x]", "html", None, Some(15), &[15]),
    ("[href$=x]", "container", None, None, &[]),
    ("[href*=\"\"]", "doc", None, None, &[]),
    ("[href*=\"\"]", "html", None, None, &[]),
    ("[href*=\"\"]", "container", None, None, &[]),
    (
        "[data-x!=en-US]",
        "doc",
        None,
        Some(2),
        &[
            2, 3, 4, 6, 8, 10, 12, 13, 15, 17, 18, 20, 22, 25, 27, 28, 29, 31, 33, 34, 35,
        ],
    ),
    (
        "[data-x!=en-US]",
        "html",
        None,
        Some(3),
        &[
            3, 4, 6, 8, 10, 12, 13, 15, 17, 18, 20, 22, 25, 27, 28, 29, 31, 33, 34, 35,
        ],
    ),
    ("[data-x!=en-US]", "container", None, Some(34), &[34, 35]),
    (
        ":not(p)",
        "doc",
        None,
        Some(2),
        &[
            2, 3, 4, 6, 7, 10, 13, 15, 17, 18, 20, 22, 25, 27, 28, 29, 31, 33, 34,
        ],
    ),
    (
        ":not(p)",
        "html",
        None,
        Some(3),
        &[
            3, 4, 6, 7, 10, 13, 15, 17, 18, 20, 22, 25, 27, 28, 29, 31, 33, 34,
        ],
    ),
    (":not(p)", "container", None, Some(34), &[34]),
    ("div:has(> b)", "doc", None, None, &[]),
    ("div:has(> b)", "html", None, None, &[]),
    ("div:has(> b)", "container", None, None, &[]),
    ("div:has(b)", "doc", None, Some(7), &[7]),
    ("div:has(b)", "html", None, Some(7), &[7]),
    ("div:has(b)", "container", None, None, &[]),
    ("li:has(+ li.c)", "doc", None, Some(18), &[18]),
    ("li:has(+ li.c)", "html", None, Some(18), &[18]),
    ("li:has(+ li.c)", "container", None, None, &[]),
    (":is(span, a)[lang]", "doc", None, Some(13), &[13]),
    (":is(span, a)[lang]", "html", None, Some(13), &[13]),
    (":is(span, a)[lang]", "container", None, None, &[]),
    ("input:checked", "doc", None, Some(27), &[27]),
    ("input:checked", "html", None, Some(27), &[27]),
    ("input:checked", "container", None, None, &[]),
    ("option:selected", "doc", None, Some(31), &[31]),
    ("option:selected", "html", None, Some(31), &[31]),
    ("option:selected", "container", None, None, &[]),
    (":checkbox", "doc", None, Some(27), &[27]),
    (":checkbox", "html", None, Some(27), &[27]),
    (":checkbox", "container", None, None, &[]),
    (":root", "doc", None, Some(2), &[2]),
    (":root", "html", None, None, &[]),
    (":root", "container", None, None, &[]),
    (":scope", "doc", None, None, &[]),
    (":scope", "html", None, None, &[]),
    (":scope", "container", None, None, &[]),
    (":scope > body", "doc", None, None, &[]),
    (":scope > body", "html", None, Some(6), &[6]),
    (":scope > body", "container", None, None, &[]),
    ("> body", "doc", None, None, &[]),
    ("> body", "html", None, Some(6), &[6]),
    ("> body", "container", None, None, &[]),
    ("+ x", "doc", None, None, &[]),
    ("+ x", "html", None, None, &[]),
    ("+ x", "container", None, None, &[]),
    ("body > :first-child", "doc", None, Some(7), &[7]),
    ("body > :first-child", "html", None, Some(7), &[7]),
    ("body > :first-child", "container", None, None, &[]),
    ("a:link", "doc", None, Some(15), &[15]),
    ("a:link", "html", None, Some(15), &[15]),
    ("a:link", "container", None, None, &[]),
    ("a:any-link", "doc", None, Some(15), &[15]),
    ("a:any-link", "html", None, Some(15), &[15]),
    ("a:any-link", "container", None, None, &[]),
    (":header", "doc", None, None, &[]),
    (":header", "html", None, None, &[]),
    (":header", "container", None, None, &[]),
    ("li:nth-child(-n+2)", "doc", None, Some(18), &[18, 20]),
    ("li:nth-child(-n+2)", "html", None, Some(18), &[18, 20]),
    ("li:nth-child(-n+2)", "container", None, None, &[]),
    (
        "li:nth-child(2n+)",
        "doc",
        Some("n-th rule couldn't be parsed ('2n+')"),
        None,
        &[],
    ),
    (
        "li:nth-child(2n+)",
        "html",
        Some("n-th rule couldn't be parsed ('2n+')"),
        None,
        &[],
    ),
    (
        "li:nth-child(2n+)",
        "container",
        Some("n-th rule couldn't be parsed ('2n+')"),
        None,
        &[],
    ),
    (":foo", "doc", Some("Unknown pseudo-class :foo"), None, &[]),
    (":foo", "html", Some("Unknown pseudo-class :foo"), None, &[]),
    (
        ":foo",
        "container",
        Some("Unknown pseudo-class :foo"),
        None,
        &[],
    ),
    (
        "::before",
        "doc",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "::before",
        "html",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "::before",
        "container",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "a::after",
        "doc",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "a::after",
        "html",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "a::after",
        "container",
        Some("Pseudo-elements are not supported by css-select"),
        None,
        &[],
    ),
    (
        "[x",
        "doc",
        Some("Attribute selector didn't terminate"),
        None,
        &[],
    ),
    (
        "[x",
        "html",
        Some("Attribute selector didn't terminate"),
        None,
        &[],
    ),
    (
        "[x",
        "container",
        Some("Attribute selector didn't terminate"),
        None,
        &[],
    ),
    (
        ":is(a",
        "doc",
        Some("Missing closing parenthesis in :is (:is(a)"),
        None,
        &[],
    ),
    (
        ":is(a",
        "html",
        Some("Missing closing parenthesis in :is (:is(a)"),
        None,
        &[],
    ),
    (
        ":is(a",
        "container",
        Some("Missing closing parenthesis in :is (:is(a)"),
        None,
        &[],
    ),
    ("a,", "doc", Some("Empty sub-selector"), None, &[]),
    ("a,", "html", Some("Empty sub-selector"), None, &[]),
    ("a,", "container", Some("Empty sub-selector"), None, &[]),
    (
        "|a",
        "doc",
        Some("Namespaced tag names are not yet supported by css-select"),
        None,
        &[],
    ),
    (
        "|a",
        "html",
        Some("Namespaced tag names are not yet supported by css-select"),
        None,
        &[],
    ),
    (
        "|a",
        "container",
        Some("Namespaced tag names are not yet supported by css-select"),
        None,
        &[],
    ),
    (
        "[a|b]",
        "doc",
        Some("Namespaced attributes are not yet supported by css-select"),
        None,
        &[],
    ),
    (
        "[a|b]",
        "html",
        Some("Namespaced attributes are not yet supported by css-select"),
        None,
        &[],
    ),
    (
        "[a|b]",
        "container",
        Some("Namespaced attributes are not yet supported by css-select"),
        None,
        &[],
    ),
    (":hover", "doc", None, None, &[]),
    (":hover", "html", None, None, &[]),
    (":hover", "container", None, None, &[]),
    (
        "p:first-child(1)",
        "doc",
        Some("Pseudo-class :first-child doesn't have any arguments"),
        None,
        &[],
    ),
    (
        "p:first-child(1)",
        "html",
        Some("Pseudo-class :first-child doesn't have any arguments"),
        None,
        &[],
    ),
    (
        "p:first-child(1)",
        "container",
        Some("Pseudo-class :first-child doesn't have any arguments"),
        None,
        &[],
    ),
    (
        ":nth-child",
        "doc",
        Some("Cannot read properties of null (reading 'trim')"),
        None,
        &[],
    ),
    (
        ":nth-child",
        "html",
        Some("Cannot read properties of null (reading 'trim')"),
        None,
        &[],
    ),
    (
        ":nth-child",
        "container",
        Some("Cannot read properties of null (reading 'trim')"),
        None,
        &[],
    ),
    ("div p", "doc", None, Some(8), &[8, 12, 35]),
    ("div p", "html", None, Some(8), &[8, 12, 35]),
    ("div p", "container", None, Some(35), &[35]),
    ("section p", "doc", None, Some(35), &[35]),
    ("section p", "html", None, Some(35), &[35]),
    ("section p", "container", None, None, &[]),
    (".in p", "doc", None, Some(35), &[35]),
    (".in p", "html", None, Some(35), &[35]),
    (".in p", "container", None, Some(35), &[35]),
    ("html body", "doc", None, Some(6), &[6]),
    ("html body", "html", None, Some(6), &[6]),
    ("html body", "container", None, None, &[]),
    ("html > body", "doc", None, Some(6), &[6]),
    ("html > body", "html", None, Some(6), &[6]),
    ("html > body", "container", None, None, &[]),
    (":root p", "doc", None, Some(8), &[8, 12, 35]),
    (":root p", "html", None, Some(8), &[8, 12, 35]),
    (":root p", "container", None, None, &[]),
    (":is(p p)", "doc", None, None, &[]),
    (":is(p p)", "html", None, None, &[]),
    (":is(p p)", "container", None, None, &[]),
    ("li:not(li li)", "doc", None, Some(18), &[18, 20, 22, 25]),
    ("li:not(li li)", "html", None, Some(18), &[18, 20, 22, 25]),
    ("li:not(li li)", "container", None, None, &[]),
    ("b:not(div b)", "doc", None, None, &[]),
    ("b:not(div b)", "html", None, None, &[]),
    ("b:not(div b)", "container", None, None, &[]),
    ("div:has(div p)", "doc", None, Some(7), &[7, 34]),
    ("div:has(div p)", "html", None, Some(7), &[7, 34]),
    ("div:has(div p)", "container", None, Some(34), &[34]),
    ("p:not(.foo)", "doc", None, Some(12), &[12, 35]),
    ("p:not(.foo)", "html", None, Some(12), &[12, 35]),
    ("p:not(.foo)", "container", None, Some(35), &[35]),
    ("[lang=en]", "doc", None, Some(2), &[2, 13]),
    ("[lang=en]", "html", None, Some(13), &[13]),
    ("[lang=en]", "container", None, None, &[]),
    ("[LANG=En]", "doc", None, Some(2), &[2, 13]),
    ("[LANG=En]", "html", None, Some(13), &[13]),
    ("[LANG=En]", "container", None, None, &[]),
    ("[title=A I]", "doc", None, None, &[]),
    ("[title=A I]", "html", None, None, &[]),
    ("[title=A I]", "container", None, None, &[]),
];

#[test]
fn golden() {
    let d = build();
    // id контекстов: html=2 container=33
    assert_eq!(d.name(2), Some("html"));
    assert_eq!(d.name(33), Some("section"));
    let mut failures = Vec::new();
    for &(selector, context, error, one, all) in CASES {
        let ctx = match context {
            "doc" => DOCUMENT,
            "html" => 2,
            _ => 33,
        };
        let got = match select::compile(&d, selector, ctx) {
            Err(e) => (Some(e), None, Vec::new()),
            Ok(q) => (
                None,
                select::select_one(&d, &q, ctx),
                select::select_all(&d, &q, ctx),
            ),
        };
        let want = (error.map(str::to_owned), one, all.to_vec());
        if got != want {
            failures.push(format!(
                "{selector:?} @ {context}: got {got:?}, want {want:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
