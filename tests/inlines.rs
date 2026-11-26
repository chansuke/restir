//! Inline markup recognition, through a one-paragraph document.

mod common;

use common::{inlines_outline, parse_checked};
use restir::*;

fn inl(src: &str) -> String {
    let doc = parse_checked(src);
    match doc.blocks.as_slice() {
        [b] => match &b.node {
            Block::Paragraph(i) => inlines_outline(i),
            other => panic!("expected a paragraph, got {other:?}"),
        },
        other => panic!("expected one block, got {other:?}"),
    }
}

#[test]
fn every_construct() {
    assert_eq!(
        inl("*em* **strong** ``lit`` :code:`x` `cite` `l <http://u>`_ `l`_ `a`__ `b <u>`__"),
        r#"em"em" " " strong"strong" " " lit"lit" " " role:code"x" " " cite"cite" " " ref"l"->"http://u" " " ref"l" " " anon"a" " " anon"b"->"u""#
    );
}

#[test]
fn markup_spans_cover_delimiters() {
    let src = "a *b* c";
    let doc = parse_checked(src);
    let Block::Paragraph(i) = &doc.blocks[0].node else {
        panic!()
    };
    assert_eq!(i[1].span, Span::new(2, 5));
    assert_eq!(i[1].node, Inline::Emphasis("b"));
}

#[test]
fn not_markup() {
    for src in [
        "2*3*4", "**bold*", "*a", "`x", "``y", "x * a*", "*a *", "\"*\"", "(*)", "a*b*", "*a*b",
        "**a**b", ":r:`x`_", "\\*a*", "「*」",
    ] {
        assert_eq!(inl(src), format!("{src:?}"), "{src}");
    }
}

#[test]
fn surrounding_punctuation() {
    assert_eq!(inl("(*a*)"), r#""(" em"a" ")""#);
    assert_eq!(inl("'*a*'"), r#""'" em"a" "'""#);
    assert_eq!(inl("-*a*-"), r#""-" em"a" "-""#);
    assert_eq!(inl("*a*."), r#"em"a" ".""#);
}

#[test]
fn end_string_search_skips_invalid_candidates() {
    assert_eq!(inl("*a*b*"), r#"em"a*b""#);
    assert_eq!(inl("``a`b``"), r#"lit"a`b""#);
}

#[test]
fn unclosed_markup_resumes_after_start_string() {
    assert_eq!(inl("`a *b*"), r#""`a " em"b""#);
    assert_eq!(inl("**a *b*"), r#""**a " em"b""#);
}

#[test]
fn escapes() {
    assert_eq!(inl("*a\\*b*"), r#"em"a\\*b""#);
    assert_eq!(inl("`a\\`b`"), r#"cite"a\\`b""#);
    // No escapes inside literals: the backslash is content.
    assert_eq!(inl("``a\\``"), r#"lit"a\\""#);
    assert_eq!(inl("x\\ *y*\\ z"), r#""x\\ " em"y" "\\ z""#);
}

#[test]
fn roles() {
    assert_eq!(inl(":py:func:`f`"), r#"role:py:func"f""#);
    assert_eq!(inl(":not role:`x`"), r#"":not role:" cite"x""#);
    assert_eq!(inl(":a::b:`x`"), r#"":a:" role:b"x""#);
    assert_eq!(inl(":-a:`x`"), r#"":-a:" cite"x""#);
}

#[test]
fn role_spans() {
    let src = "see :code:`x y`.";
    let doc = parse_checked(src);
    let Block::Paragraph(i) = &doc.blocks[0].node else {
        panic!()
    };
    let Inline::Role { name, content } = &i[1].node else {
        panic!()
    };
    assert_eq!(name.span, Span::new(5, 9));
    assert_eq!(content.span, Span::new(11, 14));
    assert_eq!(i[1].span, Span::new(4, 15));
}

#[test]
fn reference_label_and_target() {
    assert_eq!(inl("`a  <u>`_"), r#"ref"a"->"u""#);
    assert_eq!(inl("`label\n<url>`_"), r#"ref"label"->"url""#);
    assert_eq!(inl("`a <b> c <d>`_"), r#"ref"a <b> c"->"d""#);
    assert_eq!(inl("`<u>`_"), r#"ref"<u>""#);
    assert_eq!(inl("`a <>`_"), r#"ref"a <>""#);
    assert_eq!(inl("`a<u>`_"), r#"ref"a<u>""#);
}

#[test]
fn multi_line_markup() {
    assert_eq!(inl("*a\nb*"), r#"em"a\nb""#);
}

#[test]
fn japanese_markup() {
    assert_eq!(
        inl("これは\\ *強調*\\ です。「*強調*」と「*」。"),
        r#""これは\\ " em"強調" "\\ です。「" em"強調" "」と「*」。""#
    );
    assert_eq!(inl("（*a*）、"), r#""（" em"a" "）、""#);
    // Without escaped spaces, CJK letters around markup block recognition.
    assert_eq!(inl("日本語の*強調*です"), r#""日本語の*強調*です""#);
}

#[test]
fn long_unclosed_markup_is_linear() {
    let src = "*a `b ``c :r:`d ".repeat(5000);
    let doc = parse_checked(&src);
    assert_eq!(doc.blocks.len(), 1);
}
