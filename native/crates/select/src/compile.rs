//! Порт компиляции css-select 5.2.2 (compile.js, sort.js, general.js,
//! attributes.js, pseudo-selectors/*) в дерево матчеров, вычисляемое справа налево.
//!
//! Тождества `trueFunc` / `falseFunc` из boolbase моделируются [`M::True`] /
//! [`M::False`]: css-select ветвится по ним (пропускает компиляцию остатка
//! составного селектора, отбрасывает обёртки `:is`/`:not`, …), что влияет и на
//! результаты, и на то, бросает ли компиляция исключение.

use dom::{Dom, NodeId};

use crate::jsstr;
use crate::nth::{self, Nth};
use crate::parse::{self, AttributeAction, IgnoreCase, PseudoData, Token};

/// Скомпилированная функция селектора.
#[derive(Clone, Debug)]
pub(crate) enum M {
    True,
    False,
    Op(Box<Op>),
}

impl M {
    fn op(op: Op) -> M {
        M::Op(Box::new(op))
    }
}

/// Элемент, на который ссылается `:scope` (массив `context` в css-select, здесь
/// всегда длины ≤ 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Контекста нет: `:scope` ведёт себя как `:root`.
    None,
    Node(NodeId),
    /// `[PLACEHOLDER_ELEMENT]` из `:has`, перезаписывается проверяемым элементом.
    Slot(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SimplePseudo {
    Empty,
    FirstChild,
    LastChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    OnlyChild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NthKind {
    Child,
    LastChild,
    OfType,
    LastOfType,
}

/// Ключи `Object.prototype` только в нижнем регистре, достижимые через `attribs[name]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProtoMember {
    None,
    /// `constructor` (`Object`), перекрывается собственным атрибутом с таким именем.
    Constructor,
    /// `__proto__` (`Object.prototype`); в domhandler никогда не бывает собственным атрибутом.
    Proto,
}

#[derive(Clone, Debug)]
pub(crate) enum AttrTest {
    Exists,
    Equals(String),
    /// Значение в нижнем регистре; сравниваются длины значения в нижнем регистре.
    EqualsIc(String),
    Hyphen {
        len: usize,
        value: String,
    },
    HyphenIc {
        len: usize,
        value: String,
    },
    Element {
        value: String,
        ignore_case: bool,
        len: usize,
    },
    Start(String),
    StartIc {
        len: usize,
        value: String,
    },
    End(String),
    EndIc {
        len: usize,
        value: String,
    },
    Any(String),
    AnyIc {
        value: String,
        len: usize,
    },
    /// `[a!=""]`: `!!attr`.
    NotEmpty,
    Not(String),
    NotIc(String),
}

#[derive(Clone, Debug)]
pub(crate) enum Op {
    Tag {
        name: String,
        next: M,
    },
    Attr {
        name: String,
        /// Какой член `Object.prototype` подставляется для `attribs[name]` при отсутствии атрибута.
        proto: ProtoMember,
        test: AttrTest,
        next: M,
    },
    Descendant {
        /// Индекс в `isFalseCache` (только для селекторов верхнего уровня).
        cache: Option<usize>,
        next: M,
    },
    FlexibleDescendant {
        next: M,
    },
    Parent {
        next: M,
    },
    Child {
        next: M,
    },
    Sibling {
        next: M,
    },
    Adjacent {
        next: M,
    },
    Contains {
        text: String,
        next: M,
    },
    IContains {
        text: String,
        next: M,
    },
    Nth {
        kind: NthKind,
        check: Nth,
        next: M,
    },
    /// Результат `trueFunc` из nth-check: родитель должен быть элементом.
    HasElementParent {
        next: M,
    },
    Root {
        next: M,
    },
    Scope {
        scope: Scope,
        next: M,
    },
    Pseudo {
        kind: SimplePseudo,
        next: M,
    },
    Is {
        func: M,
        next: M,
    },
    Not {
        func: M,
        next: M,
    },
    /// `:has` без контекста (без обхода внутри) или с телом `trueFunc`.
    Has {
        inner: M,
        next: M,
    },
    /// Относительный `:has`: записывает элемент в `slot`, затем ищет.
    HasRelative {
        slot: usize,
        inner: M,
        test_next_siblings: bool,
        next: M,
    },
    Or(M, M),
}

const CASE_INSENSITIVE_ATTRIBUTES: [&str; 46] = [
    "accept",
    "accept-charset",
    "align",
    "alink",
    "axis",
    "bgcolor",
    "charset",
    "checked",
    "clear",
    "codetype",
    "color",
    "compact",
    "declare",
    "defer",
    "dir",
    "direction",
    "disabled",
    "enctype",
    "face",
    "frame",
    "hreflang",
    "http-equiv",
    "lang",
    "language",
    "link",
    "media",
    "method",
    "multiple",
    "nohref",
    "noresize",
    "noshade",
    "nowrap",
    "readonly",
    "rel",
    "rev",
    "rules",
    "scope",
    "scrolling",
    "selected",
    "shape",
    "target",
    "text",
    "type",
    "valign",
    "valuetype",
    "vlink",
];

const ALIASES: [(&str, &str); 20] = [
    ("any-link", ":is(a, area, link)[href]"),
    ("link", ":any-link:not(:visited)"),
    (
        "disabled",
        ":is(\n        :is(button, input, select, textarea, optgroup, option)[disabled],\n        optgroup[disabled] > option,\n        fieldset[disabled]:not(fieldset[disabled] legend:first-of-type *)\n    )",
    ),
    ("enabled", ":not(:disabled)"),
    (
        "checked",
        ":is(:is(input[type=radio], input[type=checkbox])[checked], option:selected)",
    ),
    ("required", ":is(input, select, textarea)[required]"),
    ("optional", ":is(input, select, textarea):not([required])"),
    (
        "selected",
        "option:is([selected], select:not([multiple]):not(:has(> option[selected])) > :first-of-type)",
    ),
    ("checkbox", "[type=checkbox]"),
    ("file", "[type=file]"),
    ("password", "[type=password]"),
    ("radio", "[type=radio]"),
    ("reset", "[type=reset]"),
    ("image", "[type=image]"),
    ("submit", "[type=submit]"),
    ("parent", ":not(:empty)"),
    ("header", ":is(h1, h2, h3, h4, h5, h6)"),
    ("button", ":is(button, input[type=button])"),
    ("input", ":is(input, textarea, select, button)"),
    ("text", "input:is(:not([type!='']), [type=text])"),
];

fn alias(name: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, selector)| *selector)
}

/// Токен составного селектора в процессе компиляции (`_flexibleDescendant` —
/// внутреннее добавление css-select).
#[derive(Clone, Debug)]
enum Rule {
    T(Token),
    Flexible,
}

/// sort.js `isTraversal`: всё, кроме universal, tag, attribute и pseudo.
fn sort_is_traversal(token: &Token) -> bool {
    !matches!(
        token,
        Token::Universal { .. }
            | Token::Tag { .. }
            | Token::Attribute { .. }
            | Token::Pseudo { .. }
    )
}

/// sort.js `getProcedure`.
fn procedure(token: &Token) -> i32 {
    match token {
        Token::Universal { .. } => 50,
        Token::Tag { .. } => 30,
        Token::Attribute {
            name,
            action,
            ignore_case,
            ..
        } => {
            let mut proc = match action {
                AttributeAction::Exists => 10,
                AttributeAction::Equals => 8,
                AttributeAction::Not => 7,
                AttributeAction::Start | AttributeAction::End => 6,
                AttributeAction::Any => 5,
                AttributeAction::Hyphen | AttributeAction::Element => 4,
            };
            if *action == AttributeAction::Equals && name == "id" {
                proc = 9;
            }
            if ignore_case.is_truthy() {
                proc >>= 1;
            }
            proc
        }
        Token::Pseudo { name, data } => match data {
            PseudoData::None => 3,
            PseudoData::Str(s) if s.is_empty() => 3,
            _ if name == "has" || name == "contains" => 0,
            PseudoData::Selectors(list) => {
                let min = list
                    .iter()
                    .map(|tokens| tokens.iter().map(procedure).min().unwrap_or(i32::MAX))
                    .min()
                    .unwrap_or(i32::MAX);
                min.max(0)
            }
            PseudoData::Str(_) => 2,
        },
        _ => -1,
    }
}

/// sort.js `sortByProcedure` (сортировка вставками; traversal-токены остаются на месте).
fn sort_by_procedure(arr: &mut [Token]) {
    let mut procs: Vec<i32> = arr.iter().map(procedure).collect();
    for i in 1..arr.len() {
        let proc_new = procs[i];
        if proc_new < 0 {
            continue;
        }
        let mut j = i as isize - 1;
        while j >= 0 && proc_new < procs[j as usize] {
            let ju = j as usize;
            arr.swap(ju + 1, ju);
            procs[ju + 1] = procs[ju];
            procs[ju] = proc_new;
            j -= 1;
        }
    }
}

fn includes_scope_pseudo(token: &Token) -> bool {
    match token {
        Token::Pseudo { name, data } => {
            name == "scope"
                || matches!(data, PseudoData::Selectors(list)
                    if list.iter().any(|tokens| tokens.iter().any(includes_scope_pseudo)))
        }
        _ => false,
    }
}

fn scope_token() -> Token {
    Token::Pseudo {
        name: "scope".to_owned(),
        data: PseudoData::None,
    }
}

pub(crate) struct Compiled {
    pub(crate) func: M,
    pub(crate) should_test_next_siblings: bool,
}

pub(crate) struct Compiler<'d> {
    dom: &'d Dom,
    pub(crate) slots: usize,
    pub(crate) caches: usize,
}

/// `options.cacheResults` после `copyOptions`: кэширует только верхний уровень.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Top,
    Nested,
}

impl<'d> Compiler<'d> {
    pub(crate) fn new(dom: &'d Dom) -> Self {
        Compiler {
            dom,
            slots: 0,
            caches: 0,
        }
    }

    /// `compileToken(token, defaultOptions, context)` для одиночного (не массива) контекста.
    pub(crate) fn compile_top(
        &mut self,
        token: Vec<Vec<Token>>,
        context: NodeId,
    ) -> Result<Compiled, String> {
        self.compile_token(token, Level::Top, Scope::Node(context), false)
    }

    /// `compileToken`. `scope` — это `finalContext` (`Scope::None`, если undefined);
    /// `is_array_context` — `Array.isArray(context)` аргумента.
    fn compile_token(
        &mut self,
        mut token: Vec<Vec<Token>>,
        level: Level,
        scope: Scope,
        is_array_context: bool,
    ) -> Result<Compiled, String> {
        for tokens in &mut token {
            sort_by_procedure(tokens);
        }

        let has_context = match scope {
            Scope::None => false,
            Scope::Slot(_) => true,
            Scope::Node(id) => {
                self.dom.is_element(id)
                    && self.dom.parent(id).is_some_and(|p| self.dom.is_element(p))
            }
        };
        let mut rule_lists: Vec<Vec<Rule>> = Vec::with_capacity(token.len());
        for tokens in token {
            let starts_with_traversal = tokens
                .first()
                .is_some_and(|t| sort_is_traversal(t) && *t != Token::Descendant);
            let mut rules: Vec<Rule> = Vec::with_capacity(tokens.len() + 2);
            if starts_with_traversal {
                rules.push(Rule::T(scope_token()));
            } else if has_context && !tokens.iter().any(includes_scope_pseudo) {
                rules.push(Rule::T(scope_token()));
                rules.push(Rule::T(Token::Descendant));
            }
            rules.extend(tokens.into_iter().map(Rule::T));
            rule_lists.push(rules);
        }

        let mut should_test_next_siblings = false;
        let mut compiled = Vec::with_capacity(rule_lists.len());
        for mut rules in rule_lists {
            if rules.len() >= 2 {
                let first_is_scope =
                    matches!(&rules[0], Rule::T(Token::Pseudo { name, .. }) if name == "scope");
                if first_is_scope {
                    match &rules[1] {
                        Rule::T(Token::Descendant) if is_array_context => {
                            rules[1] = Rule::Flexible;
                        }
                        Rule::T(Token::Adjacent | Token::Sibling) => {
                            should_test_next_siblings = true;
                        }
                        _ => {}
                    }
                }
            }
            compiled.push(self.compile_rules(rules, level, scope)?);
        }

        let func = compiled.into_iter().fold(M::False, reduce_rules);
        Ok(Compiled {
            func,
            should_test_next_siblings,
        })
    }

    fn compile_rules(&mut self, rules: Vec<Rule>, level: Level, scope: Scope) -> Result<M, String> {
        let mut previous = M::True;
        for rule in rules {
            if matches!(previous, M::False) {
                return Ok(M::False);
            }
            previous = self.compile_general(previous, rule, level, scope)?;
        }
        Ok(previous)
    }

    fn compile_general(
        &mut self,
        next: M,
        rule: Rule,
        level: Level,
        scope: Scope,
    ) -> Result<M, String> {
        let token = match rule {
            Rule::Flexible => return Ok(M::op(Op::FlexibleDescendant { next })),
            Rule::T(token) => token,
        };
        Ok(match token {
            Token::PseudoElement { .. } => {
                return Err("Pseudo-elements are not supported by css-select".to_owned());
            }
            Token::ColumnCombinator => {
                return Err("Column combinators are not yet supported by css-select".to_owned());
            }
            Token::Attribute {
                name,
                action,
                value,
                namespace,
                ignore_case,
            } => {
                if namespace.is_some() {
                    return Err(
                        "Namespaced attributes are not yet supported by css-select".to_owned()
                    );
                }
                let name = jsstr::lower(&name);
                compile_attribute(next, name, action, value, ignore_case)
            }
            Token::Pseudo { name, data } => {
                return self.compile_pseudo(next, name, data, scope);
            }
            Token::Tag { name, namespace } => {
                if namespace.is_some() {
                    return Err(
                        "Namespaced tag names are not yet supported by css-select".to_owned()
                    );
                }
                M::op(Op::Tag {
                    name: jsstr::lower(&name),
                    next,
                })
            }
            Token::Descendant => {
                let cache = (level == Level::Top).then(|| {
                    self.caches += 1;
                    self.caches - 1
                });
                M::op(Op::Descendant { cache, next })
            }
            Token::Parent => M::op(Op::Parent { next }),
            Token::Child => M::op(Op::Child { next }),
            Token::Sibling => M::op(Op::Sibling { next }),
            Token::Adjacent => M::op(Op::Adjacent { next }),
            Token::Universal { namespace } => {
                if namespace.as_deref().is_some_and(|ns| ns != "*") {
                    return Err(
                        "Namespaced universal selectors are not yet supported by css-select"
                            .to_owned(),
                    );
                }
                next
            }
        })
    }

    fn compile_pseudo(
        &mut self,
        next: M,
        name: String,
        data: PseudoData,
        scope: Scope,
    ) -> Result<M, String> {
        let data = match data {
            PseudoData::Selectors(list) => {
                return match name.as_str() {
                    "is" | "matches" | "where" => self.subselect_is(next, list, scope),
                    "not" => {
                        let func = self.compile_nested(list, scope)?;
                        Ok(match func {
                            M::False => next,
                            M::True => M::False,
                            func => M::op(Op::Not { func, next }),
                        })
                    }
                    "has" => self.subselect_has(next, list),
                    _ => {
                        let count: usize = list.iter().map(Vec::len).sum();
                        Err(format!(
                            "Unknown pseudo-class :{name}({})",
                            vec!["[object Object]"; count].join(",")
                        ))
                    }
                };
            }
            PseudoData::None => None,
            PseudoData::Str(s) => Some(s),
        };

        if let Some(selector) = alias(&name) {
            if data.is_some() {
                return Err(format!("Pseudo {name} doesn't have any arguments"));
            }
            let alias = parse::parse(selector).expect("aliases parse");
            return self.subselect_is(next, alias, scope);
        }

        match name.as_str() {
            "contains" => {
                let text = data.unwrap_or_else(|| "null".to_owned());
                return Ok(M::op(Op::Contains { text, next }));
            }
            "icontains" => {
                let Some(text) = data else {
                    return Err("Cannot read properties of null (reading 'toLowerCase')".to_owned());
                };
                return Ok(M::op(Op::IContains {
                    text: jsstr::lower(&text),
                    next,
                }));
            }
            "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                let Some(formula) = data else {
                    return Err("Cannot read properties of null (reading 'trim')".to_owned());
                };
                let kind = match name.as_str() {
                    "nth-child" => NthKind::Child,
                    "nth-last-child" => NthKind::LastChild,
                    "nth-of-type" => NthKind::OfType,
                    _ => NthKind::LastOfType,
                };
                return Ok(match nth::nth_check(&formula)? {
                    Nth::False => M::False,
                    Nth::True => M::op(Op::HasElementParent { next }),
                    check => M::op(Op::Nth { kind, check, next }),
                });
            }
            "root" => return Ok(M::op(Op::Root { next })),
            "scope" => {
                return Ok(match scope {
                    Scope::None => M::op(Op::Root { next }),
                    scope => M::op(Op::Scope { scope, next }),
                });
            }
            // В DomUtils нет isHovered / isVisited / isActive.
            "hover" | "visited" | "active" => return Ok(M::False),
            // `filters.constructor` — это `Object`, а `Object(next, …)` возвращает `next`.
            "constructor" => return Ok(next),
            "__proto__" => return Err("filters_js_1.filters[name] is not a function".to_owned()),
            _ => {}
        }

        let kind = match name.as_str() {
            "empty" => SimplePseudo::Empty,
            "first-child" => SimplePseudo::FirstChild,
            "last-child" => SimplePseudo::LastChild,
            "first-of-type" => SimplePseudo::FirstOfType,
            "last-of-type" => SimplePseudo::LastOfType,
            "only-of-type" => SimplePseudo::OnlyOfType,
            "only-child" => SimplePseudo::OnlyChild,
            _ => return Err(format!("Unknown pseudo-class :{name}")),
        };
        if data.is_some() {
            return Err(format!("Pseudo-class :{name} doesn't have any arguments"));
        }
        Ok(M::op(Op::Pseudo { kind, next }))
    }

    /// `compileToken(token, copyOptions(options), context)` из подселектора.
    fn compile_nested(&mut self, token: Vec<Vec<Token>>, scope: Scope) -> Result<M, String> {
        let is_array = scope != Scope::None;
        Ok(self
            .compile_token(token, Level::Nested, scope, is_array)?
            .func)
    }

    fn subselect_is(&mut self, next: M, token: Vec<Vec<Token>>, scope: Scope) -> Result<M, String> {
        let func = self.compile_nested(token, scope)?;
        Ok(match func {
            M::True => next,
            M::False => M::False,
            func => M::op(Op::Is { func, next }),
        })
    }

    fn subselect_has(&mut self, next: M, token: Vec<Vec<Token>>) -> Result<M, String> {
        let relative = token
            .iter()
            .any(|tokens| tokens.iter().any(sort_is_traversal));
        let scope = if relative {
            self.slots += 1;
            Scope::Slot(self.slots - 1)
        } else {
            Scope::None
        };
        let compiled = self.compile_token(token, Level::Nested, scope, relative)?;
        Ok(match (compiled.func, scope) {
            (M::False, _) => M::False,
            (inner, Scope::Slot(slot)) if !matches!(inner, M::True) => M::op(Op::HasRelative {
                slot,
                inner,
                test_next_siblings: compiled.should_test_next_siblings,
                next,
            }),
            // `inner` может быть `True`: `ensureIsTag(trueFunc)` проверяет только, что это элемент.
            (inner, _) => M::op(Op::Has { inner, next }),
        })
    }
}

/// compile.js `reduceRules`.
fn reduce_rules(a: M, b: M) -> M {
    if matches!(b, M::False) || matches!(a, M::True) {
        return a;
    }
    if matches!(a, M::False) || matches!(b, M::True) {
        return b;
    }
    M::op(Op::Or(a, b))
}

fn should_ignore_case(ignore_case: IgnoreCase, name: &str) -> bool {
    match ignore_case {
        IgnoreCase::True => true,
        IgnoreCase::False => false,
        // `!!options.quirksMode`
        IgnoreCase::Quirks => false,
        IgnoreCase::Unknown => CASE_INSENSITIVE_ATTRIBUTES.contains(&name),
    }
}

/// attributes.js `attributeRules[action]`.
fn compile_attribute(
    next: M,
    name: String,
    action: AttributeAction,
    value: String,
    ignore_case: IgnoreCase,
) -> M {
    let ic = should_ignore_case(ignore_case, &name);
    let test = match action {
        AttributeAction::Equals => {
            if ic {
                AttrTest::EqualsIc(jsstr::lower(&value))
            } else {
                AttrTest::Equals(value)
            }
        }
        AttributeAction::Hyphen => {
            let len = jsstr::utf16_len(&value);
            if ic {
                AttrTest::HyphenIc {
                    len,
                    value: jsstr::lower(&value),
                }
            } else {
                AttrTest::Hyphen { len, value }
            }
        }
        AttributeAction::Element => {
            if value.chars().any(dom::js::is_whitespace) {
                return M::False;
            }
            AttrTest::Element {
                len: jsstr::utf16_len(&value),
                value,
                ignore_case: ic,
            }
        }
        AttributeAction::Exists => AttrTest::Exists,
        AttributeAction::Start => {
            let len = jsstr::utf16_len(&value);
            if len == 0 {
                return M::False;
            }
            if ic {
                AttrTest::StartIc {
                    len,
                    value: jsstr::lower(&value),
                }
            } else {
                AttrTest::Start(value)
            }
        }
        AttributeAction::End => {
            let len = jsstr::utf16_len(&value);
            if len == 0 {
                return M::False;
            }
            if ic {
                AttrTest::EndIc {
                    len,
                    value: jsstr::lower(&value),
                }
            } else {
                AttrTest::End(value)
            }
        }
        AttributeAction::Any => {
            if value.is_empty() {
                return M::False;
            }
            if ic {
                AttrTest::AnyIc {
                    len: jsstr::utf16_len(&value),
                    value,
                }
            } else {
                AttrTest::Any(value)
            }
        }
        AttributeAction::Not => {
            if value.is_empty() {
                AttrTest::NotEmpty
            } else if ic {
                AttrTest::NotIc(jsstr::lower(&value))
            } else {
                AttrTest::Not(value)
            }
        }
    };
    let proto = match name.as_str() {
        "constructor" => ProtoMember::Constructor,
        "__proto__" => ProtoMember::Proto,
        _ => ProtoMember::None,
    };
    M::op(Op::Attr {
        name,
        proto,
        test,
        next,
    })
}
