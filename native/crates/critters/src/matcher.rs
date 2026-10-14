//! `crittersContainer.exists(selector)`: `cachedQuerySelector` из critters, отвечающий
//! по снимку class/id документа и индексу элементов вместо обхода DOM.

use std::collections::{HashMap, HashSet};

use dom::{Dom, ElementKind, NodeData, NodeId, js};
use select::{AttributeAction, IgnoreCase, Token};

/// `buildCache(container)` из critters в том виде, как он работал в `createDocument()`, с его особенностями:
/// классы делятся по `' '` только после `trim()`, id обрезаются, контейнер включён,
/// спуск только в потомков с `type === 'tag'`.
pub struct ClassCache {
    classes: HashSet<String>,
    ids: HashSet<String>,
}

impl ClassCache {
    pub fn build(dom: &Dom, container: NodeId) -> Self {
        let mut classes = HashSet::new();
        let mut ids = HashSet::new();
        let mut queue = std::collections::VecDeque::from([container]);
        while let Some(node) = queue.pop_front() {
            if let Some(class) = dom.attr(node, "class") {
                for name in js::trim(class).split(' ') {
                    classes.insert(name.to_owned());
                }
            }
            if let Some(id) = dom.attr(node, "id") {
                ids.insert(js::trim(id).to_owned());
            }
            queue.extend(dom.children(node).iter().copied().filter(|&child| {
                matches!(&dom.node(child).data, NodeData::Element(element) if element.kind == ElementKind::Tag)
            }));
        }
        ClassCache { classes, ids }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum KeyKind {
    Tag,
    Class,
    Id,
}

type Key<'a> = (KeyKind, &'a str);

/// Элементы под контейнером по tag / class / id (сам контейнер
/// исключён, как в контексте поиска css-select). Каждый элемент, который может совпасть
/// с селектором, несёт все ключи его правого compound, поэтому одна корзина содержит всех
/// кандидатов.
struct Index {
    buckets: HashMap<(KeyKind, String), Vec<NodeId>>,
    /// Число потомков каждого индексированного узла (по `NodeId`).
    subtree: Vec<u32>,
}

impl Index {
    fn build(dom: &Dom, root: NodeId) -> Self {
        let mut buckets: HashMap<(KeyKind, String), Vec<NodeId>> = HashMap::new();
        let mut subtree = vec![0u32; dom.nodes.len()];
        let order: Vec<NodeId> = dom.descendants(root).collect();
        for &node in order.iter().rev() {
            if let Some(parent) = dom.parent(node) {
                subtree[parent] += subtree[node] + 1;
            }
        }
        for node in order {
            let Some(element) = dom.element(node) else {
                continue;
            };
            buckets
                .entry((KeyKind::Tag, element.name.clone()))
                .or_default()
                .push(node);
            if let Some(class) = element.attrs.get("class") {
                let mut seen: Vec<&str> = Vec::new();
                for name in class
                    .split(js::is_whitespace)
                    .filter(|name| !name.is_empty())
                {
                    if !seen.contains(&name) {
                        seen.push(name);
                        buckets
                            .entry((KeyKind::Class, name.to_owned()))
                            .or_default()
                            .push(node);
                    }
                }
            }
            if let Some(id) = element.attrs.get("id") {
                buckets
                    .entry((KeyKind::Id, id.to_owned()))
                    .or_default()
                    .push(node);
            }
        }
        Index { buckets, subtree }
    }

    fn bucket(&self, (kind, value): Key) -> &[NodeId] {
        // Ключей на селектор мало; аллокация дешевле map с заимствованными ключами.
        self.buckets
            .get(&(kind, value.to_owned()))
            .map_or(&[], Vec::as_slice)
    }
}

fn key_of(token: &Token) -> Option<Key<'_>> {
    match token {
        Token::Tag {
            name,
            namespace: None,
        } if name != "*" && name.is_ascii() => Some((KeyKind::Tag, name.as_str())),
        Token::Attribute {
            name,
            action,
            value,
            namespace: None,
            ignore_case,
        } if *ignore_case != IgnoreCase::True => match (name.as_str(), action) {
            ("class", AttributeAction::Element) => Some((KeyKind::Class, value.as_str())),
            ("id", AttributeAction::Equals) => Some((KeyKind::Id, value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// Ключи каждого compound слева направо (аргументы псевдоклассов не
/// обязательны) и признак, что все комбинаторы — `' '` или `>`: тогда совпадение
/// лежит в поддереве элемента, совпавшего с любым более левым compound.
fn compound_keys(selector: &[Token]) -> (Vec<Vec<Key<'_>>>, bool) {
    let mut compounds = vec![Vec::new()];
    let mut nested = true;
    for token in selector {
        if token.is_traversal() {
            nested &= matches!(token, Token::Descendant | Token::Child);
            compounds.push(Vec::new());
        } else if let Some(key) = key_of(token) {
            compounds.last_mut().expect("never empty").push(key);
        }
    }
    (compounds, nested)
}

/// Где искать элемент, совпадающий с одним подселектором.
enum Candidates<'a> {
    /// Элементы с ключом правого compound.
    Bucket(&'a [NodeId]),
    /// Потомки элементов с ключом более левого compound.
    Subtrees(&'a [NodeId]),
}

fn element_has_key(dom: &Dom, node: NodeId, (kind, value): Key) -> bool {
    let Some(element) = dom.element(node) else {
        return false;
    };
    match kind {
        KeyKind::Tag => element.name == value,
        KeyKind::Id => element.attrs.get("id") == Some(value),
        KeyKind::Class => element
            .attrs
            .get("class")
            .is_some_and(|class| class.split(js::is_whitespace).any(|name| name == value)),
    }
}

/// Имена атрибутов, которые css-select читает через обычный объект: сопоставление по ним может
/// бросить исключение в JS, поэтому они всегда идут полным обходом, чтобы выдать ту же ошибку.
const PROTOTYPE_NAMES: [&str; 12] = [
    "constructor",
    "__proto__",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "toLocaleString",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
];

/// Наименьшая корзина среди ключей более левых compound, когда селектор только
/// вкладывает (`' '` / `>`), так что любое совпадение — потомок элемента, совпавшего с этим
/// compound. Compound, с которыми может совпасть сам контейнер, пропускаются: он
/// предок всего, но в индекс не входит.
fn smallest_anchor_bucket<'a>(
    dom: &Dom,
    container: NodeId,
    index: &'a Index,
    earlier: &[Vec<Key>],
    nested: bool,
) -> Option<&'a [NodeId]> {
    if !nested {
        return None;
    }
    earlier
        .iter()
        .filter(|keys| {
            keys.iter()
                .any(|&key| !element_has_key(dom, container, key))
        })
        .flatten()
        .map(|&key| index.bucket(key))
        .min_by_key(|bucket| bucket.len())
}

pub struct Matcher {
    container: NodeId,
    cache: ClassCache,
    /// css-select ограничивает запросы контейнером ниже элемента: тогда без индекса.
    indexed: bool,
    index: Option<Index>,
    detached: bool,
    memo: HashMap<String, Result<bool, String>>,
}

impl Matcher {
    pub fn new(dom: &Dom, container: NodeId, cache: ClassCache) -> Self {
        let indexed = !dom
            .parent(container)
            .is_some_and(|parent| dom.is_element(parent));
        Matcher {
            container,
            cache,
            indexed,
            index: None,
            detached: false,
            memo: HashMap::new(),
        }
    }

    /// Документ изменился (заменён текст стиля): структурные ответы могут отличаться.
    pub fn dom_changed(&mut self) {
        self.memo.clear();
    }

    /// Стиль удалён из DOM: индексированные элементы могут быть отсоединены.
    pub fn style_removed(&mut self) {
        self.memo.clear();
        self.detached = true;
    }

    /// `cachedQuerySelector(selector, container)` из critters.
    pub fn exists(&mut self, dom: &Dom, selector: &str) -> Result<bool, String> {
        if let Some(known) = self.memo.get(selector) {
            return known.clone();
        }
        let result = self.evaluate(dom, selector);
        self.memo.insert(selector.to_owned(), result.clone());
        result
    }

    fn evaluate(&mut self, dom: &Dom, selector: &str) -> Result<bool, String> {
        let tokens = select::parse(selector)?;
        if let Some(cached) = self.cached_answer(&tokens) {
            return Ok(cached);
        }
        if let Some(indexed) = self.indexed_answer(dom, selector, &tokens) {
            return Ok(indexed);
        }
        Ok(select::query_selector(dom, selector, self.container)?.is_some())
    }

    /// Первый подселектор атрибута `class`/`id` из одного токена, при любом action.
    fn cached_answer(&self, tokens: &[Vec<Token>]) -> Option<bool> {
        tokens
            .iter()
            .find_map(|subselector| match subselector.as_slice() {
                [Token::Attribute { name, value, .. }] if name == "class" => {
                    Some(self.cache.classes.contains(value))
                }
                [Token::Attribute { name, value, .. }] if name == "id" => {
                    Some(self.cache.ids.contains(value))
                }
                _ => None,
            })
    }

    /// Ответ по индексу; `None`, когда точно ответит только полный обход.
    fn indexed_answer(&mut self, dom: &Dom, selector: &str, tokens: &[Vec<Token>]) -> Option<bool> {
        if !self.indexed
            || selector.contains("scope")
            || PROTOTYPE_NAMES.iter().any(|name| selector.contains(name))
        {
            return None;
        }
        // Ошибки компиляции всплывают из полного пути с сообщением css-select, даже
        // для селекторов, которые индекс отсёк бы.
        let query = select::compile(dom, selector, self.container).ok()?;
        let container = self.container;
        let index = self
            .index
            .get_or_insert_with(|| Index::build(dom, container));

        let mut candidates: Vec<Candidates> = Vec::with_capacity(tokens.len());
        for subselector in tokens {
            let (compounds, nested) = compound_keys(subselector);
            let satisfiable = compounds
                .iter()
                .flatten()
                .all(|&key| !index.bucket(key).is_empty() || element_has_key(dom, container, key));
            if !satisfiable {
                continue;
            }
            let (rightmost, earlier) = compounds.split_last().expect("never empty");
            let Some(bucket) = rightmost
                .iter()
                .map(|&key| index.bucket(key))
                .min_by_key(|bucket| bucket.len())
            else {
                // Справа ключа нет: якоря всё равно ограничивают пространство поиска.
                let anchors = smallest_anchor_bucket(dom, container, index, earlier, nested)?;
                candidates.push(Candidates::Subtrees(anchors));
                continue;
            };
            let subtree_cost = |anchors: &[NodeId]| -> usize {
                anchors
                    .iter()
                    .map(|&anchor| index.subtree.get(anchor).copied().unwrap_or(u32::MAX) as usize)
                    .sum()
            };
            match smallest_anchor_bucket(dom, container, index, earlier, nested) {
                Some(anchors) if subtree_cost(anchors) < bucket.len() => {
                    candidates.push(Candidates::Subtrees(anchors));
                }
                _ => candidates.push(Candidates::Bucket(bucket)),
            }
        }
        if candidates
            .iter()
            .all(|candidates| matches!(candidates, Candidates::Bucket(bucket) if bucket.is_empty()))
        {
            return Some(false);
        }

        let detached = self.detached;
        let test = |element: NodeId| -> Option<bool> {
            if !dom.is_element(element) || (detached && !dom.is_descendant_of(element, container)) {
                return Some(false);
            }
            select::try_is_match(dom, &query, element).ok()
        };
        for candidates in candidates {
            match candidates {
                Candidates::Bucket(bucket) => {
                    for &element in bucket {
                        if test(element)? {
                            return Some(true);
                        }
                    }
                }
                Candidates::Subtrees(anchors) => {
                    for &anchor in anchors {
                        for element in dom.descendants(anchor) {
                            if test(element)? {
                                return Some(true);
                            }
                        }
                    }
                }
            }
        }
        Some(false)
    }
}
