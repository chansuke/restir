#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod ast;
mod block;
mod html;
mod inline;
#[cfg(kani)]
mod proofs;

pub use ast::*;

/// The source does not fit in `u32` byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputTooLarge;

pub fn try_parse(source: &str) -> Result<Document<'_>, InputTooLarge> {
    if source.len() > u32::MAX as usize {
        return Err(InputTooLarge);
    }
    Ok(Document {
        source,
        blocks: block::parse_document(source),
    })
}

/// Parses `source`. Malformed markup degrades to text and never panics.
///
/// # Panics
///
/// If `source` is 4 GiB or larger; use [`try_parse`] for untrusted sizes.
pub fn parse(source: &str) -> Document<'_> {
    try_parse(source).expect("restir: source exceeds u32::MAX bytes")
}

pub fn render_html(doc: &Document<'_>) -> String {
    html::render(doc)
}

impl<'a> Directive<'a> {
    /// Parses the body as nested reST. Spans stay absolute to the original
    /// source because the body is itself a slice of it.
    pub fn parse_body(&self) -> Blocks<'a> {
        match &self.body {
            Some(body) => block::parse_nested(body.node, body.span.start),
            None => Vec::new(),
        }
    }
}
