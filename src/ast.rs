//! Zero-copy AST. Every `&'a str` is a subslice of the parsed source, and
//! every node carries the byte range it was parsed from.

/// Byte range `start..end` into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Span { start, end }
    }

    pub fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Spanned { node, span }
    }
}

pub type Inlines<'a> = Vec<Spanned<Inline<'a>>>;
pub type Blocks<'a> = Vec<Spanned<Block<'a>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<'a> {
    pub source: &'a str,
    pub blocks: Blocks<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block<'a> {
    /// `level` starts at 1 and follows the order in which adornment styles
    /// first appear in the document.
    Heading {
        level: u8,
        style: HeadingStyle,
        title: Inlines<'a>,
    },
    Paragraph(Inlines<'a>),
    BulletList {
        marker: BulletMarker,
        items: Vec<Spanned<ListItem<'a>>>,
    },
    EnumeratedList {
        /// `None` for auto-numbered (`#.`) lists.
        start: Option<u32>,
        items: Vec<Spanned<ListItem<'a>>>,
    },
    /// Raw source lines, starting at column 0 of the first line, so they
    /// keep their indentation. Renderers dedent them.
    LiteralBlock(&'a str),
    BlockQuote(Blocks<'a>),
    Directive(Directive<'a>),
    Transition,
    /// Comment text after `..`, raw.
    Comment(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeadingStyle {
    pub adornment: u8,
    pub overline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BulletMarker {
    Hyphen,
    Asterisk,
    Plus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem<'a> {
    pub blocks: Blocks<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive<'a> {
    pub name: Spanned<&'a str>,
    /// Text after `name::` plus continuation lines up to the first blank or
    /// option line. The first line is always arguments, even for directives
    /// such as `note` that docutils would treat as taking content there.
    pub arguments: Option<Spanned<&'a str>>,
    pub options: Vec<Spanned<DirectiveOption<'a>>>,
    /// Raw content block, starting at column 0 of its first line. Parse it
    /// with [`Directive::parse_body`] when the directive's content is reST.
    pub body: Option<Spanned<&'a str>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveOption<'a> {
    pub name: Spanned<&'a str>,
    /// Raw value; may span several indented lines.
    pub value: Option<Spanned<&'a str>>,
}

/// Inline markup in reST does not nest, so marked-up content is a plain
/// slice. Escapes (`\*`) stay in the slice; renderers resolve them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline<'a> {
    Text(&'a str),
    Emphasis(&'a str),
    Strong(&'a str),
    Literal(&'a str),
    /// `` :name:`content` ``
    Role {
        name: Spanned<&'a str>,
        content: Spanned<&'a str>,
    },
    /// `` `text` `` with no explicit role.
    Interpreted(&'a str),
    /// `` `label <url>`_ `` or `` `label`_ ``; trailing `__` marks anonymous.
    Reference {
        label: Spanned<&'a str>,
        target: Option<Spanned<&'a str>>,
        anonymous: bool,
    },
}
