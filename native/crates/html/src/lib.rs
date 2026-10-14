//! htmlparser2 8.0.2 `parseDocument(html, { decodeEntities: false })` и
//! dom-serializer 2.0.0 `render(node, { decodeEntities: false })`, байт-в-байт.

mod parser;
mod serializer;

pub use parser::parse_document;
pub use serializer::render;

#[cfg(test)]
mod tests;
