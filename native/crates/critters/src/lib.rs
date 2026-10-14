//! `process()` из critters 0.0.23 для опций, которые передаёт Next.js:
//! `{ ssrMode: true, reduceInlineStyles: false, path, publicPath, preload: 'media', fonts: false }`.
//! Всё, что вне этого пути, возвращается как fallback, чтобы JS-реализация
//! сама выдала результат (или исключение).

use std::collections::HashSet;

use dom::{DOCUMENT, Dom, NodeId, js};

mod files;
mod format;
mod matcher;

pub use files::FileCache;

use format::{format_size, percent};
use matcher::{ClassCache, Matcher};

pub struct Options<'a> {
    /// Абсолютный нормализованный каталог результата сборки (`renderOpts.distDir`).
    pub path: &'a str,
    pub public_path: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Log {
    pub level: Level,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Html {
        html: String,
        logs: Vec<Log>,
    },
    /// Документ должна обработать JS-реализация; причина — для диагностики.
    Fallback(String),
}

macro_rules! fallback {
    ($($arg:tt)*) => {
        return Outcome::Fallback(format!($($arg)*))
    };
}

/// `<style>`, созданный critters для `<link>` (`$$external`), с его `$$name`.
struct External {
    style: NodeId,
    name: String,
}

pub fn process(html: &str, options: &Options, files: &FileCache) -> Outcome {
    if !options.path.starts_with('/') || normalize_path(options.path) != options.path {
        fallback!("options.path is not an absolute normalized path");
    }

    let mut dom = html::parse_document(html);
    let mut logs = Vec::new();

    // createDocument(): контейнер — первый [data-critters-container], иначе <html>.
    let container = match select_one(&dom, "[data-critters-container]", DOCUMENT) {
        Ok(Some(element)) => element,
        Ok(None) => {
            let Some(root) = dom
                .children(DOCUMENT)
                .iter()
                .copied()
                .find(|&child| dom.name(child) == Some("html"))
            else {
                fallback!("document has no <html> element");
            };
            dom.set_attr(root, "data-critters-container", "");
            root
        }
        Err(message) => fallback!("container query failed: {message}"),
    };
    let class_cache = ClassCache::build(&dom, container);

    let links = match select_all(&dom, "link[rel=\"stylesheet\"]", DOCUMENT) {
        Ok(links) => links,
        Err(message) => fallback!("link query failed: {message}"),
    };
    let mut externals: Vec<External> = Vec::new();
    for link in links {
        match embed_linked_stylesheet(&mut dom, link, options, files, &mut logs) {
            Ok(Some(external)) => externals.push(external),
            Ok(None) => {}
            Err(reason) => return Outcome::Fallback(reason),
        }
    }

    // getAffectedStyleTags(): все <style>, созданные critters, в порядке документа.
    let styles = affected_styles(&dom, &externals);

    let mut matcher = Matcher::new(&dom, container, class_cache);
    for &(style, ref name) in &styles {
        if let Err(reason) = process_style(&mut dom, style, name, &mut matcher, &mut logs) {
            return Outcome::Fallback(reason);
        }
    }

    if !styles.is_empty() {
        let remaining = affected_styles(&dom, &externals);
        match remaining.split_first() {
            None => logs.push(Log {
                level: Level::Warn,
                message: "Merging inline stylesheets into a single <style> tag skipped, no inline stylesheets to merge".to_owned(),
            }),
            Some((&(first, _), rest)) => {
                let mut sheet = dom.text_content(first);
                for &(node, _) in rest {
                    sheet.push_str(&dom.text_content(node));
                    dom.remove(node);
                }
                dom.set_text_content(first, &sheet);
            }
        }
    }

    Outcome::Html {
        html: html::render(&dom, DOCUMENT),
        logs,
    }
}

fn select_one(dom: &Dom, selector: &str, context: NodeId) -> Result<Option<NodeId>, String> {
    let query = select::compile(dom, selector, context)?;
    Ok(select::select_one(dom, &query, context))
}

fn select_all(dom: &Dom, selector: &str, context: NodeId) -> Result<Vec<NodeId>, String> {
    let query = select::compile(dom, selector, context)?;
    Ok(select::select_all(dom, &query, context))
}

fn affected_styles(dom: &Dom, externals: &[External]) -> Vec<(NodeId, String)> {
    let created: HashSet<NodeId> = externals.iter().map(|external| external.style).collect();
    let styles = select_all(dom, "style", DOCUMENT).unwrap_or_default();
    styles
        .into_iter()
        .filter(|style| created.contains(style))
        .map(|style| {
            let name = externals
                .iter()
                .find(|external| external.style == style)
                .map(|external| external.name.clone())
                .unwrap_or_default();
            (style, name)
        })
        .collect()
}

/// `embedLinkedStylesheet(link, document)` with `preload: 'media'`.
fn embed_linked_stylesheet(
    dom: &mut Dom,
    link: NodeId,
    options: &Options,
    files: &FileCache,
    logs: &mut Vec<Log>,
) -> Result<Option<External>, String> {
    let href = dom.attr(link, "href").map(str::to_owned);
    // validateMediaQuery() (postcss-media-query-parser) не портирован: голый media
    // type там валиден и остаётся как есть; всё остальное уходит в JS.
    let media = match dom.attr(link, "media") {
        None | Some("") => "all".to_owned(),
        Some(media @ ("all" | "print" | "screen" | "speech")) => media.to_owned(),
        Some(other) => return Err(format!("stylesheet link media needs validation: {other}")),
    };

    let Some(href) = href.filter(|href| href.ends_with(".css")) else {
        return Ok(None);
    };

    let Some(filename) = css_asset_path(&href, options)? else {
        return Ok(None);
    };
    let sheet = match files.read(&filename) {
        Ok(sheet) => sheet,
        Err(_) => {
            logs.push(Log {
                level: Level::Warn,
                message: format!("Unable to locate stylesheet: {filename}"),
            });
            return Ok(None);
        }
    };
    if sheet.is_empty() {
        return Ok(None);
    }

    let style = dom.create_element("style");
    dom.set_text_content(style, &sheet);
    // `link.parentNode.insertBefore(style, link)`: здесь у link всегда есть родитель.
    if dom.parent(link).is_none() {
        return Err("stylesheet link has no parent".to_owned());
    }
    dom.insert_before(link, style);

    dom.set_attr(link, "media", "print");
    dom.set_attr(link, "onload", &format!("this.media='{media}'"));

    if !href.contains("</noscript>") {
        let noscript = dom.create_element("noscript");
        let noscript_link = dom.clone_shallow(link);
        dom.remove_attr(noscript_link, "id");
        dom.append_child(noscript, noscript_link);
        // `insertBefore(noscript, link.nextSibling)`, при его отсутствии — append.
        match dom.next_sibling(link) {
            Some(next) => dom.insert_before(next, noscript),
            None => {
                let parent = dom.parent(link).expect("checked above");
                dom.append_child(parent, noscript);
            }
        }
    }

    Ok(Some(External { style, name: href }))
}

/// `getCssAsset(href)`: файл на диске; `None` для удалённых стилей и стилей вне каталога сборки.
/// Пути, требующие нормализации `path.resolve`, оставлены JS.
fn css_asset_path(href: &str, options: &Options) -> Result<Option<String>, String> {
    let mut normalized = href.strip_prefix('/').unwrap_or(href);
    let public = options.public_path;
    let public = public.strip_prefix('/').unwrap_or(public);
    let public = public.strip_suffix('/').unwrap_or(public);
    let prefix = format!("{public}/");
    if let Some(rest) = normalized.strip_prefix(prefix.as_str()) {
        normalized = rest.strip_prefix('/').unwrap_or(rest);
    }

    if normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || href.starts_with("//")
    {
        return Ok(None);
    }

    let filename = format!("{}/{}", options.path, normalized);
    if normalized.is_empty() || normalize_path(&filename) != filename || filename.contains('\\') {
        return Err(format!("stylesheet path needs path.resolve: {href}"));
    }
    Ok(Some(filename))
}

/// Схлопывает сегменты `//`, `.` и `..`; для уже нормализованного пути совпадает со входом.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

/// `processStyle(style, document)` для внешнего стиля.
fn process_style(
    dom: &mut Dom,
    style: NodeId,
    name: &str,
    matcher: &mut Matcher,
    logs: &mut Vec<Log>,
) -> Result<(), String> {
    let name = name.strip_prefix('/').unwrap_or(name);
    let before = dom.text_content(style);
    if before.is_empty() {
        return Ok(());
    }

    let processed = {
        let dom_ref: &Dom = dom;
        let mut exists = |selector: &str| matcher.exists(dom_ref, selector);
        css::process_style(&before, &mut exists)
            .map_err(|error| format!("css {:?}: {}", error.kind, error.message))?
    };
    if processed.skipped {
        return Ok(());
    }
    // Ниже DOM меняется; зависевшие от него ответы переиспользовать нельзя.
    matcher.dom_changed();

    if !processed.failed_selectors.is_empty() {
        logs.push(Log {
            level: Level::Warn,
            message: format!(
                "{} rules skipped due to selector errors:\n  {}",
                processed.failed_selectors.len(),
                processed.failed_selectors.join("\n  ")
            ),
        });
    }

    let sheet = processed.css;
    if js::trim(&sheet).is_empty() {
        if dom.parent(style).is_some() {
            dom.remove(style);
            matcher.style_removed();
        }
        return Ok(());
    }

    dom.set_text_content(style, &sheet);
    let sheet_len = js::utf16_len(&sheet);
    let before_len = js::utf16_len(&before);
    logs.push(Log {
        level: Level::Info,
        message: format!(
            "\u{1b}[32mInlined {} ({}% of original {}) of {}.\u{1b}[39m",
            format_size(sheet_len),
            percent(sheet_len, before_len),
            format_size(before_len),
            name
        ),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dom::NodeData;

    #[test]
    fn resolves_next_asset_paths() {
        let options = Options {
            path: "/app/.next",
            public_path: "/_next/",
        };
        assert_eq!(
            css_asset_path("/_next/static/css/a.css", &options),
            Ok(Some("/app/.next/static/css/a.css".to_owned()))
        );
        assert_eq!(css_asset_path("https://cdn/x.css", &options), Ok(None));
        assert_eq!(css_asset_path("//cdn/x.css", &options), Ok(None));
        assert!(css_asset_path("/_next/../../etc/x.css", &options).is_err());
        let prefixed = Options {
            path: "/app/.next",
            public_path: "https://cdn.example/_next/",
        };
        assert!(css_asset_path("/_next/static/a.css", &prefixed).is_ok());
    }

    #[test]
    fn data_nodes_are_untouched() {
        let mut dom = Dom::new();
        let text = dom.create(NodeData::Text("x".into()));
        dom.append_child(DOCUMENT, text);
        assert_eq!(dom.text_content(DOCUMENT), "");
    }
}
