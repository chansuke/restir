//! Block constructs, driven through `restir::parse`. Expected outlines come
//! from the reST rules: `H` heading, `P` paragraph, `UL`/`OL` lists with
//! items separated by `;`, `Q` block quote, `LIT` literal block, `D`
//! directive, `C` comment, `HR` transition.

mod common;

use common::{blocks_outline, outline, parse_checked};
use restir::*;

fn out(src: &str) -> Vec<String> {
    outline(src)
}

#[test]
fn empty_input() {
    assert!(out("").is_empty());
    assert!(out("\n\n  \n").is_empty());
}

#[test]
fn paragraphs_are_separated_by_blank_lines() {
    assert_eq!(out("a\nb\n\nc"), [r#"P["a\nb"]"#, r#"P["c"]"#]);
}

#[test]
fn heading_levels_follow_first_appearance() {
    let src = "=====\nTitle\n=====\n\nSub\n===\n\nOther\n-----\n\nSub2\n====\n";
    assert_eq!(
        out(src),
        [
            r#"H1["Title"]"#,
            r#"H2["Sub"]"#,
            r#"H3["Other"]"#,
            r#"H2["Sub2"]"#
        ]
    );
    let doc = parse_checked(src);
    let styles: Vec<_> = doc
        .blocks
        .iter()
        .map(|b| match b.node {
            Block::Heading { style, .. } => (style.adornment, style.overline),
            _ => panic!("expected headings"),
        })
        .collect();
    assert_eq!(
        styles,
        [('=', true), ('=', false), ('-', false), ('=', false)]
    );
}

#[test]
fn heading_spans_cover_adornments() {
    let src = "Title\n=====\n\ntext\n\n===\nTop\n===\n";
    let doc = parse_checked(src);
    assert_eq!(doc.blocks[0].span, Span::new(0, 11));
    assert_eq!(doc.blocks[1].span, Span::new(13, 17));
    assert_eq!(&src[doc.blocks[2].span.range()], "===\nTop\n===");
}

#[test]
fn short_underline_is_paragraph() {
    assert_eq!(out("Title\n===\n"), [r#"P["Title\n==="]"#]);
    // Char count, not bytes: three CJK chars need only three adornment chars.
    assert_eq!(out("見出し\n===\n"), [r#"H1["見出し"]"#]);
    assert_eq!(
        out("日本語タイトル\n===\n"),
        [r#"P["日本語タイトル\n==="]"#]
    );
}

#[test]
fn title_detection_only_on_first_two_lines() {
    assert_eq!(
        out("line one\nline two\n========\n"),
        [r#"P["line one\nline two\n========"]"#]
    );
    assert_eq!(
        out("Title\n=====\nparagraph right after\n=====\n"),
        [r#"H1["Title"]"#, r#"P["paragraph right after\n====="]"#]
    );
}

#[test]
fn overline_title_may_be_inset_and_shorter() {
    assert_eq!(
        out("======\n  Inset title\n======\n\ntext"),
        [r#"H1["Inset title"]"#, r#"P["text"]"#]
    );
}

#[test]
fn overline_without_matching_underline() {
    assert_eq!(
        out("=====\nTitle\n-----\n"),
        [r#"P["=====\nTitle\n-----"]"#]
    );
    assert_eq!(out("=====\nTitle\n"), [r#"P["=====\nTitle"]"#]);
}

#[test]
fn headings_only_at_top_level() {
    assert_eq!(
        out("  ======\n  Inset\n  ======\n"),
        [r#"Q(P["======\n  Inset\n  ======"])"#]
    );
    assert_eq!(out("- Title\n  =====\n"), [r#"UL-(P["Title\n  ====="])"#]);
    assert_eq!(out("  ----\n"), [r#"Q(P["----"])"#]);
}

#[test]
fn transitions() {
    assert_eq!(
        out("para\n\n-----\n\npara"),
        [r#"P["para"]"#, "HR", r#"P["para"]"#]
    );
    assert_eq!(out("----\n\ntext"), ["HR", r#"P["text"]"#]);
    assert_eq!(out("text\n\n****"), [r#"P["text"]"#, "HR"]);
    // Too short.
    assert_eq!(out("a\n\n---\n"), [r#"P["a"]"#, r#"P["---"]"#]);
}

#[test]
fn literal_marker_variants() {
    assert_eq!(
        out("Para::\n\n    code\n\n    more\n\nafter"),
        [
            r#"P["Para:"]"#,
            r#"LIT"    code\n\n    more""#,
            r#"P["after"]"#
        ]
    );
    assert_eq!(
        out("Para ::\n\n  code\n"),
        [r#"P["Para"]"#, r#"LIT"  code""#]
    );
    assert_eq!(out("::\n\n  code\n"), [r#"LIT"  code""#]);
}

#[test]
fn literal_marker_without_block() {
    assert_eq!(out("Para::"), [r#"P["Para:"]"#]);
    assert_eq!(out("Para ::\n\nnext"), [r#"P["Para"]"#, r#"P["next"]"#]);
    assert!(out("::").is_empty());
    // The lone marker produces nothing, so its quote is empty.
    assert_eq!(out("x\n  ::"), [r#"P["x"]"#, "Q()"]);
}

#[test]
fn literal_block_extent() {
    // No blank line before, and ended by a dedent without a blank line.
    assert_eq!(
        out("x::\n  a\nb"),
        [r#"P["x:"]"#, r#"LIT"  a""#, r#"P["b"]"#]
    );
    let src = "x::\n\n   a\n\n  b\n\n\ny";
    let doc = parse_checked(src);
    assert_eq!(&src[doc.blocks[1].span.range()], "   a\n\n  b");
    assert_eq!(doc.blocks[1].span.start, 5);
}

#[test]
fn block_quote_without_blank_lines() {
    assert_eq!(
        out("para\n  quote\nback\n"),
        [r#"P["para"]"#, r#"Q(P["quote"])"#, r#"P["back"]"#]
    );
}

#[test]
fn block_quote_dedent_below_its_base() {
    assert_eq!(
        out("    four\n  two\nzero\n"),
        [r#"Q(Q(P["four"]), P["two"])"#, r#"P["zero"]"#]
    );
}

#[test]
fn bullet_lists_split_on_marker_change() {
    assert_eq!(
        out("- a\n- b\n\n  b2\n* c\n+ d\n"),
        [
            r#"UL-(P["a"]; P["b"], P["b2"])"#,
            r#"UL*(P["c"])"#,
            r#"UL+(P["d"])"#
        ]
    );
}

#[test]
fn bullet_item_with_empty_first_line() {
    assert_eq!(out("-\n  e\n"), [r#"UL-(P["e"])"#]);
    assert_eq!(out("-\n"), ["UL-()"]);
}

#[test]
fn list_continuation_at_odd_indent() {
    // The item body is indented to the first line's text column (docutils
    // `get_first_known_indented`). Deeper lines form a block quote here
    // (docutils makes a definition list, which is out of scope).
    assert_eq!(
        out("- item\n    deeper\n  back\n- next\n   odd\n"),
        [r#"UL-(P["item"], Q(P["deeper"]), P["back"]; P["next"], Q(P["odd"]))"#]
    );
    // A line shallower than the text column but deeper than the marker ends
    // the list and starts a block quote, as docutils does.
    assert_eq!(out("-   a\n  b\n"), [r#"UL-(P["a"])"#, r#"Q(P["b"])"#]);
    assert_eq!(out("1.  a\n   b\n"), [r#"OL1(P["a"])"#, r#"Q(P["b"])"#]);
}

#[test]
fn list_ends_at_unindented_text() {
    assert_eq!(out("- a\nb\n"), [r#"UL-(P["a"])"#, r#"P["b"]"#]);
}

#[test]
fn nested_lists() {
    assert_eq!(
        out("- - a\n  - b\n- c"),
        [r#"UL-(UL-(P["a"]; P["b"]); P["c"])"#]
    );
}

#[test]
fn bullet_list_spans() {
    let src = "- a\n\n  b\n- c\n";
    let doc = parse_checked(src);
    let Block::BulletList { items, .. } = &doc.blocks[0].node else {
        panic!("expected list");
    };
    assert_eq!(doc.blocks[0].span, Span::new(0, 12));
    assert_eq!(items[0].span, Span::new(0, 8));
    assert_eq!(items[1].span, Span::new(9, 12));
    assert_eq!(items[0].node.blocks[0].span, Span::new(2, 3));
}

#[test]
fn enumerated_lists() {
    assert_eq!(out("1. one\n2. two\n"), [r#"OL1(P["one"]; P["two"])"#]);
    assert_eq!(out("3. x\n4. y\n"), [r#"OL3(P["x"]; P["y"])"#]);
    assert_eq!(out("#. a\n#. b\n"), [r#"OL#(P["a"]; P["b"])"#]);
    assert_eq!(
        out("9. a\n   continued\n10. b\n"),
        [r#"OL9(P["a\n   continued"]; P["b"])"#]
    );
}

#[test]
fn enumerated_lists_require_sequence() {
    // Expected trees come from docutils 0.23. An explicit ordinal may be
    // followed by `#.`, but `#.` may not be followed by an explicit one.
    assert_eq!(out("1. a\n\n3. b\n"), [r#"OL1(P["a"])"#, r#"OL3(P["b"])"#]);
    assert_eq!(out("1. a\n\n#. b\n"), [r#"OL1(P["a"]; P["b"])"#]);
    assert_eq!(
        out("1. a\n2. b\n#. c\n"),
        [r#"OL1(P["a"]; P["b"]; P["c"])"#]
    );
    assert_eq!(out("#. a\n\n2. b\n"), [r#"OL#(P["a"])"#, r#"OL2(P["b"])"#]);
    // Without a blank line, a non-successor makes the line before it text.
    assert_eq!(out("1. a\n3. b\n"), [r#"P["1. a\n3. b"]"#]);
    assert_eq!(
        out("#. a\n#. b\n3. c\n"),
        [r#"OL#(P["a"])"#, r##"P["#. b\n3. c"]"##]
    );
}

#[test]
fn enumerator_followed_by_text_is_paragraph() {
    assert_eq!(
        out("2026. was a year\nand more\n"),
        [r#"P["2026. was a year\nand more"]"#]
    );
    assert_eq!(out("1. Intro\n========\n"), [r#"H1["1. Intro"]"#]);
    assert_eq!(out("1.\n"), [r#"P["1."]"#]);
}

#[test]
fn directive_with_arguments_options_and_body() {
    let src = ".. code-block:: rust\n   :linenos:\n   :caption: Example\n      continued\n\n   fn main() {}\n     indented\n";
    assert_eq!(
        out(src),
        [
            r#"Dcode-block[Some("rust")]{linenos=None,caption=Some("Example\n      continued")}(Some("   fn main() {}\n     indented"))"#
        ]
    );
}

#[test]
fn directive_argument_lines_and_body_without_blank() {
    // Arguments continue until an option line; a non-option line after the
    // options starts the body.
    let src = ".. figure:: a.png\n   more args\n   :alt: x\n   not an option\n\n   caption\n";
    assert_eq!(
        out(src),
        [
            r#"Dfigure[Some("a.png\n   more args")]{alt=Some("x")}(Some("   not an option\n\n   caption"))"#
        ]
    );
}

#[test]
fn directive_without_arguments_or_body() {
    assert_eq!(out(".. contents::"), ["Dcontents[None]{}(None)"]);
    assert_eq!(
        out(".. image::\n   :alt: hi\n"),
        [r#"Dimage[None]{alt=Some("hi")}(None)"#]
    );
}

#[test]
fn directive_first_line_is_arguments() {
    assert_eq!(
        out(".. note:: This is\n   a note\n"),
        [r#"Dnote[Some("This is\n   a note")]{}(None)"#]
    );
}

#[test]
fn directive_parse_body_keeps_absolute_spans() {
    let src = "intro\n\n.. note::\n\n   Hello *world*.\n\n   - a\n   - b\n\n   .. warning:: inner\n\n      deep\n";
    let doc = parse_checked(src);
    let Block::Directive(d) = &doc.blocks[1].node else {
        panic!("expected directive");
    };
    let body = d.parse_body();
    assert_eq!(
        blocks_outline(&body),
        r#"P["Hello " em"world" "."], UL-(P["a"]; P["b"]), Dwarning[Some("inner")]{}(Some("      deep"))"#
    );
    assert_eq!(&src[body[0].span.range()], "Hello *world*.");
    let Block::Directive(inner) = &body[2].node else {
        panic!("expected nested directive");
    };
    let deep = inner.parse_body();
    assert_eq!(blocks_outline(&deep), r#"P["deep"]"#);
    assert_eq!(&src[deep[0].span.range()], "deep");
}

#[test]
fn comments() {
    let src = ".. a comment\n   continues\n\n..\n\n   quoted\n\n..\n   text\n.. _target: http://x\n.. [1] note\n.. |s| image:: x\n.. foo bar:: x\n.. -x:: y\n.. a::b\n";
    assert_eq!(
        out(src),
        [
            r#"C"a comment\n   continues""#,
            r#"C"""#,
            r#"Q(P["quoted"])"#,
            r#"C"text""#,
            r#"C"_target: http://x""#,
            r#"C"[1] note""#,
            r#"C"|s| image:: x""#,
            r#"C"foo bar:: x""#,
            r#"C"-x:: y""#,
            r#"C"a::b""#,
        ]
    );
}

#[test]
fn ellipsis_is_not_explicit_markup() {
    assert_eq!(out("...\n"), [r#"P["..."]"#]);
}

#[test]
fn crlf_input() {
    let src = "Title\r\n=====\r\n\r\nPara one\r\nline two\r\n\r\n- a\r\n  b\r\n\r\n.. note:: x\r\n\r\n   body\r\n";
    assert_eq!(
        out(src),
        [
            r#"H1["Title"]"#,
            r#"P["Para one\r\nline two"]"#,
            r#"UL-(P["a\r\n  b"])"#,
            r#"Dnote[Some("x")]{}(Some("   body"))"#,
        ]
    );
}

#[test]
fn tabs_expand_to_multiples_of_eight() {
    assert_eq!(
        out("\tquoted\n\n::\n\n\tcode\n\t  more\n\n-\tx\n\ty\n"),
        [
            r#"Q(P["quoted"])"#,
            r#"LIT"\tcode\n\t  more""#,
            r#"UL-(P["x\n\ty"])"#
        ]
    );
    // Eight spaces and one tab are the same indent.
    assert_eq!(out("        a\n\tb\n"), [r#"Q(P["a\n\tb"])"#]);
}

#[test]
fn japanese_text() {
    assert_eq!(
        out("日本語の段落です。\n二行目。\n\n見出し\n======\n"),
        [r#"P["日本語の段落です。\n二行目。"]"#, r#"H1["見出し"]"#]
    );
}
