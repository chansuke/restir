//! Inline pass: splits one text slice into `Text` runs and inline markup,
//! following the docutils inline markup recognition rules.

use crate::ast::{Inline, Inlines, Span, Spanned};

/// Characters that may precede a start-string, besides whitespace.
const OPENERS: &[char] = &[
    '-', ':', '/', '\'', '"', '<', '(', '[', '{', '「', '『', '（', '【', '〔', '〈', '《', '［',
    '｛', '、', '。', '・', '，', '：',
];

/// Characters that may follow an end-string, besides whitespace.
const CLOSERS: &[char] = &[
    '-', '.', ',', ':', ';', '!', '?', '\\', '/', '\'', '"', ')', ']', '}', '>', '」', '』', '）',
    '】', '〕', '〉', '》', '］', '｝', '、', '。', '！', '？', '，', '．', '：', '；', '・',
];

/// A start-string between one of these pairs (`"*"`, `(*)`) is not markup.
const PAIRS: &[(char, char)] = &[
    ('\'', '\''),
    ('"', '"'),
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('<', '>'),
    ('「', '」'),
    ('『', '』'),
    ('（', '）'),
    ('【', '】'),
    ('〔', '〕'),
    ('〈', '〉'),
    ('《', '》'),
    ('［', '］'),
    ('｛', '｝'),
];

/// Punctuation allowed between the alphanumeric runs of a role or directive
/// name.
const NAME_PUNCT: &[char] = &['-', '_', '+', '.', ':'];

/// A docutils "simple name": alphanumeric runs joined by single `-_+.:`
/// characters, so it cannot start or end with punctuation or contain `::`.
pub(crate) fn is_simple_name(name: &str) -> bool {
    let mut prev_punct = true;
    for c in name.chars() {
        if c.is_alphanumeric() {
            prev_punct = false;
        } else if NAME_PUNCT.contains(&c) && !prev_punct {
            prev_punct = true;
        } else {
            return false;
        }
    }
    !prev_punct
}

/// Parses `text`, which starts at absolute byte `offset` in the source.
pub(crate) fn parse_inlines(text: &str, offset: u32) -> Inlines<'_> {
    let mut scanner = Scanner {
        text,
        offset,
        out: Vec::new(),
        pending: 0,
        no_end_after: [usize::MAX; EndKind::COUNT],
    };
    scanner.run();
    scanner.out
}

/// End-string searches that are memoized: once a search from `cs` fails,
/// every later search of the same kind fails too, because candidate
/// validity depends only on the candidate position.
#[derive(Clone, Copy)]
enum EndKind {
    Literal,
    Strong,
    Emphasis,
    Backtick,
    RoleBacktick,
}

impl EndKind {
    const COUNT: usize = 5;
}

struct Scanner<'a> {
    text: &'a str,
    offset: u32,
    out: Inlines<'a>,
    /// Start of the text run not yet pushed.
    pending: usize,
    no_end_after: [usize; EndKind::COUNT],
}

impl<'a> Scanner<'a> {
    fn run(&mut self) {
        let bytes = self.text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            i = match bytes[i] {
                b'\\' => i + 1 + self.char_len(i + 1),
                b'`' if bytes.get(i + 1) == Some(&b'`') => self.literal(i),
                b'`' => self.interpreted(i),
                b'*' if bytes.get(i + 1) == Some(&b'*') => {
                    self.delimited(i, "**", EndKind::Strong, Inline::Strong)
                }
                b'*' => self.delimited(i, "*", EndKind::Emphasis, Inline::Emphasis),
                b':' => self.role(i),
                _ => i + self.char_len(i),
            };
        }
        self.flush(bytes.len());
    }

    fn char_len(&self, i: usize) -> usize {
        self.text[i..].chars().next().map_or(0, char::len_utf8)
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.offset + start as u32, self.offset + end as u32)
    }

    fn spanned_slice(&self, start: usize, end: usize) -> Spanned<&'a str> {
        Spanned::new(&self.text[start..end], self.span(start, end))
    }

    fn flush(&mut self, upto: usize) {
        if upto > self.pending {
            let text = &self.text[self.pending..upto];
            let span = self.span(self.pending, upto);
            self.out.push(Spanned::new(Inline::Text(text), span));
        }
        self.pending = upto;
    }

    /// Pushes markup covering `start..end` and returns `end`.
    fn emit(&mut self, start: usize, end: usize, node: Inline<'a>) -> usize {
        self.flush(start);
        self.out.push(Spanned::new(node, self.span(start, end)));
        self.pending = end;
        end
    }

    /// Start-string at `s..e`: preceded by text start, whitespace or an
    /// opener, followed by non-whitespace, and not enclosed in a quote pair.
    fn start_ok(&self, s: usize, e: usize) -> bool {
        let Some(next) = self.text[e..].chars().next() else {
            return false;
        };
        if next.is_whitespace() {
            return false;
        }
        match self.text[..s].chars().next_back() {
            None => true,
            Some(p) if p.is_whitespace() => true,
            Some(p) if OPENERS.contains(&p) => !PAIRS.iter().any(|&(o, c)| o == p && c == next),
            Some(_) => false,
        }
    }

    /// End-string at `s..e`: preceded by non-whitespace, followed by text
    /// end, whitespace or a closer.
    fn end_ok(&self, s: usize, e: usize) -> bool {
        let prev_ok = self.text[..s]
            .chars()
            .next_back()
            .is_some_and(|p| !p.is_whitespace());
        prev_ok
            && self.text[e..]
                .chars()
                .next()
                .is_none_or(|n| n.is_whitespace() || CLOSERS.contains(&n))
    }

    /// Finds the first valid end-string after the non-empty content that
    /// starts at `cs`. `probe` returns the end-string length at a candidate
    /// position, or `None` when it is not a candidate.
    fn find_end(
        &mut self,
        cs: usize,
        kind: EndKind,
        escapes: bool,
        probe: impl Fn(&str) -> Option<usize>,
    ) -> Option<(usize, usize)> {
        if cs >= self.no_end_after[kind as usize] {
            return None;
        }
        let bytes = self.text.as_bytes();
        let mut j = cs;
        while j < bytes.len() {
            if escapes && bytes[j] == b'\\' {
                j += 1 + self.char_len(j + 1);
                continue;
            }
            if j > cs {
                if let Some(len) = probe(&self.text[j..]) {
                    if self.end_ok(j, j + len) {
                        return Some((j, len));
                    }
                }
            }
            j += self.char_len(j);
        }
        self.no_end_after[kind as usize] = cs;
        None
    }

    /// `*emphasis*` and `**strong**`. Returns the next scan position.
    fn delimited(
        &mut self,
        i: usize,
        delim: &'static str,
        kind: EndKind,
        node: fn(&'a str) -> Inline<'a>,
    ) -> usize {
        let cs = i + delim.len();
        if !self.start_ok(i, cs) {
            return cs;
        }
        let probe = |rest: &str| rest.starts_with(delim).then_some(delim.len());
        match self.find_end(cs, kind, true, probe) {
            Some((j, len)) => self.emit(i, j + len, node(&self.text[cs..j])),
            None => cs,
        }
    }

    /// ``` ``literal`` ```: no escapes inside.
    fn literal(&mut self, i: usize) -> usize {
        let cs = i + 2;
        if !self.start_ok(i, cs) {
            return cs;
        }
        let probe = |rest: &str| rest.starts_with("``").then_some(2);
        match self.find_end(cs, EndKind::Literal, false, probe) {
            Some((j, len)) => self.emit(i, j + len, Inline::Literal(&self.text[cs..j])),
            None => cs,
        }
    }

    /// `` `interpreted` ``, `` `reference`_ `` and `` `anonymous`__ ``.
    fn interpreted(&mut self, i: usize) -> usize {
        let cs = i + 1;
        if !self.start_ok(i, cs) {
            return cs;
        }
        let probe = |rest: &str| {
            if rest.starts_with("`__") {
                Some(3)
            } else if rest.starts_with("`_") {
                Some(2)
            } else {
                rest.starts_with('`').then_some(1)
            }
        };
        let Some((j, len)) = self.find_end(cs, EndKind::Backtick, true, probe) else {
            return cs;
        };
        let node = match len {
            1 => Inline::Interpreted(&self.text[cs..j]),
            _ => self.reference(cs, j, len == 3),
        };
        self.emit(i, j + len, node)
    }

    /// Splits `label <target>` when the content ends with `<...>` preceded
    /// by whitespace; otherwise the whole content is the label.
    fn reference(&self, cs: usize, ce: usize, anonymous: bool) -> Inline<'a> {
        let content = &self.text[cs..ce];
        let split = content
            .strip_suffix('>')
            .and_then(|inner| inner.rfind('<'))
            .filter(|&p| content[..p].ends_with(char::is_whitespace) && p + 1 < content.len() - 1);
        let (label, target) = match split {
            Some(p) => {
                let label_end = cs + content[..p].trim_end().len();
                let target = self.spanned_slice(cs + p + 1, ce - 1);
                (self.spanned_slice(cs, label_end), Some(target))
            }
            None => (self.spanned_slice(cs, ce), None),
        };
        Inline::Reference {
            label,
            target,
            anonymous,
        }
    }

    /// `` :name:`content` ``.
    fn role(&mut self, i: usize) -> usize {
        let Some(q) = self.role_name_end(i) else {
            return i + 1;
        };
        let cs = q + 2;
        if !self.start_ok(i, cs) {
            return cs;
        }
        let probe = |rest: &str| rest.starts_with('`').then_some(1);
        let Some((j, len)) = self.find_end(cs, EndKind::RoleBacktick, true, probe) else {
            return cs;
        };
        let node = Inline::Role {
            name: self.spanned_slice(i + 1, q),
            content: self.spanned_slice(cs, j),
        };
        self.emit(i, j + len, node)
    }

    /// For a `:` at `i`, the position of the `:` that closes a valid role
    /// name and is directly followed by a backtick.
    fn role_name_end(&self, i: usize) -> Option<usize> {
        let rest = &self.text[i + 1..];
        let mut prev_punct = true;
        for (k, c) in rest.char_indices() {
            if c.is_alphanumeric() {
                prev_punct = false;
            } else if NAME_PUNCT.contains(&c) && !prev_punct {
                if c == ':' && rest[k + 1..].starts_with('`') {
                    return Some(i + 1 + k);
                }
                prev_punct = true;
            } else {
                return None;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_names() {
        assert!(is_simple_name("code-block"));
        assert!(is_simple_name("py:func"));
        assert!(is_simple_name("a_b+c.d"));
        assert!(!is_simple_name(""));
        assert!(!is_simple_name("-a"));
        assert!(!is_simple_name("a-"));
        assert!(!is_simple_name("a::b"));
        assert!(!is_simple_name("a b"));
    }
}
