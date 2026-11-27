//! Shared test support: the span invariant walker, a compact outline of a
//! parsed tree for expected values, and the corpus every invariant runs on.
#![allow(dead_code)]

use restir::*;

/// Parses `src`, checks every span invariant, and returns the document.
pub fn parse_checked(src: &str) -> Document<'_> {
    let doc = parse(src);
    check_document(&doc);
    doc
}

pub fn outline(src: &str) -> Vec<String> {
    let doc = parse_checked(src);
    doc.blocks.iter().map(|b| block_outline(&b.node)).collect()
}

pub fn html(src: &str) -> String {
    render_html(&parse_checked(src))
}

// ---------------------------------------------------------------- outline

fn block_outline(b: &Block<'_>) -> String {
    match b {
        Block::Heading { level, title, .. } => format!("H{level}[{}]", inlines_outline(title)),
        Block::Paragraph(i) => format!("P[{}]", inlines_outline(i)),
        Block::BulletList { marker, items } => {
            let m = match marker {
                BulletMarker::Hyphen => '-',
                BulletMarker::Asterisk => '*',
                BulletMarker::Plus => '+',
                other => panic!("unhandled variant {other:?}"),
            };
            format!("UL{m}({})", items_outline(items))
        }
        Block::EnumeratedList { start, items, .. } => {
            let s = start.map_or("#".to_string(), |n| n.to_string());
            format!("OL{s}({})", items_outline(items))
        }
        Block::LiteralBlock(s) => format!("LIT{s:?}"),
        Block::BlockQuote(b) => format!("Q({})", blocks_outline(b)),
        Block::Directive(d) => {
            let args = d.arguments.as_ref().map(|a| a.node);
            let opts: Vec<String> = d
                .options
                .iter()
                .map(|o| {
                    format!(
                        "{}={:?}",
                        o.node.name.node,
                        o.node.value.as_ref().map(|v| v.node)
                    )
                })
                .collect();
            let body = d.body.as_ref().map(|b| b.node);
            format!("D{}[{args:?}]{{{}}}({body:?})", d.name.node, opts.join(","))
        }
        Block::Transition => "HR".to_string(),
        Block::Comment(s) => format!("C{s:?}"),
        other => panic!("unhandled variant {other:?}"),
    }
}

pub fn blocks_outline(blocks: &[Spanned<Block<'_>>]) -> String {
    blocks
        .iter()
        .map(|b| block_outline(&b.node))
        .collect::<Vec<_>>()
        .join(", ")
}

fn items_outline(items: &[Spanned<ListItem<'_>>]) -> String {
    items
        .iter()
        .map(|i| blocks_outline(&i.node.blocks))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn inlines_outline(inlines: &[Spanned<Inline<'_>>]) -> String {
    inlines
        .iter()
        .map(|i| match &i.node {
            Inline::Text(t) => format!("{t:?}"),
            Inline::Emphasis(t) => format!("em{t:?}"),
            Inline::Strong(t) => format!("strong{t:?}"),
            Inline::Literal(t) => format!("lit{t:?}"),
            Inline::Interpreted(t) => format!("cite{t:?}"),
            Inline::Role { name, content } => format!("role:{}{:?}", name.node, content.node),
            Inline::Reference {
                label,
                target,
                anonymous,
            } => {
                let kind = if *anonymous { "anon" } else { "ref" };
                match target {
                    Some(t) => format!("{kind}{:?}->{:?}", label.node, t.node),
                    None => format!("{kind}{:?}", label.node),
                }
            }
            other => panic!("unhandled variant {other:?}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------- span checker

/// Byte offset of `s` in `src`, asserting that `s` is a subslice of it.
fn offset_in(src: &str, s: &str) -> usize {
    let base = src.as_ptr() as usize;
    let p = s.as_ptr() as usize;
    assert!(
        p >= base && p + s.len() <= base + src.len(),
        "slice {s:?} is not inside the source"
    );
    p - base
}

fn check_slice(src: &str, s: &str, span: Span) {
    assert_eq!(
        offset_in(src, s),
        span.start as usize,
        "slice {s:?} starts at its span"
    );
    assert_eq!(
        s.len(),
        span.len() as usize,
        "slice {s:?} has its span's length"
    );
    assert_eq!(&src[span.range()], s);
}

fn check_spanned_slice(src: &str, s: &Spanned<&str>) {
    check_slice(src, s.node, s.span);
}

fn assert_within(inner: Span, outer: Span, what: &str) {
    assert!(
        outer.start <= inner.start && inner.end <= outer.end,
        "{what} {inner:?} is not within {outer:?}"
    );
}

/// Siblings are in order and do not overlap, and all sit inside `parent`.
fn check_siblings(spans: impl IntoIterator<Item = Span>, parent: Span, what: &str) {
    let mut prev_end = parent.start;
    for span in spans {
        assert_within(span, parent, what);
        assert!(
            span.start >= prev_end,
            "{what} {span:?} overlaps its previous sibling"
        );
        prev_end = span.end;
    }
}

fn is_line_start(src: &str, at: usize) -> bool {
    at == 0 || src.as_bytes()[at - 1] == b'\n'
}

pub fn check_document(doc: &Document<'_>) {
    let whole = Span::new(0, doc.source.len() as u32);
    check_blocks(doc.source, &doc.blocks, whole);
}

fn check_blocks(src: &str, blocks: &[Spanned<Block<'_>>], parent: Span) {
    check_siblings(blocks.iter().map(|b| b.span), parent, "block");
    for b in blocks {
        check_block(src, b);
    }
}

fn check_block(src: &str, b: &Spanned<Block<'_>>) {
    let text = &src[b.span.range()];
    assert!(!text.is_empty(), "block {:?} has an empty span", b.node);
    assert!(
        !text.ends_with(char::is_whitespace),
        "block span {text:?} ends with whitespace"
    );
    if !matches!(b.node, Block::LiteralBlock(_)) {
        assert!(
            !text.starts_with(char::is_whitespace),
            "block span {text:?} starts with whitespace"
        );
    }
    match &b.node {
        Block::Heading { title, .. } => {
            assert!(!title.is_empty(), "heading has a title");
            check_inlines(src, title, b.span);
        }
        Block::Paragraph(inlines) => {
            assert!(!inlines.is_empty(), "paragraph has inlines");
            assert_eq!(
                inlines[0].span.start, b.span.start,
                "paragraph inlines start with it"
            );
            check_inlines(src, inlines, b.span);
        }
        Block::BulletList { marker, items } => {
            let want = match marker {
                BulletMarker::Hyphen => b'-',
                BulletMarker::Asterisk => b'*',
                BulletMarker::Plus => b'+',
                other => panic!("unhandled variant {other:?}"),
            };
            check_items(src, items, b.span);
            for item in items {
                assert_eq!(src.as_bytes()[item.span.start as usize], want);
            }
        }
        Block::EnumeratedList { items, .. } => check_items(src, items, b.span),
        Block::LiteralBlock(s) => {
            check_slice(src, s, b.span);
            assert!(
                is_line_start(src, b.span.start as usize),
                "literal block starts at column 0"
            );
        }
        Block::BlockQuote(children) => check_blocks(src, children, b.span),
        Block::Directive(d) => check_directive(src, d, b.span),
        Block::Transition => {
            let c = text.as_bytes()[0];
            assert!(
                text.len() >= 4 && text.bytes().all(|x| x == c),
                "transition {text:?}"
            );
        }
        Block::Comment(s) => {
            assert!(
                text.starts_with(".."),
                "comment span {text:?} starts with .."
            );
            let at = offset_in(src, s);
            assert_within(
                Span::new(at as u32, (at + s.len()) as u32),
                b.span,
                "comment text",
            );
        }
        other => panic!("unhandled variant {other:?}"),
    }
}

fn check_items(src: &str, items: &[Spanned<ListItem<'_>>], parent: Span) {
    assert!(!items.is_empty(), "list has items");
    assert_eq!(
        items[0].span.start, parent.start,
        "list starts with its first item"
    );
    assert_eq!(
        items.last().unwrap().span.end,
        parent.end,
        "list ends with its last item"
    );
    check_siblings(items.iter().map(|i| i.span), parent, "list item");
    for item in items {
        let text = &src[item.span.range()];
        assert!(
            !text.ends_with(char::is_whitespace),
            "item {text:?} ends with whitespace"
        );
        check_blocks(src, &item.node.blocks, item.span);
    }
}

fn check_directive(src: &str, d: &Directive<'_>, span: Span) {
    assert!(
        src[span.range()].starts_with(".."),
        "directive starts with .."
    );
    check_spanned_slice(src, &d.name);
    assert!(!d.name.node.is_empty());
    let mut children = vec![d.name.span];
    if let Some(a) = &d.arguments {
        check_spanned_slice(src, a);
        children.push(a.span);
    }
    for o in &d.options {
        check_spanned_slice(src, &o.node.name);
        assert_eq!(
            o.node.name.span.start,
            o.span.start + 1,
            "option name follows its colon"
        );
        let mut parts = vec![o.node.name.span];
        if let Some(v) = &o.node.value {
            check_spanned_slice(src, v);
            parts.push(v.span);
        }
        check_siblings(parts, o.span, "option part");
        children.push(o.span);
    }
    if let Some(body) = &d.body {
        check_spanned_slice(src, body);
        assert!(
            is_line_start(src, body.span.start as usize),
            "body starts at column 0"
        );
        children.push(body.span);
        let nested = d.parse_body();
        check_blocks(src, &nested, body.span);
    } else {
        assert!(d.parse_body().is_empty());
    }
    check_siblings(children, span, "directive part");
}

fn check_inlines(src: &str, inlines: &[Spanned<Inline<'_>>], parent: Span) {
    // Text fills the gaps between markup, so inlines are contiguous.
    for pair in inlines.windows(2) {
        assert_eq!(
            pair[0].span.end, pair[1].span.start,
            "inlines are contiguous"
        );
        assert!(
            !(matches!(pair[0].node, Inline::Text(_)) && matches!(pair[1].node, Inline::Text(_))),
            "adjacent Text runs are merged"
        );
    }
    check_siblings(inlines.iter().map(|i| i.span), parent, "inline");
    for i in inlines {
        check_inline(src, i);
    }
}

fn check_wrapped(src: &str, span: Span, inner: &str, open: &str, close: &str) {
    let full = &src[span.range()];
    assert!(!inner.is_empty(), "markup {full:?} has content");
    assert_eq!(
        offset_in(src, inner),
        span.start as usize + open.len(),
        "content of {full:?}"
    );
    assert_eq!(full, format!("{open}{inner}{close}"));
}

fn check_inline(src: &str, i: &Spanned<Inline<'_>>) {
    match &i.node {
        Inline::Text(t) => check_slice(src, t, i.span),
        Inline::Emphasis(t) => check_wrapped(src, i.span, t, "*", "*"),
        Inline::Strong(t) => check_wrapped(src, i.span, t, "**", "**"),
        Inline::Literal(t) => check_wrapped(src, i.span, t, "``", "``"),
        Inline::Interpreted(t) => check_wrapped(src, i.span, t, "`", "`"),
        Inline::Role { name, content } => {
            check_spanned_slice(src, name);
            check_spanned_slice(src, content);
            assert!(!content.node.is_empty());
            let full = &src[i.span.range()];
            assert_eq!(full, format!(":{}:`{}`", name.node, content.node));
            assert_eq!(name.span.start, i.span.start + 1);
            assert_eq!(content.span.end + 1, i.span.end);
        }
        Inline::Reference {
            label,
            target,
            anonymous,
        } => {
            check_spanned_slice(src, label);
            assert!(!label.node.is_empty());
            let full = &src[i.span.range()];
            let suffix = if *anonymous { "`__" } else { "`_" };
            assert!(
                full.starts_with('`') && full.ends_with(suffix),
                "reference {full:?}"
            );
            assert_eq!(label.span.start, i.span.start + 1);
            let mut parts = vec![label.span];
            if let Some(t) = target {
                check_spanned_slice(src, t);
                assert!(!t.node.is_empty());
                assert_eq!(&src[t.span.start as usize - 1..t.span.start as usize], "<");
                assert_eq!(t.span.end as usize + 1 + suffix.len(), i.span.end as usize);
                parts.push(t.span);
            } else {
                assert_eq!(label.span.end as usize + suffix.len(), i.span.end as usize);
            }
            check_siblings(parts, i.span, "reference part");
        }
        other => panic!("unhandled variant {other:?}"),
    }
}

// ----------------------------------------------------------------- corpus

/// Documents that exercise every construct and its edges. The span
/// invariants and the robustness test run over all of them.
pub const CORPUS: &[&str] = &[
    "",
    "\n\n",
    "Plain paragraph.",
    "=====\nTitle\n=====\n\nSub\n===\n\nOther\n-----\n\nSub2\n====\n",
    "Title\n===\n\nshort underline\n===",
    "line one\nline two\n========\n",
    "para\n\n-----\n\npara\n\n---\n",
    "=====\nTitle\n-----\n",
    "  ======\n  Inset\n  ======\n",
    "======\n  Inset title\n======\n\ntext",
    "Para::\n\n    code\n\n    more\n\nafter",
    "Para ::\n\n  code\n",
    "::\n\n  code\n::\n",
    "x::\n  a\nb",
    "para\n  quote\nback\n",
    "    four\n  two\nzero\n",
    "- a\n- b\n\n  b2\n* c\n+ d\n-\n  e\n",
    "- item\n    deeper\n  back\n- next\n   odd\n",
    "1. one\n2. two\n\n#. auto\n#. auto2\n\n3. x\n5. y\n",
    "2026. was a year\nand more\n",
    "9. a\n   continued\n10. b\n",
    ".. code-block:: rust\n   :linenos:\n   :caption: Example\n      continued\n\n   fn main() {}\n     indented\n",
    ".. note::\n\n   Hello *world*.\n\n   - a\n   - b\n\n   .. warning:: inner\n\n      deep\n",
    ".. figure:: a.png\n   more args\n   :alt: x\n   not an option\n\n   caption\n",
    ".. a comment\n   continues\n\n..\n\n   quoted\n\n..\n   text\n.. _target: http://x\n.. [1] note\n.. |s| image:: x\n.. foo bar:: x\n.. -x:: y\n",
    "*em* **strong** ``lit`` :code:`x` `cite` `l <http://u>`_ `l`_ `a`__ `b <u>`__",
    "2*3*4 **bold* *a `x ``y * a* *a * \"*\" (*) a*b* *a*b \\*a* *a\\*b* ``a\\``",
    "(*a*) *a*b* :not role:`x` :a::b:`x` x\\ *y*\\ z",
    "これは\\ *強調*\\ です。「*強調*」と「*」。\n\n見出し\n======\n\n日本語タイトル\n===\n",
    "Title\r\n=====\r\n\r\nPara one\r\nline two\r\n\r\n- a\r\n  b\r\n\r\n.. note:: x\r\n\r\n   body\r\n",
    "\tquoted\n\n::\n\n\tcode\n\t  more\n\n-\tx\n\ty\n",
    "*a\nb* `label\n  <url>`_ trailing\\",
    "- - - nested\n  - two\n\n> not quote\n\n  .. code:: py\n\n    x = 1\n",
    "Title\n=====\nparagraph right after\n=====\n",
    "``\n..\n:\n*\n#.\n1.\n- \n",
];
