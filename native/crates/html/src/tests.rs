//! Golden-кейсы: ожидаемый вывод получен из htmlparser2 8.0.2 + dom-serializer 2.0.0
//! (`node oracle/html.mjs golden <inputs.json>`).

use dom::{DOCUMENT, Dom, ElementKind, NodeData, NodeId};

use crate::{parse_document, render};

fn roundtrip(src: &str) -> String {
    render(&parse_document(src), DOCUMENT)
}

const GOLDEN: &[(&str, &str)] = &[
    ("<p>a<p>b<div>c</div>", "<p>a</p><p>b</p><div>c</div>"),
    (
        "<ul><li>a<li>b</ul><dl><dt>x<dd>y<dt>z</dl>",
        "<ul><li>a</li><li>b</li></ul><dl><dt>x</dt><dd>y</dd><dt>z</dt></dl>",
    ),
    (
        "<table><tr><td>1<td>2<tr><th>3<td>4</table>",
        "<table><tr><td>1</td><td>2</td></tr><tr><th>3</th><td>4</td></tr></table>",
    ),
    (
        "<select><option>a<option>b<optgroup><option>c</select><input><button>",
        "<select><option>a</option><option>b</option><optgroup><option>c</option></optgroup></select><input><button></button>",
    ),
    (
        "<p>x<h2>y</h2><p>z<section>s</section>",
        "<p>x</p><h2>y</h2><p>z</p><section>s</section>",
    ),
    (
        "<ruby>a<rt>b<rp>c<rt>d</ruby>",
        "<ruby>a<rt>b</rt><rp>c</rp><rt>d</rt></ruby>",
    ),
    (
        "<tbody><tr><td>x<tfoot><td>y",
        "<tbody><tr><td>x<tfoot><td>y</td></tfoot></td></tr></tbody>",
    ),
    (
        "<head><link rel=x><script>s</script><body>b",
        "<head><link rel=\"x\"><script>s</script></head><body>b</body>",
    ),
    ("a</p>b", "a<p></p>b"),
    ("a</br>b</BR >c</img>", "a<br>b<br>c"),
    (
        "<svg><path d=\"M0\"/><g><rect/></g><circle></circle></svg><p/>x",
        "<svg><path d=\"M0\"/><g><rect/></g><circle/></svg><p>x</p>",
    ),
    (
        "<svg><foreignobject><div/>t</foreignobject><clippath clippathunits=a/></svg>",
        "<svg><foreignObject><div>t</div></foreignObject><clipPath clipPathUnits=\"a/\"/></svg>",
    ),
    (
        "<SVG VIEWBOX=\"0 0 2 2\"><FEGAUSSIANBLUR STDDEVIATION=2 empty></SVG>",
        "<svg viewBox=\"0 0 2 2\"><feGaussianBlur stdDeviation=\"2\" empty=\"\"/></svg>",
    ),
    (
        "<math><mi/><mtext><b/>x</mtext><annotation-xml/><mo>y</mo></math>",
        "<math><mi><mtext><b>x</b></mtext><annotation-xml><mo>y</mo></annotation-xml></mi></math>",
    ),
    (
        "<svg><title/><desc>d</desc><g/></svg>",
        "<svg><title><desc>d</desc><g></g></title></svg>",
    ),
    (
        "<svg><title></title></svg><div/>",
        "<svg><title/></svg><div></div>",
    ),
    ("<svg></foreignObject><g/></svg>", "<svg><g/></svg>"),
    (
        "<script>if(a<b&&c>d)x=\"</scr\"+\"ipt>\"</script>",
        "<script>if(a<b&&c>d)x=\"</scr\"+\"ipt>\"</script>",
    ),
    (
        "<script>a</SCRIPT\t>b<script>c</script/x>d",
        "<script>a</script>b<script>c</script/x>d</script>",
    ),
    ("<script/>x<b>y</b></script>", "<script>x<b>y</b></script>"),
    (
        "<style>a>b{c:d}</style><STYLE>e</style >",
        "<style>a>b{c:d}</style><style>e</style>",
    ),
    (
        "<title>a<b>&amp;</b></TITLE>t",
        "<title>a<b>&amp;</b></title>t",
    ),
    (
        "<textarea><b>x</b></textarea>",
        "<textarea><b>x</b></textarea>",
    ),
    (
        "<noscript><link rel=stylesheet href=a.css></noscript>",
        "<noscript><link rel=\"stylesheet\" href=\"a.css\"></noscript>",
    ),
    (
        "<scriptx>a</scriptx><stylex>b</stylex><titlex>c</titlex>",
        "<scriptx>a</scriptx><stylex>b</stylex><titlex>c</titlex>",
    ),
    (
        "<!-- c --><!----><!--><!---><!-- a -- b --->",
        "<!-- c --><!----><!----><!----><!-- a -- b --->",
    ),
    (
        "<!x><!><!-y><![CDATA[ x<y ]]><![cdata[z]]>",
        "<!x><!><!-y><!--[CDATA[ x<y ]]--><![cdata[z]]>",
    ),
    (
        "<!DOCTYPE html><!doctype HTML PUBLIC \"x\"><!DocType\tfoo/bar>",
        "<!DOCTYPE html><!doctype HTML PUBLIC \"x\"><!DocType\tfoo/bar>",
    ),
    (
        "<?xml version=\"1.0\"?><?PHP echo 1 ?><?>",
        "<?xml version=\"1.0\"?><?PHP echo 1 ?><?>",
    ),
    ("</><//x></ x></1>", "</><!--/x--><!--1-->"),
    ("a < b > c <1> <<a", "a < b > c <1> <"),
    (
        "<a href=x/><a b/c><a =x><a b==c>",
        "<a href=\"x/\"><a b c><a =x><a b=\"=c\"></a></a></a></a>",
    ),
    (
        "<a b=\"c\"d='e' f=g\"h>",
        "<a b=\"c\" d=\"e\" f=\"g&quot;h\"></a>",
    ),
    (
        "<a id=1 ID=2 Id=3 2=two 1=one b=bee 0=zero 01=lead>",
        "<a 0=\"zero\" 1=\"one\" 2=\"two\" id=\"1\" b=\"bee\" 01=\"lead\"></a>",
    ),
    (
        "<a __proto__=x constructor=y toString>",
        "<a constructor=\"y\" tostring></a>",
    ),
    (
        "<a title='say \"hi\"' alt=\"it's\" data-x=\"&quot;&amp;\">",
        "<a title=\"say &quot;hi&quot;\" alt=\"it's\" data-x=\"&quot;&amp;\"></a>",
    ),
    (
        "<div ClAsS=Q><SPAN>x</sPaN></DiV>",
        "<div class=\"Q\"><span>x</span></div>",
    ),
    (
        "<br/><br></br><img src=x><hr/><input/>",
        "<br><br><br><img src=\"x\"><hr><input>",
    ),
    (
        "<p>unclosed<b>bold<i>italic",
        "<p>unclosed<b>bold<i>italic</i></b></p>",
    ),
    ("text<!--unterminated", "text<!--unterminated-->"),
    ("<![CDATA[unterminated", "<!--[CDATA[unterminated]]-->"),
    ("<div class=\"unterminated", ""),
    ("x</a  ", "x "),
    ("<a/", "/"),
    ("<s", "s"),
    (
        "<sc>a</sc><st>b</st><t>c</t>",
        "<sc>a</sc><st>b</st><t>c</t>",
    ),
    ("a\r\nb\u{0}c привет😀", "a\r\nb\u{0}c привет😀"),
    ("<aé b=1>x</aé><ÿ>y", "<aé b=\"1\">x</aé><ÿ>y"),
    (
        "<span>\u{1c}\u{f}script></span><script>\u{1c}\u{f}script>x</script>",
        "<span>\u{1c}\u{f}script></span><script></script>x",
    ),
];

#[test]
fn golden_outputs_match_js() {
    let mut failures = Vec::new();
    for (i, (input, expected)) in GOLDEN.iter().enumerate() {
        let actual = roundtrip(input);
        if actual != *expected {
            failures.push(format!(
                "#{i} {input:?}\n  expected {expected:?}\n  actual   {actual:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn element(dom: &Dom, id: NodeId) -> &dom::Element {
    dom.element(id).expect("element")
}

#[test]
fn text_runs_merge_into_one_node() {
    // `<`, не открывающий тег, разбивает текстовые события токенизатора.
    let dom = parse_document("a < b <1> c</>d");
    let kids = dom.children(DOCUMENT);
    assert_eq!(kids.len(), 1);
    assert_eq!(
        dom.node(kids[0]).data,
        NodeData::Text("a < b <1> c</>d".into())
    );
}

#[test]
fn element_kinds_and_attribute_order() {
    let dom = parse_document("<SCRIPT>x</SCRIPT><style></style><a 2=x b 1=y __proto__=p B=dup>");
    let kids = dom.children(DOCUMENT);
    assert_eq!(element(&dom, kids[0]).kind, ElementKind::Script);
    assert_eq!(element(&dom, kids[0]).name, "script");
    assert_eq!(element(&dom, kids[1]).kind, ElementKind::Style);
    let attrs: Vec<(&str, &str)> = element(&dom, kids[2]).attrs.iter().collect();
    assert_eq!(attrs, [("1", "y"), ("2", "x"), ("b", "")]);
}

#[test]
fn directives_and_comments() {
    let dom = parse_document("<!DocType html><?Xml/x?><!--a--><!--b--><![CDATA[c]]>");
    let data: Vec<&NodeData> = dom
        .children(DOCUMENT)
        .iter()
        .map(|&id| &dom.node(id).data)
        .collect();
    assert_eq!(
        data,
        [
            &NodeData::Directive {
                name: "!doctype".into(),
                data: "!DocType html".into()
            },
            &NodeData::Directive {
                name: "?xml".into(),
                data: "?Xml/x?".into()
            },
            &NodeData::Comment("a".into()),
            &NodeData::Comment("b".into()),
            &NodeData::Comment("[CDATA[c]]".into()),
        ]
    );
}

#[test]
fn render_subtree_starts_in_html_mode() {
    let dom = parse_document("<svg><foreignobject><g/></foreignobject></svg>");
    let svg = dom.children(DOCUMENT)[0];
    let foreign = dom.children(svg)[0];
    // Из документа: foreign-режим, отображённое имя, integration point для <g>.
    assert_eq!(
        render(&dom, DOCUMENT),
        "<svg><foreignObject><g></g></foreignObject></svg>"
    );
    // Рендер одного ребёнка: html-режим, имя как распарсено. (JS даёт это на свежем
    // дереве; после рендера документа выведет `foreignObject`, т.к. dom-serializer
    // переименовывает на месте. critters рендерит документ один раз, в конце.)
    assert_eq!(
        render(&dom, foreign),
        "<foreignobject><g></g></foreignobject>"
    );
}

#[test]
fn deep_nesting_does_not_overflow() {
    let src = "<div>".repeat(200_000);
    let dom = parse_document(&src);
    let out = render(&dom, DOCUMENT);
    assert_eq!(out.len(), src.len() * 2 + 200_000);
}
