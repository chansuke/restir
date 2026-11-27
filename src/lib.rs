#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod ast;
mod block;
mod html;
mod inline;
#[cfg(kani)]
mod proofs;

pub use ast::*;

/// The source is 4 GiB or larger, so it does not fit in `u32` byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputTooLarge;

impl std::fmt::Display for InputTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("source exceeds u32::MAX bytes")
    }
}

impl std::error::Error for InputTooLarge {}

/// Parses `source`, or returns [`InputTooLarge`] if it is 4 GiB or larger.
/// Malformed markup degrades to text and never fails.
///
/// ```
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let doc = restir::try_parse("Hello, *world*.")?;
///     assert_eq!(doc.blocks.len(), 1);
///     Ok(())
/// }
/// ```
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

/// Renders `doc` as an HTML fragment, with one element per line.
pub fn render_html(doc: &Document<'_>) -> String {
    html::render(doc)
}

impl<'a> Directive<'a> {
    /// Parses the body as nested reST. Spans stay absolute to the original
    /// source because the body is itself a slice of it. Section titles and
    /// transitions are not recognized inside a body.
    pub fn parse_body(&self) -> Blocks<'a> {
        match &self.body {
            Some(body) => block::parse_nested(body.node, body.span.start),
            None => Vec::new(),
        }
    }
}
