//! Golden-кейсы; ожидания получены реальными critters 0.0.23 + postcss
//! 8.5.8 через `node oracle/css.mjs golden 15`.

use super::*;

enum Expect {
    Css(&'static str),
    Removed,
    Skipped,
    Throw,
}

fn fnv(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

fn predicate(seed: i64, sel: &str) -> Result<bool, String> {
    match seed {
        -1 => Ok(true),
        -2 => Ok(false),
        _ => match (fnv(sel) ^ (seed as u32)) % 7 {
            0 => Err(format!("boom:{sel}")),
            1..=3 => Ok(false),
            _ => Ok(true),
        },
    }
}

#[rustfmt::skip]
const GOLDEN: &[(&str, i64, Expect, &str, &[&str])] = &[
    (".a , .b { color : red ; }", -1, Expect::Css(".a,.b{color :red }"), "", &[".a", ".b"]),
    ("a{color:red;/* c */}", -1, Expect::Css("a{color:red}"), "", &["a"]),
    ("a{b:1;c:\"</style>\";}", -1, Expect::Css("a{b:1}"), "", &["a"]),
    ("a{b:c;;}", -1, Expect::Css("a{b:c;}"), "", &["a"]),
    ("a{b:c};;b{d:e};", -1, Expect::Css("a{b:c};;b{d:e};"), "", &["a", "b"]),
    ("a{*zoom:1;_height:2px}", -1, Expect::Css("a{*zoom:1;_height:2px}"), "", &["a"]),
    ("a{$&x:1;y$$:2;$`z :3}", -1, Expect::Css("a{$&x:x:1;y$:2;z :3}"), "", &["a"]),
    ("@import \"a\";a{b:c}", -2, Expect::Css("@import \"a\""), "", &["a"]),
    ("@import \"a\";a{b:c}", -1, Expect::Css("@import \"a\";a{b:c}"), "", &["a"]),
    ("a{b:c}@foo bar /* c */", -1, Expect::Css("a{b:c}@foo bar/* c */"), "", &["a"]),
    ("@media (x){a{b:c}}", -2, Expect::Removed, "", &["a"]),
    ("@media (x) {a{b:c}}@media  {}", -1, Expect::Css("@media (x){a{b:c}}"), "", &["a"]),
    ("@keyframes fadeIn{from{opacity:0}}a{animation:fadeIn 1s}", -1, Expect::Css("@keyframes fadeIn{from{opacity:0}}a{animation:fadeIn 1s}"), "", &["a"]),
    ("@keyframes fadeIn{from{opacity:0}}a{animation:fadeIn 1s}", -2, Expect::Removed, "", &["a"]),
    ("@-webkit-keyframes x{from{a:b}}", -2, Expect::Css("@-webkit-keyframes x{from{a:b}}"), "", &[]),
    ("@KEYFRAMES Foo{from{a:b}to{c:d}}", 1, Expect::Css("@KEYFRAMES Foo{from{a:b}to{c:d}}"), "", &["from", "to"]),
    ("@font-face{font-family:x;src:url(a.woff2)}a{b:c}", -1, Expect::Css("a{b:c}"), "", &["a"]),
    ("@font-face;", -1, Expect::Throw, "", &[]),
    ("/*critters:include*/.x{a:b}", -2, Expect::Css(".x{a:b}"), "", &[]),
    ("/*critters:exclude*/.x{a:b}", -1, Expect::Removed, "", &[]),
    ("/*critters:include start*/.x{a:b}.y{c:d}/*critters:include end*/.z{e:f}", -2, Expect::Css(".x{a:b}.y{c:d}"), "", &[".z"]),
    ("/*! critters:include */.x{a:b}/* critters:include */.y{c:d}", -2, Expect::Css(".y{c:d}"), "", &[".x"]),
    ("a,b\\,c,d{x:y}", -1, Expect::Css("a,b\\,c,d{x:y}"), "", &["a", "b\\,c", "d"]),
    ("/*critters:include*/a\\ ,b{x:y}", -1, Expect::Css("a\\,b{x:y}"), "", &[]),
    ("/*critters:include*/q{p a ~ b,*a::before, ,.p{x:y}}", -1, Expect::Css("q{p a ~ b,*a::before,.p{x:y}}"), "", &["q"]),
    (":is(.a, :focus) a:not(:hover)::before,::after,:root{x:y}", 5, Expect::Css("::after,:root{x:y}"), "1 rules skipped due to selector errors:\n  :is(.a) a -> boom::is(.a) a", &[":is(.a) a"]),
    ("a:not( ),.x\\:hover,.y::-webkit-scrollbar,:is(,.a),a:has(> b){x:y}", -1, Expect::Css("a:not( ),.x\\:hover,.y::-webkit-scrollbar,:is(,.a),a:has(> b){x:y}"), "", &["a", ".x\\:hover", ".y", ":is(.a)", "a:has(> b)"]),
    ("a{b:c!important;d:e ! important;f:g !IMPORTANT;h:i/**/!important }", -1, Expect::Css("a{b:c!important;d:e ! important;f:g !IMPORTANT;h:i/**/!important}"), "", &["a"]),
    ("a{--x:{a:b;c:d};--y: 1px ;}", -1, Expect::Css("a{--x:{a:b;c:d};--y:1px }"), "", &["a"]),
    ("a{background:url(data:image/png;base64,xx==);c:url( \"a b\" )}", -1, Expect::Css("a{background:url(data:image/png;base64,xx==);c:url( \"a b\" )}"), "", &["a"]),
    ("a{b:c}}", -1, Expect::Throw, "", &[]),
    ("a{", -1, Expect::Throw, "", &[]),
    ("\"x", -1, Expect::Throw, "", &[]),
    ("/* x", -1, Expect::Throw, "", &[]),
    ("a{b}", -1, Expect::Throw, "", &[]),
    ("a{color:red background:blue}", -1, Expect::Throw, "", &[]),
    ("a{filter:progid:DX(a:b)}", -1, Expect::Css("a{filter:progid:DX(a:b)}"), "", &["a"]),
    ("\u{feff}a{b:c}", -1, Expect::Css("a{b:c}"), "", &["a"]),
    ("", -1, Expect::Skipped, "", &[]),
    ("  \n", -1, Expect::Removed, "", &[]),
    (".\\😀x,.\\😀 .y{a:b}", -1, Expect::Css(".\\😀x,.\\😀 .y{a:b}"), "", &[".\\😀x", ".\\😀 .y"]),
    ("a{b:url\\😀(x y)}", -1, Expect::Css("a{b:url\\😀(x y)}"), "", &["a"]),
    ("a{color:red;b{c:d}}", 3, Expect::Removed, "1 rules skipped due to selector errors:\n  b -> boom:b", &["b", "a"]),
    ("@media x{a{b:c}/*critters:include*/}b{c:d}", -2, Expect::Css("@media x{}b{c:d}"), "", &["a"]),
    ("a{b:c;/**/}a{;b:c}", -1, Expect::Css("a{b:c}a{;b:c}"), "", &["a", "a"]),
    ("a/**/,b /* c */{c:d}", -1, Expect::Css("a/**/,b{c:d}"), "", &["a/**/", "b"]),
    ("@layer a, b;@layer x{a{b:c}}", -1, Expect::Css("@layer a, b;@layer x{a{b:c}}"), "", &["a"]),
    ("a{b:c}x:y", -1, Expect::Css("a{b:c}x:y"), "", &["a"]),
    ("@ x{}", -1, Expect::Throw, "", &[]),
    ("a{b:c;d:\"</style>\"}", -1, Expect::Css("a{b:c;}"), "", &["a"]),
    ("@media x  {a{b:c}}@supports (a:b)\n\t{c{d:e}}", -1, Expect::Css("@media x {a{b:c}}@supports (a:b)\n{c{d:e}}"), "", &["a", "c"]),
    ("a{b:url (x;y)}", -1, Expect::Css("a{b:url (x;y)}"), "", &["a"]),
    ("a{b:url(x\\\\)y)}", -1, Expect::Css("a{b:url(x\\\\)y)}"), "", &["a"]),
    ("a{b:\\😀:c}", -1, Expect::Throw, "", &[]),
    ("a{b:c\\😀}", -1, Expect::Css("a{b:c\\😀}"), "", &["a"]),
    ("\\😀{b:c}", -1, Expect::Css("\\😀{b:c}"), "", &["\\😀"]),
    ("a{b:url\\😀 (x)}", -1, Expect::Css("a{b:url\\😀 (x)}"), "", &["a"]),
    ("😀{x:\"</style>\";}", -1, Expect::Css("😀}"), "", &["😀"]),
    ("a{b:red ! important /**/;}", -1, Expect::Css("a{b:red ! important /**/}"), "", &["a"]),
    ("a{b:red !/**/important}", -1, Expect::Css("a{b:red !/**/important}"), "", &["a"]),
    ("a{b:! important}", -1, Expect::Css("a{b:! important}"), "", &["a"]),
    ("a{b:x !ie important}", -1, Expect::Css("a{b:x !ie important}"), "", &["a"]),
    ("a{b:red!important!important}", -1, Expect::Css("a{b:red!important!important}"), "", &["a"]),
    ("a{filter:progid:a:b}", -1, Expect::Throw, "", &[]),
    ("a{--x:{a:{b:c}}}", -1, Expect::Css("a{--x:{a:{b:c}}}"), "", &["a"]),
    ("a{--x: ;}", -1, Expect::Css("a{--x: }"), "", &["a"]),
    ("a{--x:/*c*/}", -1, Expect::Css("a{--x:/*c*/}"), "", &["a"]),
    ("a{b:/*c*/}", -1, Expect::Css("a{b:}"), "", &["a"]),
    ("a{b: /*c*/ x /*d*/ }", -1, Expect::Css("a{b: /*c*/x}"), "", &["a"]),
    ("a{b:x/*c*/,/*d*/y}", -1, Expect::Css("a{b:x/*c*/,/*d*/y}"), "", &["a"]),
    ("a{\"s\"x:y}", -1, Expect::Css("a{\"s\"x:y}"), "", &["a"]),
    ("a{(a)b:c}", -1, Expect::Css("a{(a)b:c}"), "", &["a"]),
    ("a{:a}", -1, Expect::Css("a{:a}"), "", &["a"]),
    ("a{_:x}", -1, Expect::Css("a{_:x}"), "", &["a"]),
    ("a{*:x}", -1, Expect::Css("a{*:x}"), "", &["a"]),
    ("@media{}a{b:c}", -1, Expect::Css("a{b:c}"), "", &["a"]),
    ("@media x;a{b:c}", -1, Expect::Css("@media x;a{b:c}"), "", &["a"]),
    ("a{@media x{b{c:d}}}", 2, Expect::Removed, "1 rules skipped due to selector errors:\n  a -> boom:a", &["b", "a"]),
    ("a{b{c:d}e:f}", -1, Expect::Css("a{b{c:d}e:f}"), "", &["b", "a"]),
    ("{a:b}", -1, Expect::Removed, "", &[]),
    ("/*critters:include*/{a:b}", -1, Expect::Css("{a:b}"), "", &[]),
    (".a\\\\,.b{c:d}", 3, Expect::Removed, "1 rules skipped due to selector errors:\n  .b -> boom:.b", &[".a\\\\", ".b"]),
    (".a\\\\ ,.b{c:d}", 7, Expect::Css(".a\\\\,.b{c:d}"), "", &[".a\\\\", ".b"]),
    ("\"a,b\",c{d:e}", 9, Expect::Removed, "", &["\"a,b\"", "c"]),
    ("a(b,c),d{e:f}", 11, Expect::Css("a(b,c){e:f}"), "", &["a(b,c)", "d"]),
    ("a,b{c:d}", 13, Expect::Removed, "", &["a", "b"]),
    ("a, b , c{d:e}", -1, Expect::Css("a,b,c{d:e}"), "", &["a", "b", "c"]),
    ("@font-face{font-family:x}@media x{@font-face;}", -1, Expect::Throw, "", &[]),
    ("/*critters:exclude*/a{@font-face;}", -1, Expect::Throw, "", &[]),
    ("a{b:c}@media x{@font-face{}}", -1, Expect::Css("a{b:c}@media x{}"), "", &["a"]),
    ("@media x{/*critters:include*/}a{b:c}", -2, Expect::Css("@media x{}"), "", &["a"]),
    ("@keyframes a{from{b:c}}@keyframes  a {from{b:c}}x{animation:a}", -1, Expect::Css("@keyframes a{from{b:c}}@keyframes  a{from{b:c}}x{animation:a}"), "", &["x"]),
    ("@keyframes a/**/{from{b:c}}x{animation-name:a}", -1, Expect::Css("@keyframes a/**/{from{b:c}}x{animation-name:a}"), "", &["x"]),
    ("x{animation:\na\t1s}@keyframes a{0%{b:c}}", -1, Expect::Css("x{animation:a\t1s}@keyframes a{0%{b:c}}"), "", &["x"]),
    ("x{Animation:a}@keyframes a{0%{b:c}}", -1, Expect::Css("x{Animation:a}"), "", &["x"]),
    ("a{b:c}/* x */", -1, Expect::Css("a{b:c}"), "", &["a"]),
    ("a{b:c} ; ", -1, Expect::Css("a{b:c};"), "", &["a"]),
    ("a{b:c};b{c:d}", -2, Expect::Removed, "", &["a", "b"]),
    ("@charset \"x\";", -1, Expect::Css("@charset \"x\";"), "", &[]),
    ("@foo", -1, Expect::Css("@foo"), "", &[]),
    ("@foo ", -1, Expect::Css("@foo"), "", &[]),
    ("@foo /*x*/", -1, Expect::Css("@foo /*x*/"), "", &[]),
    (":root{--a:1}html{b:c}body{d:e}::before{f:g}:after{h:i}", -2, Expect::Css(":root{--a:1}html{b:c}body{d:e}::before{f:g}:after{h:i}"), "", &[]),
    ("a::before:hover,b:not( ){c:d}", -2, Expect::Removed, "", &["a", "b"]),
    ("a{b:c;}\u{2028}b{c:d}", -1, Expect::Css("a{b:c}b{c:d}"), "", &["a", "b"]),
    ("a\u{a0}{b:c}", -1, Expect::Css("a{b:c}"), "", &["a"]),
    ("a{b\u{a0}:c}", -1, Expect::Css("a{b\u{a0}:c}"), "", &["a"]),
    ("a:is(b,c),d{e:f}", -1, Expect::Css("a:is(b,c),d{e:f}"), "", &["a:is(b,c)", "d"]),
    ("to{--y-z\u{c}: \n   ;}", -1, Expect::Css("to{--y-z\u{c}: \n   }"), "", &["to"]),
    ("x[y='a\\'b,c']{/*critters:exclude end*/--y-z:/**/red !IMPORTANT;;content : \\31 x;}@media screen{/*critters:exclude start*//*critters:include start*/.x\\ + a::before{transition:/**/calc(1px + 2px);color:/* only */;/*critters:includex*/}}@layer a, b;/*critters:include*/\\", 248, Expect::Throw, "", &[]),
    ("a::before{/*critters:includex*//*critters:exclude end*/@import\"c.css\";;content:/**/rgba(0,0,0,.5);}  a{b:c}}", -2, Expect::Throw, "", &[]),
    ("@keyframes spin\t{0%{opacity:0}\u{c}100%{opacity:1}}", 213, Expect::Removed, "", &[]),
    ("@font-face{a{}} \n\t", 450, Expect::Removed, "", &["a"]),
    ("/*c*/a:hover::before ,.p,  ,  :is(.a, :focus) + 'q,r',::after>.z\u{a0}{a\\:b : \"</style>\";}.a, ,.e\\\",\t,a:not(:hover) ,.😀>body.x,  ,  a:1{margin/**/:\"</style>\";}/*critters:include*/@supports not (x:y){--y-z : red/**/!important;*zoom:'a\\'b';}", 887, Expect::Css(".p,::after>.z}.e\\\"}@supports not (x:y){--y-z :red/**/!important;*zoom:'a\\'b'}"), "2 rules skipped due to selector errors:\n  a -> boom:a\n  a -> boom:a", &["a", ".p", ":is(.a) + 'q,r'", ">.z", ".a", ".e\\\"", "a", ".😀>body.x", "a:1"]),
    (".q\\\\:hover{margin : 'a\\'b';}a:not(\u{a0}){\r\n}.k:not(.l)\r\n{li:first-child{}display:important}a\u{a0}b\r\n{$`d/**/:red/**/!important;a\\ \n  {@media screen{/*critters:exclude start*/a:is(b,c)a\u{a0}b{/**/;}ul li{x;$var:/**/important }p{a$$b : !ie}}a /* c */\r\n{}$&a: \n\t\"</style>\" }}/* x", 694, Expect::Throw, "", &[]),
    (".f:focus-within{color:fn( a , b )}::-moz-selection, .w-1\\/2,a > b,,a/**/b{}\\", 461, Expect::Throw, "", &[]),
    ("a:not( ){filter: \n  url(a.png);-\\$: 1px,/**/2px;}*{/*! critters:include *//*critters:include*/}a{b}", 499, Expect::Throw, "", &[]),
    (":host\u{c}{\n\t--y-z  : 1px /**/ 2px;\n_height  :/**/  var(--x, 1px);  color\t:\r\n1px/**/2px \n\t}  a{b}", 30, Expect::Throw, "", &[]),
    ("@font-face{}", 840, Expect::Removed, "", &[]),
    ("/*critters:exclude start*/", 728, Expect::Removed, "", &[]),
    ("[a{/*c*/&,& .n,,*,,::slotted(span) + .😀{\u{c}}a\\:b/**/:red !important ;@media print\n{/* x */}}@charset 'utf-8';/*critters:exclude end*/", 575, Expect::Throw, "", &[]),
    ("/*critters:*/", -2, Expect::Removed, "", &[]),
    ("@font-face{}@media x{}", 768, Expect::Removed, "", &[]),
];

#[test]
fn golden() {
    for (i, (input, seed, expect, warn, calls)) in GOLDEN.iter().enumerate() {
        let mut seen: Vec<String> = Vec::new();
        let res = process_style(input, &mut |s| {
            seen.push(s.to_string());
            predicate(*seed, s)
        });
        assert_eq!(seen, *calls, "case {i} calls: {input:?}");
        match (expect, res) {
            (Expect::Throw, Err(e)) => assert_ne!(e.kind, ErrorKind::Unsupported, "case {i}"),
            (Expect::Throw, Ok(r)) => panic!("case {i}: expected throw, got {r:?}: {input:?}"),
            (_, Err(e)) => panic!("case {i}: unexpected error {e}: {input:?}"),
            (exp, Ok(r)) => {
                match exp {
                    Expect::Skipped => assert!(r.skipped, "case {i}"),
                    Expect::Removed => {
                        assert!(
                            !r.skipped && dom::js::trim(&r.css).is_empty(),
                            "case {i}: {r:?}"
                        )
                    }
                    Expect::Css(css) => assert_eq!(r.css, *css, "case {i}: {input:?}"),
                    Expect::Throw => unreachable!(),
                }
                let w = if r.failed_selectors.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{} rules skipped due to selector errors:\n  {}",
                        r.failed_selectors.len(),
                        r.failed_selectors.join("\n  ")
                    )
                };
                assert_eq!(w, *warn, "case {i} failed selectors: {input:?}");
            }
        }
    }
}

#[test]
fn source_map_annotation_is_unsupported() {
    let r = process_style("a{b:c}/*# sourceMappingURL=a.css.map */", &mut |_| Ok(true));
    assert_eq!(r.unwrap_err().kind, ErrorKind::Unsupported);
    // Пустую аннотацию postcss игнорирует.
    assert!(process_style("a{b:c}/*# sourceMappingURL= */", &mut |_| Ok(true)).is_ok());
}

#[test]
fn comma_and_transform() {
    assert_eq!(comma("a, b ,,c"), ["a", "b", "c"]);
    assert_eq!(comma("a,"), ["a", ""]);
    assert_eq!(comma("a, ,b"), ["a", "", "b"]);
    assert_eq!(
        comma("a\\,b,\"c,d\",e(f,g)"),
        ["a\\,b", "\"c,d\"", "e(f,g)"]
    );
    assert_eq!(transform_selector("a:hover::before"), "a");
    assert_eq!(transform_selector("a\\::hover"), "a\\:");
    assert_eq!(transform_selector(":is(.a, :focus)"), ":is(.a)");
    assert_eq!(transform_selector("a:not( ) b"), "a b");
    assert_eq!(transform_selector("a:nth-child(2n)"), "a:nth-child(2n)");
}
