//! Zero-copy AST. Every `&'a str` is a subslice of the parsed source, and
//! every node carries the byte range it was parsed from.

/// Byte range `start..end` into the source text, with `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Offset of the first byte.
    pub start: u32,
    /// Offset one past the last byte.
    pub end: u32,
}

impl Span {
    /// Creates a span. `start` must not be greater than `end`.
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Span { start, end }
    }

    /// The span as a `usize` range, for slicing the source.
    pub fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }

    /// Length in bytes.
    pub fn len(self) -> u32 {
        self.end - self.start
    }

    /// Whether the span covers no bytes.
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A node with the span of source it was parsed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    /// The node.
    pub node: T,
    /// Where the node is in the source.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Pairs a node with its span.
    pub fn new(node: T, span: Span) -> Self {
        Spanned { node, span }
    }
}

/// A run of inline nodes.
pub type Inlines<'a> = Vec<Spanned<Inline<'a>>>;
/// A sequence of block nodes.
pub type Blocks<'a> = Vec<Spanned<Block<'a>>>;

/// A parsed document, returned by [`parse`](crate::parse).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Document<'a> {
    /// The text that was parsed. All spans are offsets into it.
    pub source: &'a str,
    /// The top-level blocks, in source order.
    pub blocks: Blocks<'a>,
}

/// A block-level element.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block<'a> {
    /// A section title. Sections are not nested; `level` gives the depth.
    Heading {
        /// Starts at 1 and follows the order in which title styles first
        /// appear in the document.
        level: u8,
        /// The adornment that marks this title.
        style: HeadingStyle,
        /// The title text.
        title: Inlines<'a>,
    },
    /// A paragraph. A trailing `::` is removed or reduced to `:`.
    Paragraph(Inlines<'a>),
    /// A list with `-`, `*`, or `+` markers.
    BulletList {
        /// The marker that all items share.
        marker: BulletMarker,
        /// The items, in order.
        items: Vec<Spanned<ListItem<'a>>>,
    },
    /// A list with `1.` or `#.` markers.
    #[non_exhaustive]
    EnumeratedList {
        /// The first ordinal, or `None` for auto-numbered (`#.`) lists.
        start: Option<u32>,
        /// The items, in order.
        items: Vec<Spanned<ListItem<'a>>>,
    },
    /// The raw lines of a literal block after `::`. The slice starts at
    /// column 0 of the first line, so it keeps its indentation.
    LiteralBlock(&'a str),
    /// An indented block of nested elements.
    BlockQuote(Blocks<'a>),
    /// An explicit `.. name::` directive.
    Directive(Directive<'a>),
    /// A horizontal rule, written as a line of four or more punctuation
    /// characters.
    Transition,
    /// The raw text after `..` in a comment, or in an explicit markup block
    /// that restir does not support.
    Comment(&'a str),
}

/// How a section title is adorned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeadingStyle {
    /// The ASCII punctuation character of the underline, such as `=`.
    pub adornment: char,
    /// Whether the title also has an overline.
    pub overline: bool,
}

/// The marker of a bullet list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BulletMarker {
    /// `-`
    Hyphen,
    /// `*`
    Asterisk,
    /// `+`
    Plus,
}

/// One item of a bullet or enumerated list.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListItem<'a> {
    /// The item's content.
    pub blocks: Blocks<'a>,
}

/// A directive, kept generic. Its content is not interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Directive<'a> {
    /// The name before `::`.
    pub name: Spanned<&'a str>,
    /// Text after `name::` plus continuation lines up to the first blank or
    /// option line. The first line is always arguments, even for directives
    /// such as `note` that docutils would treat as taking content there.
    pub arguments: Option<Spanned<&'a str>>,
    /// The `:name: value` lines after the arguments.
    pub options: Vec<Spanned<DirectiveOption<'a>>>,
    /// Raw content block, starting at column 0 of its first line. Parse it
    /// with [`Directive::parse_body`] when the directive's content is reST.
    pub body: Option<Spanned<&'a str>>,
}

/// One `:name: value` option of a directive.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DirectiveOption<'a> {
    /// The name between the colons.
    pub name: Spanned<&'a str>,
    /// Raw value; may span several indented lines.
    pub value: Option<Spanned<&'a str>>,
}

/// An inline element.
///
/// Inline markup in reST does not nest, so marked-up content is a plain
/// slice. Backslash escapes stay in the slice; renderers resolve them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Inline<'a> {
    /// Plain text.
    Text(&'a str),
    /// `*text*`
    Emphasis(&'a str),
    /// `**text**`
    Strong(&'a str),
    /// ``` ``text`` ```
    Literal(&'a str),
    /// `` :name:`content` ``
    Role {
        /// The role name.
        name: Spanned<&'a str>,
        /// The text between the backquotes.
        content: Spanned<&'a str>,
    },
    /// `` `text` `` with no explicit role.
    Interpreted(&'a str),
    /// `` `label <url>`_ `` or `` `label`_ ``; trailing `__` marks anonymous.
    Reference {
        /// The link text.
        label: Spanned<&'a str>,
        /// The text between `<` and `>`, when given.
        target: Option<Spanned<&'a str>>,
        /// Whether the reference ends with `__`.
        anonymous: bool,
    },
}
