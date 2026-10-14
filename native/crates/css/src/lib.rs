//! Порт CSS-части critters 0.0.23 `processStyle` вместе с
//! частями postcss 8.5.8, на которые она опирается.
//!
//! Вывод байт-в-байт совпадает с JS для набора опций, который использует Next.js
//! (`keyframes: 'critical'`, без `allowRules`, без `pruneSource`, fonts выключены).

mod list;
mod parser;
mod process;
mod stringify;
mod tokenize;

pub use list::comma;
pub use process::transform_selector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedStyle {
    /// `serializeStylesheet(ast, { compress: true })`. critters удаляет
    /// элемент `<style>`, когда `css.trim()` пуст (если не `skipped`).
    pub css: String,
    /// Записи `failedSelectors` из critters: `${sel} -> ${e.message}`.
    pub failed_selectors: Vec<String>,
    /// processStyle вышел досрочно (пустая таблица): элемент не трогаем.
    pub skipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// postcss бросает `CssSyntaxError`.
    Syntax,
    /// Бросает сам critters (напр. `@font-face;` без тела).
    Runtime,
    /// Не ошибка JS: вход, который порт не воспроизводит (напр. аннотация
    /// source map, которую postcss попытался бы загрузить). Фолбэк на JS.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssError {
    pub message: String,
    pub kind: ErrorKind,
}

impl CssError {
    pub(crate) fn new(kind: ErrorKind, message: String) -> Self {
        CssError { message, kind }
    }
}

impl std::fmt::Display for CssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CssError {}

/// Опция `keyframes` из critters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keyframes {
    #[default]
    Critical,
    All,
    None,
}

/// critters `processStyle` с опциями Next.js.
pub fn process_style(
    sheet: &str,
    exists: &mut dyn FnMut(&str) -> Result<bool, String>,
) -> Result<ProcessedStyle, CssError> {
    process_style_with(sheet, Keyframes::Critical, exists)
}

pub fn process_style_with(
    sheet: &str,
    keyframes: Keyframes,
    exists: &mut dyn FnMut(&str) -> Result<bool, String>,
) -> Result<ProcessedStyle, CssError> {
    if sheet.is_empty() {
        return Ok(ProcessedStyle {
            css: String::new(),
            failed_selectors: Vec::new(),
            skipped: true,
        });
    }
    // postcss Input: срезать ведущий BOM.
    let css = sheet
        .strip_prefix('\u{FEFF}')
        .or_else(|| sheet.strip_prefix('\u{FFFE}'))
        .unwrap_or(sheet);
    if has_source_map_annotation(css) {
        return Err(CssError::new(
            ErrorKind::Unsupported,
            "source map annotation".to_string(),
        ));
    }
    let mut ast = parser::parse(css)?;
    let mut ctx = process::Ctx::new(exists, keyframes);
    process::walk1(&mut ast, &mut ctx, None);
    process::walk2(&mut ast, &ctx, None)?;
    let css = stringify::serialize(&ast)?;
    Ok(ProcessedStyle {
        css,
        failed_selectors: ctx.failed,
        skipped: false,
    })
}

/// postcss `PreviousMap#loadAnnotation`: непустая аннотация заставляет postcss
/// читать/декодировать source map (ФС / data URI), это не портируем.
fn has_source_map_annotation(css: &str) -> bool {
    const NEEDLE: &str = "# sourceMappingURL=";
    let b = css.as_bytes();
    let mut last: Option<(usize, usize)> = None; // (начало, конец префикса)
    let mut i = 0;
    while let Some(p) = tokenize::find_sub(b, b"/*", i) {
        let rest = &css[p + 2..];
        let ws = rest.len() - dom::js::trim_start(rest).len();
        if rest[ws..].starts_with(NEEDLE) {
            last = Some((p, p + 2 + ws + NEEDLE.len()));
        }
        i = p + 1;
    }
    let Some((start, pre)) = last else {
        return false;
    };
    let Some(end) = tokenize::find_sub(b, b"*/", start) else {
        return false;
    };
    if end < pre {
        // Само совпадение содержит "*/" — невозможно ("/*" + ws + "#...").
        return true;
    }
    !dom::js::trim(&css[pre..end]).is_empty()
}

#[cfg(test)]
mod tests;
