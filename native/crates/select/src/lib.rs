//! Порт css-what 6.2.2 + css-select 5.2.2 (+ nth-check 2.1.1, адаптер domutils 3)
//! в режиме HTML с опциями по умолчанию поверх арены [`dom`].
//!
//! [`Query`] компилируется *под контекст*: css-select абсолютизирует селектор
//! (префикс `:scope `), если родитель контекстного элемента — элемент, а
//! `:scope` ссылается на контекстный узел по id. Переиспользовать `Query` можно только
//! с тем контекстом (id узла, тот же вид родителя), под который он скомпилирован.

mod compile;
mod jsstr;
mod nth;
pub mod parse;

use dom::{Dom, NodeData, NodeId};

use compile::{AttrTest, Compiler, M, NthKind, Op, ProtoMember, Scope, SimplePseudo};

pub use parse::{
    AttributeAction, IgnoreCase, PseudoData, SelectorType, Token, parse, selectors_to_json,
};

/// Скомпилированный селектор (css-select `compileUnsafe(selector, {}, context)`).
#[derive(Clone, Debug)]
pub struct Query {
    func: M,
    should_test_next_siblings: bool,
    slots: usize,
    caches: usize,
}

impl Query {
    /// Является ли скомпилированная функция `falseFunc` из boolbase (никогда не матчит;
    /// `selectOne` возвращает `null` без поиска).
    pub fn is_never(&self) -> bool {
        matches!(self.func, M::False)
    }
}

/// `compileUnsafe(selector, {}, context)`: парсинг + компиляция; `Err` — сообщение JS.
pub fn compile(dom: &Dom, selector: &str, context: NodeId) -> Result<Query, String> {
    let token = parse(selector)?;
    let mut compiler = Compiler::new(dom);
    let compiled = compiler.compile_top(token, context)?;
    Ok(Query {
        func: compiled.func,
        should_test_next_siblings: compiled.should_test_next_siblings,
        slots: compiler.slots,
        caches: compiler.caches,
    })
}

/// `cssSelect.selectOne(selector, context)` за один вызов (`querySelector` в critters).
pub fn query_selector(
    dom: &Dom,
    selector: &str,
    context: NodeId,
) -> Result<Option<NodeId>, String> {
    let query = compile(dom, selector, context)?;
    try_select_one(dom, &query, context)
}

/// `selectOne` по скомпилированному запросу. В JS сам матчинг может бросить исключение
/// (только для селекторов атрибутов с именем `constructor` / `__proto__`); здесь это даёт `None`,
/// сообщение — через [`try_select_one`].
pub fn select_one(dom: &Dom, query: &Query, context: NodeId) -> Option<NodeId> {
    try_select_one(dom, query, context).ok().flatten()
}

pub fn select_all(dom: &Dom, query: &Query, context: NodeId) -> Vec<NodeId> {
    try_select_all(dom, query, context).unwrap_or_default()
}

/// Скомпилированная функция, применённая к одному элементу (`false` для не-элементов).
pub fn is_match(dom: &Dom, query: &Query, element: NodeId) -> bool {
    try_is_match(dom, query, element).unwrap_or(false)
}

pub fn try_select_one(dom: &Dom, query: &Query, context: NodeId) -> Result<Option<NodeId>, String> {
    if query.is_never() {
        return Ok(None);
    }
    let roots = prepare_context(dom, context, query.should_test_next_siblings);
    let mut ctx = Ctx::new(dom, query);
    let mut stack: Vec<(&[NodeId], usize)> = vec![(roots.as_slice(), 0)];
    while let Some(top) = stack.last_mut() {
        let (nodes, index) = *top;
        if index >= nodes.len() {
            stack.pop();
            continue;
        }
        top.1 += 1;
        let node = nodes[index];
        if dom.is_element(node) {
            let matched = ctx.test(&query.func, node);
            if let Some(error) = ctx.error.take() {
                return Err(error);
            }
            if matched {
                return Ok(Some(node));
            }
        }
        let children = dom.children(node);
        if !children.is_empty() {
            stack.push((children, 0));
        }
    }
    Ok(None)
}

pub fn try_select_all(dom: &Dom, query: &Query, context: NodeId) -> Result<Vec<NodeId>, String> {
    let mut result = Vec::new();
    if query.is_never() {
        return Ok(result);
    }
    let roots = prepare_context(dom, context, query.should_test_next_siblings);
    let mut ctx = Ctx::new(dom, query);
    let mut stack: Vec<(&[NodeId], usize)> = vec![(roots.as_slice(), 0)];
    while let Some(top) = stack.last_mut() {
        let (nodes, index) = *top;
        if index >= nodes.len() {
            stack.pop();
            continue;
        }
        top.1 += 1;
        let node = nodes[index];
        if dom.is_element(node) {
            let matched = ctx.test(&query.func, node);
            if let Some(error) = ctx.error.take() {
                return Err(error);
            }
            if matched {
                result.push(node);
            }
        }
        let children = dom.children(node);
        if !children.is_empty() {
            stack.push((children, 0));
        }
    }
    Ok(result)
}

pub fn try_is_match(dom: &Dom, query: &Query, element: NodeId) -> Result<bool, String> {
    if !dom.is_element(element) {
        return Ok(false);
    }
    let mut ctx = Ctx::new(dom, query);
    let matched = ctx.test(&query.func, element);
    match ctx.error.take() {
        Some(error) => Err(error),
        None => Ok(matched),
    }
}

/// css-select `prepareContext` для одного узла.
fn prepare_context(dom: &Dom, context: NodeId, should_test_next_siblings: bool) -> Vec<NodeId> {
    if should_test_next_siblings {
        // appendNextSiblings + removeSubsets: контекст и следующие за ним элементы-сиблинги.
        let mut elems = vec![context];
        elems.extend(next_element_siblings(dom, context));
        elems
    } else {
        dom.children(context).to_vec()
    }
}

/// subselects.js `getNextSiblings`.
fn next_element_siblings(dom: &Dom, elem: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let siblings: &[NodeId] = match dom.parent(elem) {
        Some(parent) => dom.children(parent),
        None => &[],
    };
    let start = siblings
        .iter()
        .position(|&s| s == elem)
        .map_or(siblings.len(), |i| i + 1);
    siblings[start..]
        .iter()
        .copied()
        .filter(move |&s| dom.is_element(s))
}

/// `"" + Object` (что видят `i`-регэкспы для `attribs.constructor`).
const OBJECT_FN_SOURCE: &str = "function Object() { [native code] }";

/// `elem.attribs[name]`: строка, `undefined` или член `Object.prototype`.
enum AttrVal<'a> {
    Missing,
    Str(&'a str),
    /// `Object` (функция, `length` 1).
    Ctor,
    /// `Object.prototype`.
    Proto,
}

struct Ctx<'a> {
    dom: &'a Dom,
    caches: Vec<Vec<u64>>,
    slots: Vec<NodeId>,
    error: Option<String>,
}

const PLACEHOLDER: NodeId = usize::MAX;

impl<'a> Ctx<'a> {
    fn new(dom: &'a Dom, query: &Query) -> Self {
        Ctx {
            dom,
            caches: vec![Vec::new(); query.caches],
            slots: vec![PLACEHOLDER; query.slots],
            error: None,
        }
    }

    fn fail(&mut self, message: &str) -> bool {
        if self.error.is_none() {
            self.error = Some(message.to_owned());
        }
        false
    }

    fn element_parent(&self, node: NodeId) -> Option<NodeId> {
        self.dom.parent(node).filter(|&p| self.dom.is_element(p))
    }

    fn siblings(&self, elem: &'a NodeId) -> &'a [NodeId] {
        match self.dom.parent(*elem) {
            Some(parent) => self.dom.children(parent),
            None => std::slice::from_ref(elem),
        }
    }

    fn prev_element_sibling(&self, elem: NodeId) -> Option<NodeId> {
        let parent = self.dom.parent(elem)?;
        let siblings = self.dom.children(parent);
        let index = siblings.iter().rposition(|&s| s == elem)?;
        siblings[..index]
            .iter()
            .rev()
            .copied()
            .find(|&s| self.dom.is_element(s))
    }

    fn name(&self, elem: NodeId) -> &'a str {
        self.dom.name(elem).unwrap_or("")
    }

    #[inline]
    fn attr(&self, elem: NodeId, name: &str, proto: ProtoMember) -> AttrVal<'a> {
        if proto == ProtoMember::Proto {
            return AttrVal::Proto;
        }
        match self.dom.attr(elem, name) {
            Some(value) => AttrVal::Str(value),
            None if proto == ProtoMember::Constructor => AttrVal::Ctor,
            None => AttrVal::Missing,
        }
    }

    #[inline]
    fn test(&mut self, m: &M, elem: NodeId) -> bool {
        match m {
            M::True => true,
            M::False => false,
            M::Op(op) => self.op(op, elem),
        }
    }

    fn cache_has(&self, cache: usize, node: NodeId) -> bool {
        self.caches[cache]
            .get(node / 64)
            .is_some_and(|word| word & (1 << (node % 64)) != 0)
    }

    fn cache_add(&mut self, cache: usize, node: NodeId) {
        let bits = &mut self.caches[cache];
        if bits.is_empty() {
            bits.resize(self.dom.nodes.len().div_ceil(64), 0);
        }
        if node / 64 >= bits.len() {
            bits.resize(node / 64 + 1, 0);
        }
        bits[node / 64] |= 1 << (node % 64);
    }

    fn op(&mut self, op: &Op, elem: NodeId) -> bool {
        match op {
            Op::Tag { name, next } => self.name(elem) == name && self.test(next, elem),
            Op::Attr {
                name,
                proto,
                test,
                next,
            } => {
                let value = self.attr(elem, name, *proto);
                self.attr_test(test, value) && self.test(next, elem)
            }
            Op::Descendant { cache, next } => {
                let mut current = elem;
                match cache {
                    None => {
                        while let Some(parent) = self.element_parent(current) {
                            current = parent;
                            if self.test(next, current) {
                                return true;
                            }
                        }
                    }
                    Some(cache) => {
                        while let Some(parent) = self.element_parent(current) {
                            current = parent;
                            if !self.cache_has(*cache, current) {
                                if self.test(next, current) {
                                    return true;
                                }
                                self.cache_add(*cache, current);
                            }
                        }
                    }
                }
                false
            }
            Op::FlexibleDescendant { next } => {
                let mut current = elem;
                loop {
                    if self.test(next, current) {
                        return true;
                    }
                    match self.element_parent(current) {
                        Some(parent) => current = parent,
                        None => return false,
                    }
                }
            }
            Op::Parent { next } => {
                let dom = self.dom;
                dom.children(elem)
                    .iter()
                    .any(|&child| dom.is_element(child) && self.test(next, child))
            }
            Op::Child { next } => match self.element_parent(elem) {
                Some(parent) => self.test(next, parent),
                None => false,
            },
            Op::Sibling { next } => {
                let dom = self.dom;
                for &sibling in self.siblings(&elem) {
                    if sibling == elem {
                        break;
                    }
                    if dom.is_element(sibling) && self.test(next, sibling) {
                        return true;
                    }
                }
                false
            }
            Op::Adjacent { next } => match self.prev_element_sibling(elem) {
                Some(previous) => self.test(next, previous),
                None => false,
            },
            Op::Contains { text, next } => {
                self.test(next, elem) && self.dom.text_content(elem).contains(text.as_str())
            }
            Op::IContains { text, next } => {
                self.test(next, elem)
                    && jsstr::lower(&self.dom.text_content(elem)).contains(text.as_str())
            }
            Op::Nth { kind, check, next } => {
                let pos = self.nth_position(*kind, elem);
                check.test(pos) && self.test(next, elem)
            }
            Op::HasElementParent { next } => {
                self.element_parent(elem).is_some() && self.test(next, elem)
            }
            Op::Root { next } => self.element_parent(elem).is_none() && self.test(next, elem),
            Op::Scope { scope, next } => {
                let target = match *scope {
                    Scope::Node(node) => node,
                    Scope::Slot(slot) => self.slots[slot],
                    Scope::None => unreachable!("compiled as :root"),
                };
                target == elem && self.test(next, elem)
            }
            Op::Pseudo { kind, next } => self.simple_pseudo(*kind, elem) && self.test(next, elem),
            Op::Is { func, next } => self.test(func, elem) && self.test(next, elem),
            Op::Not { func, next } => !self.test(func, elem) && self.test(next, elem),
            Op::Has { inner, next } => {
                if !self.test(next, elem) {
                    return false;
                }
                let dom = self.dom;
                self.exists_one(inner, dom.children(elem))
            }
            Op::HasRelative {
                slot,
                inner,
                test_next_siblings,
                next,
            } => {
                if !self.test(next, elem) {
                    return false;
                }
                self.slots[*slot] = elem;
                let dom = self.dom;
                if self.exists_one(inner, dom.children(elem)) {
                    return true;
                }
                if *test_next_siblings {
                    let siblings: Vec<NodeId> = next_element_siblings(dom, elem).collect();
                    return self.exists_one(inner, &siblings);
                }
                false
            }
            Op::Or(a, b) => self.test(a, elem) || self.test(b, elem),
        }
    }

    fn exists_one(&mut self, inner: &M, nodes: &[NodeId]) -> bool {
        let dom = self.dom;
        nodes.iter().any(|&node| {
            (dom.is_element(node) && self.test(inner, node))
                || self.exists_one(inner, dom.children(node))
        })
    }

    fn nth_position(&self, kind: NthKind, elem: NodeId) -> usize {
        let dom = self.dom;
        let siblings = self.siblings(&elem);
        let counts = |s: NodeId| match kind {
            NthKind::Child | NthKind::LastChild => dom.is_element(s),
            NthKind::OfType | NthKind::LastOfType => {
                dom.is_element(s) && dom.name(s) == dom.name(elem)
            }
        };
        match kind {
            NthKind::Child | NthKind::OfType => siblings
                .iter()
                .take_while(|&&s| s != elem)
                .filter(|&&s| counts(s))
                .count(),
            NthKind::LastChild | NthKind::LastOfType => siblings
                .iter()
                .rev()
                .take_while(|&&s| s != elem)
                .filter(|&&s| counts(s))
                .count(),
        }
    }

    fn simple_pseudo(&self, kind: SimplePseudo, elem: NodeId) -> bool {
        let dom = self.dom;
        match kind {
            SimplePseudo::Empty => !dom.children(elem).iter().any(|&child| {
                dom.is_element(child)
                    || match &dom.node(child).data {
                        NodeData::Text(text) => !text.is_empty(),
                        NodeData::CData => !dom.text_content(child).is_empty(),
                        _ => false,
                    }
            }),
            SimplePseudo::FirstChild => self.prev_element_sibling(elem).is_none(),
            SimplePseudo::LastChild => {
                for &sibling in self.siblings(&elem).iter().rev() {
                    if sibling == elem {
                        return true;
                    }
                    if dom.is_element(sibling) {
                        break;
                    }
                }
                false
            }
            SimplePseudo::FirstOfType | SimplePseudo::LastOfType => {
                let name = self.name(elem);
                let siblings = self.siblings(&elem);
                let mut iter: Box<dyn Iterator<Item = &NodeId>> =
                    if kind == SimplePseudo::FirstOfType {
                        Box::new(siblings.iter())
                    } else {
                        Box::new(siblings.iter().rev())
                    };
                iter.find(|&&s| s == elem || (dom.is_element(s) && dom.name(s) == Some(name)))
                    .is_some_and(|&s| s == elem)
            }
            SimplePseudo::OnlyOfType => {
                let name = self.name(elem);
                self.siblings(&elem)
                    .iter()
                    .all(|&s| s == elem || !dom.is_element(s) || dom.name(s) != Some(name))
            }
            SimplePseudo::OnlyChild => self
                .siblings(&elem)
                .iter()
                .all(|&s| s == elem || !dom.is_element(s)),
        }
    }

    /// attributes.js, включая случай, когда `attribs[name]` резолвится в член
    /// `Object.prototype`.
    fn attr_test(&mut self, test: &AttrTest, value: AttrVal<'a>) -> bool {
        let attr = match value {
            AttrVal::Str(attr) => attr,
            AttrVal::Missing => {
                return matches!(test, AttrTest::Not(_) | AttrTest::NotIc(_));
            }
            AttrVal::Ctor | AttrVal::Proto => {
                let ctor = matches!(value, AttrVal::Ctor);
                return match test {
                    AttrTest::Exists | AttrTest::Equals(_) => false,
                    AttrTest::EqualsIc(v) => {
                        if ctor && jsstr::utf16_len(v) == 1 {
                            self.fail("attr.toLowerCase is not a function")
                        } else {
                            false
                        }
                    }
                    AttrTest::Hyphen { len, .. } | AttrTest::HyphenIc { len, .. } => {
                        if ctor && *len == 1 {
                            self.fail("attr.substr is not a function")
                        } else {
                            self.fail("attr.charAt is not a function")
                        }
                    }
                    AttrTest::Element {
                        value,
                        ignore_case,
                        len,
                    } => {
                        ctor && *len <= 1
                            && jsstr::contains_word(OBJECT_FN_SOURCE, value, *ignore_case)
                    }
                    AttrTest::Start(_) => self.fail("_a.startsWith is not a function"),
                    AttrTest::StartIc { len, .. } => {
                        if ctor && *len <= 1 {
                            self.fail("attr.substr is not a function")
                        } else {
                            false
                        }
                    }
                    AttrTest::End(_) => self.fail("_a.endsWith is not a function"),
                    AttrTest::EndIc { .. } => self.fail("_a.substr is not a function"),
                    AttrTest::Any(_) => self.fail("_a.includes is not a function"),
                    AttrTest::AnyIc { value, len } => {
                        ctor && *len <= 1 && jsstr::contains_ci(OBJECT_FN_SOURCE, value)
                    }
                    AttrTest::NotEmpty | AttrTest::Not(_) => true,
                    AttrTest::NotIc(v) => {
                        if ctor && jsstr::utf16_len(v) == 1 {
                            self.fail("attr.toLowerCase is not a function")
                        } else {
                            true
                        }
                    }
                };
            }
        };
        match test {
            AttrTest::Exists => true,
            AttrTest::Equals(v) => attr == v,
            AttrTest::EqualsIc(v) => {
                if attr.is_ascii() && v.is_ascii() {
                    attr.len() == v.len() && attr.eq_ignore_ascii_case(v)
                } else {
                    jsstr::utf16_len(attr) == jsstr::utf16_len(v) && jsstr::lower(attr) == *v
                }
            }
            AttrTest::Hyphen { len, value } => {
                (jsstr::utf16_len(attr) == *len || jsstr::char_at_is_hyphen(attr, *len))
                    && jsstr::prefix_eq(attr, *len, value)
            }
            AttrTest::HyphenIc { len, value } => {
                (jsstr::utf16_len(attr) == *len || jsstr::char_at_is_hyphen(attr, *len))
                    && jsstr::substr_lower_eq(attr, 0, *len, value)
            }
            // `attr.length >= value.length` следует из совпадения регэкспа.
            AttrTest::Element {
                value, ignore_case, ..
            } => jsstr::contains_word(attr, value, *ignore_case),
            AttrTest::Start(v) => attr.starts_with(v.as_str()),
            AttrTest::StartIc { len, value } => {
                jsstr::utf16_len(attr) >= *len && jsstr::substr_lower_eq(attr, 0, *len, value)
            }
            AttrTest::End(v) => attr.ends_with(v.as_str()),
            AttrTest::EndIc { len, value } => {
                let start = jsstr::utf16_len(attr).saturating_sub(*len);
                jsstr::substr_lower_eq(attr, start, usize::MAX, value)
            }
            AttrTest::Any(v) => attr.contains(v.as_str()),
            AttrTest::AnyIc { value, .. } => jsstr::contains_ci(attr, value),
            AttrTest::NotEmpty => !attr.is_empty(),
            AttrTest::Not(v) => attr != v,
            AttrTest::NotIc(v) => {
                if attr.is_ascii() && v.is_ascii() {
                    !(attr.len() == v.len() && attr.eq_ignore_ascii_case(v))
                } else {
                    jsstr::utf16_len(attr) != jsstr::utf16_len(v) || jsstr::lower(attr) != *v
                }
            }
        }
    }
}
