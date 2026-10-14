//! Порт htmlparser2 8.0.2 `Tokenizer` + `Parser` (HTML-режим, `decodeEntities: false`),
//! питающий порт domhandler 5.0.3 `DomHandler`.
//!
//! Токенизатор работает с байтами UTF-8, а JS — с кодовыми единицами UTF-16. Каждое
//! его решение — сравнение с ASCII (напрямую или через `c | 0x20`), и ни один
//! не-ASCII юнит/байт не равен ASCII, поэтому оба прохода
//! идут по одним переходам и каждая граница среза попадает на границу символа.

use std::collections::HashSet;

use dom::{Attrs, DOCUMENT, Dom, Element, ElementKind, Node, NodeData, NodeId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Text,
    BeforeTagName,
    InTagName,
    InSelfClosingTag,
    BeforeClosingTagName,
    InClosingTagName,
    AfterClosingTagName,
    BeforeAttributeName,
    InAttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    InAttributeValueDq,
    InAttributeValueSq,
    InAttributeValueNq,
    BeforeDeclaration,
    InDeclaration,
    InProcessingInstruction,
    BeforeComment,
    CdataSequence,
    InSpecialComment,
    InCommentLike,
    BeforeSpecialS,
    SpecialStartSequence,
    InSpecialTag,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Seq {
    Cdata,
    CdataEnd,
    CommentEnd,
    ScriptEnd,
    StyleEnd,
    TitleEnd,
}

impl Seq {
    fn bytes(self) -> &'static [u8] {
        match self {
            Seq::Cdata => b"CDATA[",
            Seq::CdataEnd => b"]]>",
            Seq::CommentEnd => b"-->",
            Seq::ScriptEnd => b"</script",
            Seq::StyleEnd => b"</style",
            Seq::TitleEnd => b"</title",
        }
    }
}

const MANY_ATTRS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    NoValue,
    Unquoted,
    Single,
    Double,
}

fn is_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\n' | b'\t' | 0x0c | b'\r')
}

fn is_end_of_tag_section(c: u8) -> bool {
    c == b'/' || c == b'>' || is_whitespace(c)
}

/// `String.prototype.toLowerCase`.
fn js_lower(s: &str) -> String {
    if s.is_ascii() {
        s.to_ascii_lowercase()
    } else {
        s.to_lowercase()
    }
}

const P_TAG: &[&str] = &["p"];
const FORM_TAGS: &[&str] = &[
    "input", "option", "optgroup", "select", "button", "datalist", "textarea",
];
const TABLE_SECTION_TAGS: &[&str] = &["thead", "tbody"];
const DDT_TAGS: &[&str] = &["dd", "dt"];
const RTP_TAGS: &[&str] = &["rt", "rp"];

fn open_implies_close(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "tr" => &["tr", "th", "td"],
        "th" => &["th"],
        "td" => &["thead", "th", "td"],
        "body" => &["head", "link", "script"],
        "li" => &["li"],
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => P_TAG,
        "select" | "input" | "output" | "button" | "datalist" | "textarea" => FORM_TAGS,
        "option" => &["option"],
        "optgroup" => &["optgroup", "option"],
        "dd" | "dt" => DDT_TAGS,
        "address" | "article" | "aside" | "blockquote" | "details" | "div" | "dl" | "fieldset"
        | "figcaption" | "figure" | "footer" | "form" | "header" | "hr" | "main" | "nav" | "ol"
        | "pre" | "section" | "table" | "ul" => P_TAG,
        "rt" | "rp" => RTP_TAGS,
        "tbody" | "tfoot" => TABLE_SECTION_TAGS,
        _ => return None,
    })
}

pub(crate) fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "basefont"
            | "br"
            | "col"
            | "command"
            | "embed"
            | "frame"
            | "hr"
            | "img"
            | "input"
            | "isindex"
            | "keygen"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_foreign_context_element(name: &str) -> bool {
    matches!(name, "math" | "svg")
}

fn is_html_integration_element(name: &str) -> bool {
    matches!(
        name,
        "mi" | "mo" | "mn" | "ms" | "mtext" | "annotation-xml" | "foreignobject" | "desc" | "title"
    )
}

/// Парсит документ целиком, как `parseDocument(src, { decodeEntities: false })`.
pub fn parse_document(src: &str) -> Dom {
    let mut parser = Parser::new(src);
    parser.parse();
    parser.finish();
    parser.handler.dom
}

/// domhandler `DomHandler`.
struct Handler {
    dom: Dom,
    tag_stack: Vec<NodeId>,
    last_node: Option<NodeId>,
}

impl Handler {
    fn add_node(&mut self, data: NodeData) -> NodeId {
        // JS бросил бы исключение на пустом стеке; посреди документа парсер сюда не попадает.
        let parent = self.tag_stack.last().copied().unwrap_or(DOCUMENT);
        let id = self.dom.nodes.len();
        self.dom.nodes.push(Node {
            parent: Some(parent),
            children: Vec::new(),
            data,
        });
        self.dom.nodes[parent].children.push(id);
        self.last_node = None;
        id
    }

    fn onclosetag(&mut self) {
        self.last_node = None;
        self.tag_stack.pop();
    }

    fn onopentag(&mut self, name: String, attrs: Attrs) {
        let kind = ElementKind::for_name(&name);
        let id = self.add_node(NodeData::Element(Element { name, attrs, kind }));
        self.tag_stack.push(id);
    }

    fn ontext(&mut self, data: &str) {
        if let Some(last) = self.last_node
            && let NodeData::Text(text) = &mut self.dom.nodes[last].data
        {
            text.push_str(data);
            return;
        }
        let id = self.add_node(NodeData::Text(data.to_owned()));
        self.last_node = Some(id);
    }

    fn oncomment(&mut self, data: String) {
        if let Some(last) = self.last_node
            && let NodeData::Comment(comment) = &mut self.dom.nodes[last].data
        {
            comment.push_str(&data);
            return;
        }
        let id = self.add_node(NodeData::Comment(data));
        self.last_node = Some(id);
    }

    fn oncommentend(&mut self) {
        self.last_node = None;
    }

    fn onprocessinginstruction(&mut self, name: String, data: String) {
        self.add_node(NodeData::Directive { name, data });
    }
}

struct Parser<'a> {
    src: &'a str,
    buf: &'a [u8],

    state: State,
    section_start: isize,
    index: usize,
    is_special: bool,
    seq: Seq,
    seq_index: usize,

    tagname: String,
    attribname: String,
    attribvalue: String,
    attribs: Option<Attrs>,
    stack: Vec<String>,
    foreign_context: Vec<bool>,
    /// Имена атрибутов текущего тега; заполняется, когда их набирается `MANY_ATTRS`.
    attrib_seen: HashSet<String>,

    handler: Handler,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        let mut dom = Dom::new();
        dom.nodes.reserve(src.len() / 32);
        Parser {
            src,
            buf: src.as_bytes(),
            state: State::Text,
            section_start: 0,
            index: 0,
            is_special: false,
            seq: Seq::Cdata,
            seq_index: 0,
            tagname: String::new(),
            attribname: String::new(),
            attribvalue: String::new(),
            attribs: None,
            stack: Vec::new(),
            foreign_context: Vec::new(),
            attrib_seen: HashSet::new(),
            handler: Handler {
                dom,
                tag_stack: vec![DOCUMENT],
                last_node: None,
            },
        }
    }

    /// `buffer.slice(start, end)`; `start` бывает отрицательным только как -1 (устаревший
    /// `sectionStart`, дошедший до `handleTrailingData`), JS отсчитывает его от конца.
    fn slice(&self, start: isize, end: usize) -> &'a str {
        let src = self.src;
        if start < 0 {
            let Some(last) = src[..end].chars().next_back() else {
                return "";
            };
            // Астральный последний символ дал бы в JS одиночный суррогат.
            return if last.len_utf16() == 1 {
                &src[end - last.len_utf8()..end]
            } else {
                "\u{FFFD}"
            };
        }
        let start = start as usize;
        if end <= start {
            return "";
        }
        debug_assert!(src.is_char_boundary(start) && src.is_char_boundary(end));
        src.get(start..end).unwrap_or("")
    }

    fn fast_forward_to(&mut self, c: u8) -> bool {
        let from = self.index + 1;
        if from < self.buf.len()
            && let Some(pos) = memchr::memchr(c, &self.buf[from..])
        {
            self.index = from + pos;
            return true;
        }
        self.index = self.buf.len() - 1;
        false
    }

    fn parse(&mut self) {
        let len = self.buf.len();
        while self.index < len {
            let c = self.buf[self.index];
            match self.state {
                State::Text => self.state_text(c),
                State::SpecialStartSequence => self.state_special_start_sequence(c),
                State::InSpecialTag => self.state_in_special_tag(c),
                State::CdataSequence => self.state_cdata_sequence(c),
                State::InAttributeValueDq => self.handle_in_attribute_value(c, b'"'),
                State::InAttributeName => self.state_in_attribute_name(c),
                State::InCommentLike => self.state_in_comment_like(c),
                State::InSpecialComment => self.state_in_special_comment(c),
                State::BeforeAttributeName => self.state_before_attribute_name(c),
                State::InTagName => self.state_in_tag_name(c),
                State::InClosingTagName => self.state_in_closing_tag_name(c),
                State::BeforeTagName => self.state_before_tag_name(c),
                State::AfterAttributeName => self.state_after_attribute_name(c),
                State::InAttributeValueSq => self.handle_in_attribute_value(c, b'\''),
                State::BeforeAttributeValue => self.state_before_attribute_value(c),
                State::BeforeClosingTagName => self.state_before_closing_tag_name(c),
                State::AfterClosingTagName => self.state_after_closing_tag_name(c),
                State::BeforeSpecialS => self.state_before_special_s(c),
                State::InAttributeValueNq => self.state_in_attribute_value_no_quotes(c),
                State::InSelfClosingTag => self.state_in_self_closing_tag(c),
                State::InDeclaration => self.state_in_declaration(c),
                State::BeforeDeclaration => self.state_before_declaration(c),
                State::BeforeComment => self.state_before_comment(c),
                State::InProcessingInstruction => self.state_in_processing_instruction(c),
            }
            self.index += 1;
        }
        self.cleanup();
    }

    fn state_text(&mut self, c: u8) {
        if c == b'<' || self.fast_forward_to(b'<') {
            if self.index as isize > self.section_start {
                self.ontext(self.section_start, self.index);
            }
            self.state = State::BeforeTagName;
            self.section_start = self.index as isize;
        }
    }

    fn state_special_start_sequence(&mut self, c: u8) {
        let seq = self.seq.bytes();
        let is_end = self.seq_index == seq.len();
        let is_match = if is_end {
            is_end_of_tag_section(c)
        } else {
            (c | 0x20) == seq[self.seq_index]
        };
        if !is_match {
            self.is_special = false;
        } else if !is_end {
            self.seq_index += 1;
            return;
        }
        self.seq_index = 0;
        self.state = State::InTagName;
        self.state_in_tag_name(c);
    }

    fn state_in_special_tag(&mut self, c: u8) {
        let seq = self.seq.bytes();
        if self.seq_index == seq.len() {
            if c == b'>' || is_whitespace(c) {
                let end_of_text = self.index - seq.len();
                if self.section_start < end_of_text as isize {
                    self.ontext(self.section_start, end_of_text);
                }
                self.is_special = false;
                self.section_start = end_of_text as isize + 2;
                self.state_in_closing_tag_name(c);
                return;
            }
            self.seq_index = 0;
        }
        if (c | 0x20) == seq[self.seq_index] {
            self.seq_index += 1;
        } else if self.seq_index == 0 {
            if self.seq != Seq::TitleEnd && self.fast_forward_to(b'<') {
                self.seq_index = 1;
            }
        } else {
            self.seq_index = usize::from(c == b'<');
        }
    }

    fn state_cdata_sequence(&mut self, c: u8) {
        let seq = Seq::Cdata.bytes();
        if c == seq[self.seq_index] {
            self.seq_index += 1;
            if self.seq_index == seq.len() {
                self.state = State::InCommentLike;
                self.seq = Seq::CdataEnd;
                self.seq_index = 0;
                self.section_start = self.index as isize + 1;
            }
        } else {
            self.seq_index = 0;
            self.state = State::InDeclaration;
            self.state_in_declaration(c);
        }
    }

    fn state_in_comment_like(&mut self, c: u8) {
        let seq = self.seq.bytes();
        if c == seq[self.seq_index] {
            self.seq_index += 1;
            if self.seq_index == seq.len() {
                if self.seq == Seq::CdataEnd {
                    self.oncdata(self.section_start, self.index, 2);
                } else {
                    self.oncomment(self.section_start, self.index, 2);
                }
                self.seq_index = 0;
                self.section_start = self.index as isize + 1;
                self.state = State::Text;
            }
        } else if self.seq_index == 0 {
            if self.fast_forward_to(seq[0]) {
                self.seq_index = 1;
            }
        } else if c != seq[self.seq_index - 1] {
            self.seq_index = 0;
        }
    }

    fn start_special(&mut self, seq: Seq, offset: usize) {
        self.is_special = true;
        self.seq = seq;
        self.seq_index = offset;
        self.state = State::SpecialStartSequence;
    }

    fn state_before_tag_name(&mut self, c: u8) {
        if c == b'!' {
            self.state = State::BeforeDeclaration;
            self.section_start = self.index as isize + 1;
        } else if c == b'?' {
            self.state = State::InProcessingInstruction;
            self.section_start = self.index as isize + 1;
        } else if c.is_ascii_alphabetic() {
            let lower = c | 0x20;
            self.section_start = self.index as isize;
            if lower == b't' {
                self.start_special(Seq::TitleEnd, 3);
            } else {
                self.state = if lower == b's' {
                    State::BeforeSpecialS
                } else {
                    State::InTagName
                };
            }
        } else if c == b'/' {
            self.state = State::BeforeClosingTagName;
        } else {
            self.state = State::Text;
            self.state_text(c);
        }
    }

    fn state_in_tag_name(&mut self, c: u8) {
        if is_end_of_tag_section(c) {
            self.onopentagname(self.section_start, self.index);
            self.section_start = -1;
            self.state = State::BeforeAttributeName;
            self.state_before_attribute_name(c);
        }
    }

    fn state_before_closing_tag_name(&mut self, c: u8) {
        if is_whitespace(c) {
            // Игнорируем
        } else if c == b'>' {
            self.state = State::Text;
        } else {
            self.state = if c.is_ascii_alphabetic() {
                State::InClosingTagName
            } else {
                State::InSpecialComment
            };
            self.section_start = self.index as isize;
        }
    }

    fn state_in_closing_tag_name(&mut self, c: u8) {
        if c == b'>' || is_whitespace(c) {
            self.onclosetag(self.section_start, self.index);
            self.section_start = -1;
            self.state = State::AfterClosingTagName;
            self.state_after_closing_tag_name(c);
        }
    }

    fn state_after_closing_tag_name(&mut self, c: u8) {
        if c == b'>' || self.fast_forward_to(b'>') {
            self.state = State::Text;
            self.section_start = self.index as isize + 1;
        }
    }

    fn state_before_attribute_name(&mut self, c: u8) {
        if c == b'>' {
            self.onopentagend();
            if self.is_special {
                self.state = State::InSpecialTag;
                self.seq_index = 0;
            } else {
                self.state = State::Text;
            }
            self.section_start = self.index as isize + 1;
        } else if c == b'/' {
            self.state = State::InSelfClosingTag;
        } else if !is_whitespace(c) {
            self.state = State::InAttributeName;
            self.section_start = self.index as isize;
        }
    }

    fn state_in_self_closing_tag(&mut self, c: u8) {
        if c == b'>' {
            self.onselfclosingtag();
            self.state = State::Text;
            self.section_start = self.index as isize + 1;
            self.is_special = false;
        } else if !is_whitespace(c) {
            self.state = State::BeforeAttributeName;
            self.state_before_attribute_name(c);
        }
    }

    fn state_in_attribute_name(&mut self, c: u8) {
        if c == b'=' || is_end_of_tag_section(c) {
            self.onattribname(self.section_start, self.index);
            self.section_start = -1;
            self.state = State::AfterAttributeName;
            self.state_after_attribute_name(c);
        }
    }

    fn state_after_attribute_name(&mut self, c: u8) {
        if c == b'=' {
            self.state = State::BeforeAttributeValue;
        } else if c == b'/' || c == b'>' {
            self.onattribend(Quote::NoValue);
            self.state = State::BeforeAttributeName;
            self.state_before_attribute_name(c);
        } else if !is_whitespace(c) {
            self.onattribend(Quote::NoValue);
            self.state = State::InAttributeName;
            self.section_start = self.index as isize;
        }
    }

    fn state_before_attribute_value(&mut self, c: u8) {
        if c == b'"' {
            self.state = State::InAttributeValueDq;
            self.section_start = self.index as isize + 1;
        } else if c == b'\'' {
            self.state = State::InAttributeValueSq;
            self.section_start = self.index as isize + 1;
        } else if !is_whitespace(c) {
            self.section_start = self.index as isize;
            self.state = State::InAttributeValueNq;
            self.state_in_attribute_value_no_quotes(c);
        }
    }

    fn handle_in_attribute_value(&mut self, c: u8, quote: u8) {
        if c == quote || self.fast_forward_to(quote) {
            self.onattribdata(self.section_start, self.index);
            self.section_start = -1;
            self.onattribend(if quote == b'"' {
                Quote::Double
            } else {
                Quote::Single
            });
            self.state = State::BeforeAttributeName;
        }
    }

    fn state_in_attribute_value_no_quotes(&mut self, c: u8) {
        if is_whitespace(c) || c == b'>' {
            self.onattribdata(self.section_start, self.index);
            self.section_start = -1;
            self.onattribend(Quote::Unquoted);
            self.state = State::BeforeAttributeName;
            self.state_before_attribute_name(c);
        }
    }

    fn state_before_declaration(&mut self, c: u8) {
        if c == b'[' {
            self.state = State::CdataSequence;
            self.seq_index = 0;
        } else {
            self.state = if c == b'-' {
                State::BeforeComment
            } else {
                State::InDeclaration
            };
        }
    }

    fn state_in_declaration(&mut self, c: u8) {
        if c == b'>' || self.fast_forward_to(b'>') {
            self.ondeclaration(self.section_start, self.index);
            self.state = State::Text;
            self.section_start = self.index as isize + 1;
        }
    }

    fn state_in_processing_instruction(&mut self, c: u8) {
        if c == b'>' || self.fast_forward_to(b'>') {
            self.onprocessinginstruction(self.section_start, self.index);
            self.state = State::Text;
            self.section_start = self.index as isize + 1;
        }
    }

    fn state_before_comment(&mut self, c: u8) {
        if c == b'-' {
            self.state = State::InCommentLike;
            self.seq = Seq::CommentEnd;
            // Разрешаем короткие комментарии (напр. <!-->)
            self.seq_index = 2;
            self.section_start = self.index as isize + 1;
        } else {
            self.state = State::InDeclaration;
        }
    }

    fn state_in_special_comment(&mut self, c: u8) {
        if c == b'>' || self.fast_forward_to(b'>') {
            self.oncomment(self.section_start, self.index, 0);
            self.state = State::Text;
            self.section_start = self.index as isize + 1;
        }
    }

    fn state_before_special_s(&mut self, c: u8) {
        let lower = c | 0x20;
        if lower == b'c' {
            self.start_special(Seq::ScriptEnd, 4);
        } else if lower == b't' {
            self.start_special(Seq::StyleEnd, 4);
        } else {
            self.state = State::InTagName;
            self.state_in_tag_name(c);
        }
    }

    fn cleanup(&mut self) {
        if self.section_start != self.index as isize {
            if self.state == State::Text
                || (self.state == State::InSpecialTag && self.seq_index == 0)
            {
                self.ontext(self.section_start, self.index);
                self.section_start = self.index as isize;
            } else if matches!(
                self.state,
                State::InAttributeValueDq | State::InAttributeValueSq | State::InAttributeValueNq
            ) {
                self.onattribdata(self.section_start, self.index);
                self.section_start = self.index as isize;
            }
        }
    }

    fn finish(&mut self) {
        if self.section_start < self.index as isize {
            self.handle_trailing_data();
        }
        self.onend();
    }

    fn handle_trailing_data(&mut self) {
        let end = self.buf.len();
        match self.state {
            State::InCommentLike => {
                if self.seq == Seq::CdataEnd {
                    self.oncdata(self.section_start, end, 0);
                } else {
                    self.oncomment(self.section_start, end, 0);
                }
            }
            State::InTagName
            | State::BeforeAttributeName
            | State::BeforeAttributeValue
            | State::AfterAttributeName
            | State::InAttributeName
            | State::InAttributeValueSq
            | State::InAttributeValueDq
            | State::InAttributeValueNq
            | State::InClosingTagName => {
                // Незавершённый тег отбрасывается.
            }
            _ => self.ontext(self.section_start, end),
        }
    }

    fn ontext(&mut self, start: isize, end: usize) {
        let data = self.slice(start, end);
        self.handler.ontext(data);
    }

    fn onopentagname(&mut self, start: isize, end: usize) {
        let name = js_lower(self.slice(start, end));
        self.emit_open_tag(name);
    }

    fn emit_open_tag(&mut self, name: String) {
        if let Some(implies_close) = open_implies_close(&name) {
            while let Some(top) = self.stack.last() {
                if !implies_close.contains(&top.as_str()) {
                    break;
                }
                self.stack.pop();
                self.handler.onclosetag();
            }
        }
        if !is_void_element(&name) {
            if is_foreign_context_element(&name) {
                self.foreign_context.push(true);
            } else if is_html_integration_element(&name) {
                self.foreign_context.push(false);
            }
            self.stack.push(name.clone());
        }
        self.tagname = name;
        self.attribs = Some(Attrs::new());
        self.attrib_seen.clear();
    }

    fn end_open_tag(&mut self) {
        let name = std::mem::take(&mut self.tagname);
        let is_void = is_void_element(&name);
        if let Some(attribs) = self.attribs.take() {
            self.handler.onopentag(name, attribs);
        }
        if is_void {
            self.handler.onclosetag();
        }
    }

    fn onopentagend(&mut self) {
        self.end_open_tag();
    }

    fn onclosetag(&mut self, start: isize, end: usize) {
        let name = js_lower(self.slice(start, end));
        if is_foreign_context_element(&name) || is_html_integration_element(&name) {
            self.foreign_context.pop();
        }
        if !is_void_element(&name) {
            if let Some(pos) = self.stack.iter().rposition(|open| *open == name) {
                for _ in pos..self.stack.len() {
                    self.handler.onclosetag();
                }
                self.stack.truncate(pos);
            } else if name == "p" {
                // Неявное открытие перед закрытием
                self.emit_open_tag(name);
                self.close_current_tag();
            }
        } else if name == "br" {
            self.handler.onopentag(name, Attrs::new());
            self.handler.onclosetag();
        }
    }

    fn onselfclosingtag(&mut self) {
        if self.foreign_context.last() == Some(&true) {
            self.close_current_tag();
        } else {
            self.onopentagend();
        }
    }

    fn close_current_tag(&mut self) {
        let is_top = self.stack.last().is_some_and(|top| *top == self.tagname);
        self.end_open_tag();
        // Самозакрывающиеся теги будут на вершине стека
        if is_top {
            self.handler.onclosetag();
            self.stack.pop();
        }
    }

    fn onattribname(&mut self, start: isize, end: usize) {
        self.attribname = js_lower(self.slice(start, end));
    }

    fn onattribdata(&mut self, start: isize, end: usize) {
        let data = self.slice(start, end);
        self.attribvalue.push_str(data);
    }

    fn onattribend(&mut self, _quote: Quote) {
        let value = std::mem::take(&mut self.attribvalue);
        let Some(attribs) = &mut self.attribs else {
            return;
        };
        // `attribs.__proto__ = "…"` — no-op на plain object.
        if self.attribname == "__proto__" {
            return;
        }
        // `insert_if_absent` сканирует линейно; при большом числе атрибутов держим set.
        if attribs.len() < MANY_ATTRS {
            attribs.insert_if_absent(self.attribname.clone(), value);
            return;
        }
        if self.attrib_seen.is_empty() {
            self.attrib_seen
                .extend(attribs.iter().map(|(key, _)| key.to_owned()));
        }
        if self.attrib_seen.insert(self.attribname.clone()) {
            attribs.insert_if_absent(self.attribname.clone(), value);
        }
    }

    fn instruction_name(value: &str) -> String {
        let end = value
            .char_indices()
            .find(|&(_, c)| c == '/' || dom::js::is_whitespace(c))
            .map_or(value.len(), |(i, _)| i);
        js_lower(&value[..end])
    }

    fn ondeclaration(&mut self, start: isize, end: usize) {
        let value = self.slice(start, end);
        let name = Self::instruction_name(value);
        self.handler
            .onprocessinginstruction(format!("!{name}"), format!("!{value}"));
    }

    fn onprocessinginstruction(&mut self, start: isize, end: usize) {
        let value = self.slice(start, end);
        let name = Self::instruction_name(value);
        self.handler
            .onprocessinginstruction(format!("?{name}"), format!("?{value}"));
    }

    fn oncomment(&mut self, start: isize, end: usize, offset: usize) {
        let data = self.slice(start, end.saturating_sub(offset)).to_owned();
        self.handler.oncomment(data);
        self.handler.oncommentend();
    }

    fn oncdata(&mut self, start: isize, end: usize, offset: usize) {
        let value = self.slice(start, end.saturating_sub(offset));
        self.handler.oncomment(format!("[CDATA[{value}]]"));
        self.handler.oncommentend();
    }

    fn onend(&mut self) {
        for _ in 0..self.stack.len() {
            self.handler.onclosetag();
        }
    }
}
