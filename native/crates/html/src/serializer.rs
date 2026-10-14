//! Порт dom-serializer 2.0.0 `render(node, { decodeEntities: false })`.
//!
//! dom-serializer переименовывает элементы на месте при рендере foreign-контента
//! (`elem.name = elementNames.get(elem.name) ?? elem.name`). Порт принимает DOM
//! по разделяемой ссылке и применяет то же отображение только к выводу; проверка
//! integration point у родителя берёт отображённое имя, как его видит JS.

use dom::{DOCUMENT, Dom, NodeData, NodeId};

use crate::parser::is_void_element;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Html,
    Foreign,
}

fn element_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "altglyph" => "altGlyph",
        "altglyphdef" => "altGlyphDef",
        "altglyphitem" => "altGlyphItem",
        "animatecolor" => "animateColor",
        "animatemotion" => "animateMotion",
        "animatetransform" => "animateTransform",
        "clippath" => "clipPath",
        "feblend" => "feBlend",
        "fecolormatrix" => "feColorMatrix",
        "fecomponenttransfer" => "feComponentTransfer",
        "fecomposite" => "feComposite",
        "feconvolvematrix" => "feConvolveMatrix",
        "fediffuselighting" => "feDiffuseLighting",
        "fedisplacementmap" => "feDisplacementMap",
        "fedistantlight" => "feDistantLight",
        "fedropshadow" => "feDropShadow",
        "feflood" => "feFlood",
        "fefunca" => "feFuncA",
        "fefuncb" => "feFuncB",
        "fefuncg" => "feFuncG",
        "fefuncr" => "feFuncR",
        "fegaussianblur" => "feGaussianBlur",
        "feimage" => "feImage",
        "femerge" => "feMerge",
        "femergenode" => "feMergeNode",
        "femorphology" => "feMorphology",
        "feoffset" => "feOffset",
        "fepointlight" => "fePointLight",
        "fespecularlighting" => "feSpecularLighting",
        "fespotlight" => "feSpotLight",
        "fetile" => "feTile",
        "feturbulence" => "feTurbulence",
        "foreignobject" => "foreignObject",
        "glyphref" => "glyphRef",
        "lineargradient" => "linearGradient",
        "radialgradient" => "radialGradient",
        "textpath" => "textPath",
        _ => return None,
    })
}

fn attribute_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "definitionurl" => "definitionURL",
        "attributename" => "attributeName",
        "attributetype" => "attributeType",
        "basefrequency" => "baseFrequency",
        "baseprofile" => "baseProfile",
        "calcmode" => "calcMode",
        "clippathunits" => "clipPathUnits",
        "diffuseconstant" => "diffuseConstant",
        "edgemode" => "edgeMode",
        "filterunits" => "filterUnits",
        "glyphref" => "glyphRef",
        "gradienttransform" => "gradientTransform",
        "gradientunits" => "gradientUnits",
        "kernelmatrix" => "kernelMatrix",
        "kernelunitlength" => "kernelUnitLength",
        "keypoints" => "keyPoints",
        "keysplines" => "keySplines",
        "keytimes" => "keyTimes",
        "lengthadjust" => "lengthAdjust",
        "limitingconeangle" => "limitingConeAngle",
        "markerheight" => "markerHeight",
        "markerunits" => "markerUnits",
        "markerwidth" => "markerWidth",
        "maskcontentunits" => "maskContentUnits",
        "maskunits" => "maskUnits",
        "numoctaves" => "numOctaves",
        "pathlength" => "pathLength",
        "patterncontentunits" => "patternContentUnits",
        "patterntransform" => "patternTransform",
        "patternunits" => "patternUnits",
        "pointsatx" => "pointsAtX",
        "pointsaty" => "pointsAtY",
        "pointsatz" => "pointsAtZ",
        "preservealpha" => "preserveAlpha",
        "preserveaspectratio" => "preserveAspectRatio",
        "primitiveunits" => "primitiveUnits",
        "refx" => "refX",
        "refy" => "refY",
        "repeatcount" => "repeatCount",
        "repeatdur" => "repeatDur",
        "requiredextensions" => "requiredExtensions",
        "requiredfeatures" => "requiredFeatures",
        "specularconstant" => "specularConstant",
        "specularexponent" => "specularExponent",
        "spreadmethod" => "spreadMethod",
        "startoffset" => "startOffset",
        "stddeviation" => "stdDeviation",
        "stitchtiles" => "stitchTiles",
        "surfacescale" => "surfaceScale",
        "systemlanguage" => "systemLanguage",
        "tablevalues" => "tableValues",
        "targetx" => "targetX",
        "targety" => "targetY",
        "textlength" => "textLength",
        "viewbox" => "viewBox",
        "viewtarget" => "viewTarget",
        "xchannelselector" => "xChannelSelector",
        "ychannelselector" => "yChannelSelector",
        "zoomandpan" => "zoomAndPan",
        _ => return None,
    })
}

fn is_foreign_mode_integration_point(name: &str) -> bool {
    matches!(
        name,
        "mi" | "mo" | "mn" | "ms" | "mtext" | "annotation-xml" | "foreignObject" | "desc" | "title"
    )
}

/// `render(node, { decodeEntities: false })`; `render(dom, DOCUMENT)` рендерит документ.
pub fn render(dom: &Dom, node: NodeId) -> String {
    let mut out = String::with_capacity(estimate_len(dom, node));
    Renderer { dom, out: &mut out }.render(node);
    out
}

fn estimate_len(dom: &Dom, node: NodeId) -> usize {
    if node == DOCUMENT {
        dom.nodes.len() * 24
    } else {
        256
    }
}

struct Frame<'a> {
    /// Элемент, чьи дети сейчас рендерятся.
    node: NodeId,
    /// Его имя в выводе (после отображения foreign-режима) — для закрывающего тега
    /// и для проверки integration point у его детей.
    name: &'a str,
    /// Его дети рендерятся в этом режиме.
    mode: Mode,
    next_child: usize,
    /// Нужен ли закрывающий тег после детей.
    close: bool,
}

struct Renderer<'a, 'o> {
    dom: &'a Dom,
    out: &'o mut String,
}

impl<'a> Renderer<'a, '_> {
    fn render(&mut self, root: NodeId) {
        let dom = self.dom;
        let mut stack: Vec<Frame<'a>> = Vec::with_capacity(64);
        if let Some(frame) = self.open(root, Mode::Html, None) {
            stack.push(frame);
        }
        while let Some(top) = stack.last_mut() {
            let children = &dom.nodes[top.node].children;
            if top.next_child < children.len() {
                let child = children[top.next_child];
                top.next_child += 1;
                let (mode, parent_name) = (top.mode, top.name);
                if let Some(frame) = self.open(child, mode, Some(parent_name)) {
                    stack.push(frame);
                }
            } else {
                let frame = stack.pop().expect("non-empty");
                if frame.close {
                    self.out.push_str("</");
                    self.out.push_str(frame.name);
                    self.out.push('>');
                }
            }
        }
    }

    /// Рендерит `node` до его детей; возвращает фрейм, если дальше
    /// идут дети (и, возможно, закрывающий тег).
    fn open(&mut self, node: NodeId, mode: Mode, parent_name: Option<&str>) -> Option<Frame<'a>> {
        let dom = self.dom;
        let out = &mut *self.out;
        match &dom.nodes[node].data {
            NodeData::Document => Some(Frame {
                node,
                name: "",
                mode,
                next_child: 0,
                close: false,
            }),
            NodeData::Directive { data, .. } => {
                out.push('<');
                out.push_str(data);
                out.push('>');
                None
            }
            NodeData::Comment(data) => {
                out.push_str("<!--");
                out.push_str(data);
                out.push_str("-->");
                None
            }
            NodeData::CData => {
                out.push_str("<![CDATA[");
                if let Some(&first) = dom.nodes[node].children.first()
                    && let NodeData::Text(text) = &dom.nodes[first].data
                {
                    out.push_str(text);
                }
                out.push_str("]]>");
                None
            }
            NodeData::Text(data) => {
                out.push_str(data);
                None
            }
            NodeData::Element(element) => {
                let mut mode = mode;
                let mut name: &'a str = element.name.as_str();
                if mode == Mode::Foreign {
                    if let Some(mapped) = element_name(name) {
                        name = mapped;
                    }
                    if parent_name.is_some_and(is_foreign_mode_integration_point) {
                        mode = Mode::Html;
                    }
                }
                if mode == Mode::Html && (name == "svg" || name == "math") {
                    mode = Mode::Foreign;
                }
                out.push('<');
                out.push_str(name);
                // `tag += " " + attribs` только если склеенные атрибуты непусты.
                let attrs_start = out.len();
                for (key, value) in element.attrs.iter() {
                    out.push(' ');
                    let key = match mode {
                        Mode::Foreign => attribute_name(key).unwrap_or(key),
                        Mode::Html => key,
                    };
                    out.push_str(key);
                    if mode == Mode::Html && value.is_empty() {
                        continue;
                    }
                    out.push_str("=\"");
                    push_replacing_quotes(out, value);
                    out.push('"');
                }
                if out.len() == attrs_start + 1 {
                    out.truncate(attrs_start);
                }
                let has_children = !dom.nodes[node].children.is_empty();
                if !has_children && mode == Mode::Foreign {
                    out.push_str("/>");
                    return None;
                }
                out.push('>');
                let close = mode == Mode::Foreign || !is_void_element(name);
                if has_children {
                    Some(Frame {
                        node,
                        name,
                        mode,
                        next_child: 0,
                        close,
                    })
                } else {
                    if close {
                        out.push_str("</");
                        out.push_str(name);
                        out.push('>');
                    }
                    None
                }
            }
        }
    }
}

/// `value.replace(/"/g, "&quot;")`.
fn push_replacing_quotes(out: &mut String, value: &str) {
    let mut rest = value;
    while let Some(pos) = memchr::memchr(b'"', rest.as_bytes()) {
        out.push_str(&rest[..pos]);
        out.push_str("&quot;");
        rest = &rest[pos + 1..];
    }
    out.push_str(rest);
}
