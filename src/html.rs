//! HTML renderer.

use std::fmt::Write;

use crate::ast::*;
use crate::inline::parse_inlines;

/// Directive bodies are parsed while rendering; past this nesting depth a
/// body is shown as preformatted text instead of being parsed again.
const MAX_DIRECTIVE_DEPTH: usize = 32;

pub(crate) fn render(doc: &Document<'_>) -> String {
    let mut out = String::new();
    blocks(&mut out, &doc.blocks, 0);
    out
}

fn blocks(out: &mut String, blocks: &[Spanned<Block<'_>>], depth: usize) {
    for b in blocks {
        block(out, &b.node, depth);
    }
}

fn block(out: &mut String, b: &Block<'_>, depth: usize) {
    match b {
        Block::Heading { level, title, .. } => {
            let n = (*level).clamp(1, 6);
            let _ = write!(out, "<h{n}>");
            inlines(out, title);
            let _ = writeln!(out, "</h{n}>");
        }
        Block::Paragraph(content) => {
            out.push_str("<p>");
            inlines(out, content);
            out.push_str("</p>\n");
        }
        Block::BulletList { items, .. } => {
            out.push_str("<ul>\n");
            list_items(out, items, depth);
            out.push_str("</ul>\n");
        }
        Block::EnumeratedList { start, items } => {
            match start {
                Some(n) if *n != 1 => {
                    let _ = writeln!(out, "<ol start=\"{n}\">");
                }
                _ => out.push_str("<ol>\n"),
            }
            list_items(out, items, depth);
            out.push_str("</ol>\n");
        }
        Block::LiteralBlock(text) => {
            out.push_str("<pre>");
            escape(out, &dedent(text));
            out.push_str("</pre>\n");
        }
        Block::BlockQuote(children) => {
            out.push_str("<blockquote>\n");
            blocks(out, children, depth + 1);
            out.push_str("</blockquote>\n");
        }
        Block::Directive(d) => directive(out, d, depth),
        Block::Transition => out.push_str("<hr />\n"),
        Block::Comment(_) => {}
    }
}

fn list_items(out: &mut String, items: &[Spanned<ListItem<'_>>], depth: usize) {
    for item in items {
        out.push_str("<li>");
        blocks(out, &item.node.blocks, depth + 1);
        out.push_str("</li>\n");
    }
}

fn directive(out: &mut String, d: &Directive<'_>, depth: usize) {
    let body = d.body.as_ref().map_or("", |b| b.node);
    match d.name.node {
        "code" | "code-block" | "sourcecode" => {
            out.push_str("<pre><code");
            if let Some(lang) = d
                .arguments
                .as_ref()
                .and_then(|a| a.node.split_whitespace().next())
            {
                out.push_str(" class=\"language-");
                escape(out, lang);
                out.push('"');
            }
            out.push('>');
            escape(out, &dedent(body));
            out.push_str("</code></pre>\n");
        }
        name => {
            out.push_str("<div class=\"directive directive-");
            escape(out, name);
            out.push_str("\">\n");
            if let Some(args) = &d.arguments {
                out.push_str("<p class=\"directive-arguments\">");
                inlines(out, &parse_inlines(args.node, args.span.start));
                out.push_str("</p>\n");
            }
            if depth < MAX_DIRECTIVE_DEPTH {
                blocks(out, &d.parse_body(), depth + 1);
            } else if !body.is_empty() {
                out.push_str("<pre>");
                escape(out, &dedent(body));
                out.push_str("</pre>\n");
            }
            out.push_str("</div>\n");
        }
    }
}

fn inlines(out: &mut String, inlines: &[Spanned<Inline<'_>>]) {
    for inline in inlines {
        match &inline.node {
            Inline::Text(t) => escape(out, &unescape(t)),
            Inline::Emphasis(t) => wrap(out, "em", t),
            Inline::Strong(t) => wrap(out, "strong", t),
            Inline::Literal(t) => {
                out.push_str("<code>");
                escape(out, t);
                out.push_str("</code>");
            }
            Inline::Interpreted(t) => wrap(out, "cite", t),
            Inline::Role { name, content } => match role_tag(name.node) {
                Some(tag) => wrap(out, tag, content.node),
                None => {
                    out.push_str("<span class=\"role-");
                    escape(out, name.node);
                    out.push_str("\">");
                    escape(out, &unescape(content.node));
                    out.push_str("</span>");
                }
            },
            Inline::Reference { label, target, .. } => {
                let label = unescape(label.node);
                let href = match target {
                    Some(t) => unescape(t.node).split_whitespace().collect::<String>(),
                    None => format!("#{}", anchor(&label)),
                };
                out.push_str("<a href=\"");
                escape(out, &href);
                out.push_str("\">");
                escape(out, &label);
                out.push_str("</a>");
            }
        }
    }
}

fn role_tag(name: &str) -> Option<&'static str> {
    Some(match name {
        "emphasis" => "em",
        "strong" => "strong",
        "literal" | "code" => "code",
        "sub" | "subscript" => "sub",
        "sup" | "superscript" => "sup",
        "title-reference" | "title" | "t" => "cite",
        _ => return None,
    })
}

fn wrap(out: &mut String, tag: &str, text: &str) {
    let _ = write!(out, "<{tag}>");
    escape(out, &unescape(text));
    let _ = write!(out, "</{tag}>");
}

/// Implicit target name for `` `label`_ ``: lowercased, whitespace runs
/// replaced by `-`.
fn anchor(label: &str) -> String {
    label
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

fn escape(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// Resolves backslash escapes (`\x` -> `x`, backslash before whitespace ->
/// nothing) and collapses each line break plus the next line's indentation
/// into one `\n`.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(n) if n == '\r' || n == '\n' => skip_line_break(&mut chars),
                Some(n) if n.is_whitespace() => {}
                Some(n) => out.push(n),
                None => out.push('\\'),
            },
            '\r' | '\n' => {
                skip_line_break(&mut chars);
                out.push('\n');
            }
            _ => out.push(c),
        }
    }
    out
}

fn skip_line_break(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars
        .next_if(|&c| matches!(c, ' ' | '\t' | '\r' | '\n'))
        .is_some()
    {}
}

/// Expands tabs and removes the smallest indentation of the non-blank lines.
fn dedent(text: &str) -> String {
    let lines: Vec<String> = text
        .split('\n')
        .map(|l| expand_tabs(l.trim_end_matches('\r')))
        .collect();
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| l.get(indent..).unwrap_or("").trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

fn expand_tabs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0;
    for c in line.chars() {
        if c == '\t' {
            let next = (col / 8 + 1) * 8;
            out.extend(std::iter::repeat_n(' ', next - col));
            col = next;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_rules() {
        assert_eq!(unescape(r"\*a\*"), "*a*");
        assert_eq!(unescape("a\\ b"), "ab");
        assert_eq!(unescape("a\\\n   b"), "ab");
        assert_eq!(unescape("a\r\n   b"), "a\nb");
        assert_eq!(unescape(r"a\\b"), r"a\b");
        assert_eq!(unescape("a\\"), "a\\");
    }

    #[test]
    fn dedent_expands_tabs() {
        assert_eq!(dedent("\tx\n\t  y\n\n"), "x\n  y\n\n");
        assert_eq!(dedent("    a\n  b"), "  a\nb");
    }
}
