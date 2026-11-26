//! Block pass: splits the source into [`Line`]s, classifies each line, and
//! dispatches to one function per construct.

use crate::ast::*;
use crate::inline::{is_simple_name, parse_inlines};

pub(crate) const TAB_WIDTH: usize = 8;

/// Nesting depth (block quotes, list items) after which the remaining lines
/// are kept as one paragraph, so hostile input cannot overflow the stack.
const MAX_DEPTH: usize = 64;

/// One physical line. Offsets are relative to the text being parsed (the
/// whole source, or a directive body).
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    /// The physical line from column 0, without its `\n`. Used to compute
    /// columns; it is never sliced into the tree.
    raw: &'a str,
    /// Byte offset of column 0.
    start: usize,
    /// Indentation in columns, with tabs advancing to the next multiple of 8.
    indent: usize,
    /// Byte offset of `text`.
    content: usize,
    /// Content after the indentation, without trailing whitespace or `\r`.
    text: &'a str,
    blank: bool,
}

impl Line<'_> {
    fn end(&self) -> usize {
        self.content + self.text.len()
    }
}

fn split_lines(src: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for raw in src.split('\n') {
        let ws = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        let text = raw[ws..].trim_end();
        lines.push(Line {
            raw,
            start,
            indent: column(raw, ws),
            content: start + ws,
            text,
            blank: text.is_empty(),
        });
        start += raw.len() + 1;
    }
    lines
}

/// Column of byte `byte` in `raw`.
pub(crate) fn column(raw: &str, byte: usize) -> usize {
    raw[..byte].chars().fold(0, |col, c| {
        if c == '\t' {
            (col / TAB_WIDTH + 1) * TAB_WIDTH
        } else {
            col + 1
        }
    })
}

/// `line` with its first `marker_len` bytes of content (a list marker) and
/// the following blanks removed, so the item body starts after the marker.
fn after_marker<'a>(line: &Line<'a>, marker_len: usize) -> Line<'a> {
    let text = line.text[marker_len..].trim_start_matches([' ', '\t']);
    let content = line.end() - text.len();
    Line {
        indent: column(line.raw, content - line.start),
        content,
        text,
        blank: text.is_empty(),
        ..*line
    }
}

/// Index after the last non-blank line, starting at `from`, of the block
/// whose lines are blank or indented more than `indent`.
fn indented_end(lines: &[Line<'_>], from: usize, indent: usize) -> usize {
    let mut end = from;
    for (j, line) in lines.iter().enumerate().skip(from) {
        if line.blank {
            continue;
        }
        if line.indent <= indent {
            break;
        }
        end = j + 1;
    }
    end
}

fn skip_blank(lines: &[Line<'_>], from: usize) -> usize {
    (from..lines.len())
        .find(|&j| !lines[j].blank)
        .unwrap_or(lines.len())
}

fn min_indent(lines: &[Line<'_>]) -> usize {
    lines
        .iter()
        .filter(|l| !l.blank)
        .map(|l| l.indent)
        .min()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Enumerator {
    /// `None` for `#`.
    pub(crate) value: Option<u32>,
    /// Marker length in bytes, including the `.`.
    pub(crate) len: usize,
}

impl Enumerator {
    /// Whether `next` continues a list whose last item used `self`.
    fn followed_by(self, next: Enumerator) -> bool {
        match (self.value, next.value) {
            (_, None) => true,
            (Some(a), Some(b)) => a.checked_add(1) == Some(b),
            (None, Some(_)) => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LineKind {
    Blank,
    Indented,
    Bullet(BulletMarker),
    Enumerator(Enumerator),
    ExplicitMarkup,
    Adornment,
    Text,
}

fn classify(line: &Line<'_>, base: usize, top: bool) -> LineKind {
    if line.blank {
        LineKind::Blank
    } else if line.indent > base {
        LineKind::Indented
    } else if let Some(marker) = bullet_marker(line.text) {
        LineKind::Bullet(marker)
    } else if let Some(e) = enumerator(line.text) {
        LineKind::Enumerator(e)
    } else if is_explicit_markup(line.text) {
        LineKind::ExplicitMarkup
    } else if top && is_adornment(line.text) {
        LineKind::Adornment
    } else {
        LineKind::Text
    }
}

fn followed_by_blank_or_end(text: &str, at: usize) -> bool {
    matches!(text.as_bytes().get(at), None | Some(b' ' | b'\t'))
}

fn bullet_marker(text: &str) -> Option<BulletMarker> {
    let marker = match text.as_bytes().first()? {
        b'-' => BulletMarker::Hyphen,
        b'*' => BulletMarker::Asterisk,
        b'+' => BulletMarker::Plus,
        _ => return None,
    };
    followed_by_blank_or_end(text, 1).then_some(marker)
}

/// `N. ` or `#. `; the marker must be followed by a blank and content.
pub(crate) fn enumerator(text: &str) -> Option<Enumerator> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let (value, len) = if digits > 0 {
        (Some(text[..digits].parse::<u32>().ok()?), digits + 1)
    } else if text.starts_with('#') {
        (None, 2)
    } else {
        return None;
    };
    let rest = text.as_bytes();
    (rest.get(len - 1) == Some(&b'.') && matches!(rest.get(len), Some(b' ' | b'\t')))
        .then_some(Enumerator { value, len })
}

fn is_explicit_markup(text: &str) -> bool {
    text.starts_with("..") && followed_by_blank_or_end(text, 2)
}

/// A line of one repeated ASCII punctuation character.
pub(crate) fn is_adornment(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .first()
        .is_some_and(|&c| c.is_ascii_punctuation() && bytes.iter().all(|&b| b == c))
}

fn option_marker_end(text: &str) -> Option<usize> {
    if !text.starts_with(':') {
        return None;
    }
    let q = text[1..].find(':').map(|q| q + 1)?;
    let name = &text[1..q];
    let valid = !name.is_empty()
        && !name.starts_with(char::is_whitespace)
        && !name.ends_with(char::is_whitespace)
        && followed_by_blank_or_end(text, q + 1);
    valid.then_some(q)
}

pub(crate) fn parse_document(src: &str) -> Blocks<'_> {
    let lines = split_lines(src);
    let mut parser = Parser::new(src, 0);
    parser.blocks(&lines, 0, true)
}

/// Parses a directive body: `body` is a slice of the source that starts at
/// column 0 of a line at absolute byte `offset`.
pub(crate) fn parse_nested(body: &str, offset: u32) -> Blocks<'_> {
    let lines = split_lines(body);
    let mut parser = Parser::new(body, offset);
    parser.nested(&lines, min_indent(&lines))
}

struct Parser<'a> {
    src: &'a str,
    /// Absolute offset of `src` in the document source.
    offset: u32,
    /// Heading styles in order of first appearance; level = index + 1.
    styles: Vec<HeadingStyle>,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, offset: u32) -> Self {
        Parser {
            src,
            offset,
            styles: Vec::new(),
            depth: 0,
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.offset + start as u32, self.offset + end as u32)
    }

    fn spanned<T>(&self, node: T, start: usize, end: usize) -> Spanned<T> {
        Spanned::new(node, self.span(start, end))
    }

    fn slice(&self, start: usize, end: usize) -> Spanned<&'a str> {
        self.spanned(&self.src[start..end], start, end)
    }

    fn inlines(&self, start: usize, end: usize) -> Inlines<'a> {
        parse_inlines(&self.src[start..end], self.offset + start as u32)
    }

    /// Parses blocks whose lines are all at indent `base` or deeper. Only the
    /// document itself is `top`: headings and transitions live there.
    fn blocks(&mut self, lines: &[Line<'a>], base: usize, top: bool) -> Blocks<'a> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            i = match classify(&lines[i], base, top) {
                LineKind::Blank => i + 1,
                LineKind::Indented => self.block_quote(lines, i, base, &mut out),
                LineKind::Bullet(marker) => self.bullet_list(lines, i, marker, &mut out),
                LineKind::Enumerator(e) => self.enumerated_list(lines, i, e, base, top, &mut out),
                LineKind::ExplicitMarkup => self.explicit_markup(lines, i, &mut out),
                LineKind::Adornment => self.adornment(lines, i, base, &mut out),
                LineKind::Text => self.paragraph(lines, i, base, top, true, &mut out),
            };
        }
        out
    }

    /// Blocks of a nested container (block quote, list item, directive body).
    fn nested(&mut self, lines: &[Line<'a>], base: usize) -> Blocks<'a> {
        if self.depth >= MAX_DEPTH {
            return self.flat_paragraph(lines);
        }
        self.depth += 1;
        let blocks = self.blocks(lines, base, false);
        self.depth -= 1;
        blocks
    }

    fn flat_paragraph(&self, lines: &[Line<'a>]) -> Blocks<'a> {
        let mut non_blank = lines.iter().filter(|l| !l.blank);
        let Some(first) = non_blank.next() else {
            return Vec::new();
        };
        let end = non_blank.next_back().unwrap_or(first).end();
        let inlines = self.inlines(first.content, end);
        vec![self.spanned(Block::Paragraph(inlines), first.content, end)]
    }

    fn heading(
        &mut self,
        adornment: u8,
        overline: bool,
        title: &Line<'a>,
        start: usize,
        end: usize,
    ) -> Spanned<Block<'a>> {
        let style = HeadingStyle {
            adornment,
            overline,
        };
        let index = match self.styles.iter().position(|s| *s == style) {
            Some(index) => index,
            None => {
                self.styles.push(style);
                self.styles.len() - 1
            }
        };
        let level = u8::try_from(index + 1).unwrap_or(u8::MAX);
        let title = self.inlines(title.content, title.end());
        self.spanned(
            Block::Heading {
                level,
                style,
                title,
            },
            start,
            end,
        )
    }

    /// A text block. At top level its first two lines may be an underlined
    /// title. A trailing `::` introduces a literal block.
    fn paragraph(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        base: usize,
        top: bool,
        allow_title: bool,
        out: &mut Blocks<'a>,
    ) -> usize {
        let first = lines[i];
        if top && allow_title && first.indent == 0 {
            if let Some(under) = lines.get(i + 1) {
                if !under.blank
                    && under.indent == 0
                    && is_adornment(under.text)
                    && under.text.len() >= first.text.chars().count()
                {
                    let heading = self.heading(
                        under.text.as_bytes()[0],
                        false,
                        &first,
                        first.content,
                        under.end(),
                    );
                    out.push(heading);
                    return i + 2;
                }
            }
        }

        let mut j = i + 1;
        while j < lines.len() && !lines[j].blank && lines[j].indent <= base {
            j += 1;
        }
        let start = first.content;
        let end = lines[j - 1].end();
        let text = &self.src[start..end];
        let Some(before) = text.strip_suffix("::") else {
            out.push(self.spanned(Block::Paragraph(self.inlines(start, end)), start, end));
            return j;
        };

        let inline_end = if before.is_empty() {
            None
        } else if before.ends_with(char::is_whitespace) {
            Some(start + before.trim_end().len())
        } else {
            Some(end - 1)
        };
        if let Some(inline_end) = inline_end {
            let inlines = self.inlines(start, inline_end);
            out.push(self.spanned(Block::Paragraph(inlines), start, end));
        }

        let k = skip_blank(lines, j);
        if k < lines.len() && lines[k].indent > base {
            let literal_end = indented_end(lines, j, base);
            let literal = self.slice(lines[k].start, lines[literal_end - 1].end());
            out.push(Spanned::new(
                Block::LiteralBlock(literal.node),
                literal.span,
            ));
            return literal_end;
        }
        j
    }

    /// An adornment line at top level: overline title, transition, or text.
    fn adornment(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        base: usize,
        out: &mut Blocks<'a>,
    ) -> usize {
        let over = lines[i];
        let ch = over.text.as_bytes()[0];
        if let (Some(title), Some(under)) = (lines.get(i + 1), lines.get(i + 2)) {
            if !title.blank
                && !under.blank
                && under.indent == 0
                && is_adornment(under.text)
                && under.text.as_bytes()[0] == ch
            {
                let heading = self.heading(ch, true, title, over.content, under.end());
                out.push(heading);
                return i + 3;
            }
        }
        let blank_before = i == 0 || lines[i - 1].blank;
        let blank_after = lines.get(i + 1).is_none_or(|l| l.blank);
        if blank_before && blank_after && over.text.len() >= 4 {
            out.push(self.spanned(Block::Transition, over.content, over.end()));
            return i + 1;
        }
        self.paragraph(lines, i, base, true, false, out)
    }

    fn block_quote(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        base: usize,
        out: &mut Blocks<'a>,
    ) -> usize {
        let end = indented_end(lines, i, base);
        let body = &lines[i..end];
        let blocks = self.nested(body, min_indent(body));
        out.push(self.spanned(
            Block::BlockQuote(blocks),
            lines[i].content,
            lines[end - 1].end(),
        ));
        end
    }

    /// One list item whose marker (`marker_len` bytes) starts line `i`.
    /// Returns the item and the index after its last non-blank line.
    fn list_item(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        marker_len: usize,
    ) -> (Spanned<ListItem<'a>>, usize) {
        let marker_line = lines[i];
        let first = after_marker(&marker_line, marker_len);
        // Like docutils, the body is indented to the first line's text
        // column; an empty first line defers to the continuation lines.
        let body_floor = if first.blank {
            marker_line.indent
        } else {
            first.indent - 1
        };
        let end = indented_end(lines, i + 1, body_floor);
        let mut item_lines = Vec::with_capacity(end - i);
        item_lines.push(first);
        item_lines.extend_from_slice(&lines[i + 1..end]);
        let blocks = self.nested(&item_lines, min_indent(&item_lines));
        let item = self.spanned(
            ListItem { blocks },
            marker_line.content,
            lines[end - 1].end(),
        );
        (item, end)
    }

    fn bullet_list(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        marker: BulletMarker,
        out: &mut Blocks<'a>,
    ) -> usize {
        let indent = lines[i].indent;
        let mut items = Vec::new();
        let mut j = i;
        loop {
            let (item, next) = self.list_item(lines, j, 1);
            items.push(item);
            let k = skip_blank(lines, next);
            match lines.get(k) {
                Some(l) if l.indent == indent && bullet_marker(l.text) == Some(marker) => j = k,
                _ => {
                    j = next;
                    break;
                }
            }
        }
        let end = lines[j - 1].end();
        out.push(self.spanned(Block::BulletList { marker, items }, lines[i].content, end));
        j
    }

    /// An enumerator line `e` starts an item only if the next line is blank,
    /// indented, a successor enumerator at the same indent, or absent; this
    /// keeps `2026. was a year` inside a paragraph.
    fn item_start_ok(lines: &[Line<'_>], i: usize, e: Enumerator) -> bool {
        match lines.get(i + 1) {
            None => true,
            Some(next) => {
                next.blank
                    || next.indent > lines[i].indent
                    || (next.indent == lines[i].indent
                        && enumerator(next.text).is_some_and(|n| e.followed_by(n)))
            }
        }
    }

    fn enumerated_list(
        &mut self,
        lines: &[Line<'a>],
        i: usize,
        first: Enumerator,
        base: usize,
        top: bool,
        out: &mut Blocks<'a>,
    ) -> usize {
        if !Self::item_start_ok(lines, i, first) {
            return self.paragraph(lines, i, base, top, true, out);
        }
        let indent = lines[i].indent;
        let mut items = Vec::new();
        let mut current = first;
        let mut j = i;
        loop {
            let (item, next) = self.list_item(lines, j, current.len);
            items.push(item);
            let k = skip_blank(lines, next);
            let following = lines
                .get(k)
                .filter(|l| l.indent == indent)
                .and_then(|l| enumerator(l.text))
                .filter(|&e| current.followed_by(e) && Self::item_start_ok(lines, k, e));
            match following {
                Some(e) => {
                    current = e;
                    j = k;
                }
                None => {
                    j = next;
                    break;
                }
            }
        }
        let end = lines[j - 1].end();
        let list = Block::EnumeratedList {
            start: first.value,
            items,
        };
        out.push(self.spanned(list, lines[i].content, end));
        j
    }

    /// `.. name:: ...` directives; everything else after `..` is a comment.
    fn explicit_markup(&mut self, lines: &[Line<'a>], i: usize, out: &mut Blocks<'a>) -> usize {
        let line = lines[i];
        if line.text == ".." && lines.get(i + 1).is_none_or(|l| l.blank) {
            // An empty comment does not swallow the indented block after the
            // blank line.
            let empty = self.slice(line.end(), line.end());
            out.push(self.spanned(Block::Comment(empty.node), line.content, line.end()));
            return i + 1;
        }
        let end = indented_end(lines, i + 1, line.indent);
        let block_end = lines[end - 1].end();
        let node = match self.directive(lines, i, end) {
            Some(directive) => Block::Directive(directive),
            None => {
                let first = line.text[2..].trim_start();
                let text_start = if !first.is_empty() {
                    line.end() - first.len()
                } else {
                    lines[i + 1..end]
                        .iter()
                        .find(|l| !l.blank)
                        .map_or(line.end(), |l| l.content)
                };
                Block::Comment(self.slice(text_start, block_end).node)
            }
        };
        out.push(self.spanned(node, line.content, block_end));
        end
    }

    /// Parses lines `i..end` as a directive, or `None` if the first line is
    /// not `.. name::` followed by a blank or the line end.
    fn directive(&self, lines: &[Line<'a>], i: usize, end: usize) -> Option<Directive<'a>> {
        let line = lines[i];
        let rest = line.text[2..].trim_start_matches([' ', '\t']);
        let name_start = line.end() - rest.len();
        let p = rest.find("::")?;
        if !is_simple_name(&rest[..p]) || !followed_by_blank_or_end(rest, p + 2) {
            return None;
        }
        let name = self.slice(name_start, name_start + p);

        // Arguments: the rest of the first line, then non-blank lines up to
        // the first option line.
        let first_arg = rest[p + 2..].trim_start();
        let mut arg_start = (!first_arg.is_empty()).then(|| line.end() - first_arg.len());
        let mut arg_end = line.end();
        let mut k = i + 1;
        while k < end && !lines[k].blank && option_marker_end(lines[k].text).is_none() {
            arg_start.get_or_insert(lines[k].content);
            arg_end = lines[k].end();
            k += 1;
        }
        let arguments = arg_start.map(|s| self.slice(s, arg_end));

        let mut options = Vec::new();
        while k < end && !lines[k].blank {
            let Some(q) = option_marker_end(lines[k].text) else {
                break;
            };
            let opt = lines[k];
            let first_value = opt.text[q + 1..].trim_start();
            let mut value_start = (!first_value.is_empty()).then(|| opt.end() - first_value.len());
            let mut value_end = opt.end();
            let mut m = k + 1;
            while m < end && !lines[m].blank && lines[m].indent > opt.indent {
                value_start.get_or_insert(lines[m].content);
                value_end = lines[m].end();
                m += 1;
            }
            let option = DirectiveOption {
                name: self.slice(opt.content + 1, opt.content + q),
                value: value_start.map(|s| self.slice(s, value_end)),
            };
            options.push(self.spanned(option, opt.content, value_end));
            k = m;
        }

        let body = (k..end)
            .find(|&b| !lines[b].blank)
            .map(|b| self.slice(lines[b].start, lines[end - 1].end()));
        Some(Directive {
            name,
            arguments,
            options,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_track_offsets_indent_and_trim() {
        let lines = split_lines("a\r\n  b  \n\tc\n \t d\n");
        let got: Vec<_> = lines
            .iter()
            .map(|l| (l.start, l.indent, l.content, l.text, l.blank))
            .collect();
        assert_eq!(
            got,
            vec![
                (0, 0, 0, "a", false),
                (3, 2, 5, "b", false),
                (9, 8, 10, "c", false),
                (12, 9, 15, "d", false),
                (17, 0, 17, "", true),
            ]
        );
    }

    #[test]
    fn marker_detection() {
        assert!(bullet_marker("- a").is_some());
        assert!(bullet_marker("-").is_some());
        assert!(bullet_marker("--").is_none());
        assert_eq!(
            enumerator("12. x").map(|e| (e.value, e.len)),
            Some((Some(12), 3))
        );
        assert_eq!(
            enumerator("#. x").map(|e| (e.value, e.len)),
            Some((None, 2))
        );
        assert!(enumerator("1.").is_none());
        assert!(enumerator("1.x").is_none());
        assert!(enumerator("99999999999. x").is_none());
        assert!(is_adornment("===="));
        assert!(!is_adornment("==-="));
        assert!(!is_adornment("== =="));
        assert!(is_explicit_markup(".."));
        assert!(is_explicit_markup(".. x"));
        assert!(!is_explicit_markup("..."));
        assert_eq!(option_marker_end(":alt: x"), Some(4));
        assert_eq!(option_marker_end(":flag:"), Some(5));
        assert_eq!(option_marker_end("::"), None);
        assert_eq!(option_marker_end(":a:b"), None);
    }
}
