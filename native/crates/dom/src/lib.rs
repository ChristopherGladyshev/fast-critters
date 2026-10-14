//! Arena-DOM с формой и семантикой мутаций domhandler 5 / domutils 3 —
//! дерево, с которым работает critters. Узел 0 — всегда документ.

pub mod js;

pub type NodeId = usize;

pub const DOCUMENT: NodeId = 0;

/// `ElementType` domhandler для элементов: у `<script>` и `<style>` свой тип.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    Tag,
    Script,
    Style,
}

impl ElementKind {
    /// Что выбирает `new Element(name)`.
    pub fn for_name(name: &str) -> Self {
        match name {
            "script" => ElementKind::Script,
            "style" => ElementKind::Style,
            _ => ElementKind::Tag,
        }
    }
}

/// Атрибуты в порядке итерации JS-объекта: ключи-индексы массива по возрастанию, затем
/// остальные в порядке вставки (domhandler хранит `attribs` в plain object).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attrs {
    entries: Vec<(String, String)>,
}

/// Канонический индекс массива (`"0"`, `"17"`, не `"01"`) меньше 2^32 - 1.
fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    let value: u64 = key.parse().ok()?;
    (value < u32::MAX as u64).then_some(value as u32)
}

impl Attrs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(key, _)| key == name)
    }

    /// `attribs[name] = value`: заменяет на месте, иначе добавляет в конец.
    pub fn set(&mut self, name: &str, value: &str) {
        match self.entries.iter_mut().find(|(key, _)| key == name) {
            Some(entry) => entry.1 = value.to_owned(),
            None => self.entries.push((name.to_owned(), value.to_owned())),
        }
    }

    /// Семантика парсера: побеждает первое вхождение атрибута.
    pub fn insert_if_absent(&mut self, name: String, value: String) {
        if !self.has(&name) {
            self.entries.push((name, value));
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|(key, _)| key != name);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Итерирует как `Object.keys(attribs)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        let mut indexed: Vec<(u32, &(String, String))> = self
            .entries
            .iter()
            .filter_map(|entry| array_index(&entry.0).map(|index| (index, entry)))
            .collect();
        indexed.sort_by_key(|(index, _)| *index);
        indexed
            .into_iter()
            .map(|(_, entry)| entry)
            .chain(
                self.entries
                    .iter()
                    .filter(|entry| array_index(&entry.0).is_none()),
            )
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    /// В HTML-режиме парсер приводит к нижнему регистру.
    pub name: String,
    pub attrs: Attrs,
    pub kind: ElementKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeData {
    Document,
    Element(Element),
    Text(String),
    Comment(String),
    /// `<!doctype …>` / `<?…>`: domhandler `ProcessingInstruction` (`name`, `data`).
    Directive {
        name: String,
        data: String,
    },
    /// Появляется только в xmlMode / recognizeCDATA; оставлено для полноты.
    CData,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub data: NodeData,
}

#[derive(Clone, Debug)]
pub struct Dom {
    pub nodes: Vec<Node>,
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

impl Dom {
    pub fn new() -> Self {
        Dom {
            nodes: vec![Node {
                parent: None,
                children: Vec::new(),
                data: NodeData::Document,
            }],
        }
    }

    /// Отсоединённый узел, как `new Element(...)` / `new Text(...)`.
    pub fn create(&mut self, data: NodeData) -> NodeId {
        self.nodes.push(Node {
            parent: None,
            children: Vec::new(),
            data,
        });
        self.nodes.len() - 1
    }

    pub fn create_element(&mut self, name: &str) -> NodeId {
        self.create(NodeData::Element(Element {
            name: name.to_owned(),
            attrs: Attrs::new(),
            kind: ElementKind::for_name(name),
        }))
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id].parent
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id].children
    }

    pub fn element(&self, id: NodeId) -> Option<&Element> {
        match &self.nodes[id].data {
            NodeData::Element(element) => Some(element),
            _ => None,
        }
    }

    pub fn element_mut(&mut self, id: NodeId) -> Option<&mut Element> {
        match &mut self.nodes[id].data {
            NodeData::Element(element) => Some(element),
            _ => None,
        }
    }

    /// domhandler `isTag`: элементы tag, script и style.
    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].data, NodeData::Element(_))
    }

    pub fn name(&self, id: NodeId) -> Option<&str> {
        self.element(id).map(|element| element.name.as_str())
    }

    pub fn attr(&self, id: NodeId, name: &str) -> Option<&str> {
        self.element(id).and_then(|element| element.attrs.get(name))
    }

    /// critters `setAttribute` (no-op для не-элементов).
    pub fn set_attr(&mut self, id: NodeId, name: &str, value: &str) {
        if let Some(element) = self.element_mut(id) {
            element.attrs.set(name, value);
        }
    }

    pub fn remove_attr(&mut self, id: NodeId, name: &str) {
        if let Some(element) = self.element_mut(id) {
            element.attrs.remove(name);
        }
    }

    fn index_in_parent(&self, id: NodeId, parent: NodeId) -> Option<usize> {
        self.nodes[parent]
            .children
            .iter()
            .rposition(|&child| child == id)
    }

    /// Соседи через детей родителя: указатели prev/next в domutils
    /// у присоединённых узлов с ними согласованы.
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.nodes[id].parent?;
        let index = self.index_in_parent(id, parent)?;
        index.checked_sub(1).map(|i| self.nodes[parent].children[i])
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.nodes[id].parent?;
        let index = self.index_in_parent(id, parent)?;
        self.nodes[parent].children.get(index + 1).copied()
    }

    /// domutils `removeElement`.
    pub fn remove(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[id].parent
            && let Some(index) = self.index_in_parent(id, parent)
        {
            self.nodes[parent].children.remove(index);
        }
        self.nodes[id].parent = None;
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.remove(child);
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    /// domutils `prepend(reference, node)`: вставляет `node` прямо перед `reference`.
    pub fn insert_before(&mut self, reference: NodeId, node: NodeId) {
        self.remove(node);
        let parent = self.nodes[reference].parent;
        if let Some(parent) = parent {
            // domutils здесь использует indexOf (первое вхождение).
            let index = self.nodes[parent]
                .children
                .iter()
                .position(|&child| child == reference)
                .unwrap_or(0);
            self.nodes[parent].children.insert(index, node);
        }
        self.nodes[node].parent = parent;
    }

    /// `cloneNode(false)` для элемента: то же имя и атрибуты, без детей.
    pub fn clone_shallow(&mut self, id: NodeId) -> NodeId {
        let data = match &self.nodes[id].data {
            NodeData::Element(element) => NodeData::Element(Element {
                name: element.name.clone(),
                attrs: element.attrs.clone(),
                kind: ElementKind::for_name(&element.name),
            }),
            other => other.clone(),
        };
        self.create(data)
    }

    /// domutils `getText` (геттер `textContent` в critters).
    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        match &self.nodes[id].data {
            NodeData::Text(text) => out.push_str(text),
            NodeData::Element(element) if element.name == "br" => out.push('\n'),
            NodeData::Element(_) | NodeData::CData => {
                for &child in &self.nodes[id].children {
                    self.collect_text(child, out);
                }
            }
            _ => {}
        }
    }

    /// Сеттер `textContent` в critters: дети заменяются одним новым текстовым узлом.
    pub fn set_text_content(&mut self, id: NodeId, text: &str) {
        // В domhandler старые дети сохраняют указатель на родителя; они в любом случае недостижимы.
        self.nodes[id].children.clear();
        let child = self.create(NodeData::Text(text.to_owned()));
        self.append_child(id, child);
    }

    /// Является ли `id` потомком `ancestor` (не самим `ancestor`).
    pub fn is_descendant_of(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut current = self.nodes[id].parent;
        while let Some(node) = current {
            if node == ancestor {
                return true;
            }
            current = self.nodes[node].parent;
        }
        false
    }

    /// Потомки `root` в порядке документа (pre-order), без самого `root`.
    pub fn descendants(&self, root: NodeId) -> Descendants<'_> {
        let mut stack: Vec<NodeId> = self.nodes[root].children.iter().rev().copied().collect();
        stack.reserve(64);
        Descendants { dom: self, stack }
    }
}

pub struct Descendants<'a> {
    dom: &'a Dom,
    stack: Vec<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        self.stack
            .extend(self.dom.nodes[id].children.iter().rev().copied());
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attrs_iterate_like_js_objects() {
        let mut attrs = Attrs::new();
        for key in ["b", "10", "a", "2", "01", "4294967295", "4294967294"] {
            attrs.insert_if_absent(key.to_owned(), String::new());
        }
        attrs.insert_if_absent("b".to_owned(), "dup".to_owned());
        let keys: Vec<&str> = attrs.iter().map(|(key, _)| key).collect();
        assert_eq!(
            keys,
            ["2", "10", "4294967294", "b", "a", "01", "4294967295"]
        );
        assert_eq!(attrs.get("b"), Some(""));
    }

    #[test]
    fn mutations_follow_domutils() {
        let mut dom = Dom::new();
        let html = dom.create_element("html");
        dom.append_child(DOCUMENT, html);
        let link = dom.create_element("link");
        dom.append_child(html, link);
        let style = dom.create_element("style");
        dom.insert_before(link, style);
        let noscript = dom.create_element("noscript");
        dom.append_child(html, noscript);
        assert_eq!(dom.children(html), [style, link, noscript]);
        assert_eq!(dom.prev_sibling(link), Some(style));
        assert_eq!(dom.next_sibling(link), Some(noscript));

        dom.set_text_content(style, "a{}");
        assert_eq!(dom.text_content(html), "a{}");
        dom.remove(style);
        assert_eq!(dom.children(html), [link, noscript]);
        assert!(!dom.is_descendant_of(style, DOCUMENT));
        assert_eq!(
            dom.descendants(DOCUMENT).collect::<Vec<_>>(),
            [html, link, noscript]
        );

        dom.set_attr(link, "rel", "stylesheet");
        let clone = dom.clone_shallow(link);
        assert_eq!(dom.attr(clone, "rel"), Some("stylesheet"));
        assert_eq!(dom.element(style).unwrap().kind, ElementKind::Style);
    }
}
