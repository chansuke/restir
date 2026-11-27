# restir

[![crates.io](https://img.shields.io/crates/v/restir.svg)](https://crates.io/crates/restir)
[![docs.rs](https://img.shields.io/docsrs/restir)](https://docs.rs/restir)

restir is a reStructuredText parser for Rust. It has no dependencies and does not copy the input. Every text node in the tree is a `&str` slice of your source, and every node carries a `Span` with `u32` byte offsets into it.

## Usage

```rust
let source = "Hello\n=====\n\nSome *emphasis* and ``code``.\n";
let doc = restir::parse(source);

let html = restir::render_html(&doc);
assert_eq!(
	html,
	"<h1>Hello</h1>\n<p>Some <em>emphasis</em> and <code>code</code>.</p>\n"
);

// Each block knows where it came from.
let first = &doc.blocks[0];
assert_eq!(&source[first.span.range()], "Hello\n=====");
```

`parse` never panics on malformed markup. Markup that does not close, such as `*text`, stays plain text. Input of 4 GiB or more does not fit in `u32` offsets, so `parse` panics on it. Use `try_parse` if your input size is not under your control.

## Supported syntax

Block syntax covers section titles with an underline or with an overline and an underline, paragraphs, bullet lists, enumerated lists with `1.` or `#.`, literal blocks after `::`, block quotes, transitions, comments, and directives. Heading levels follow the order in which each title style first appears in the document, as in docutils.

Inline syntax covers `*emphasis*`, `**strong**`, ``` ``literal`` ```, roles such as ``:sub:`2` ``, interpreted text, and hyperlink references such as `` `Rust <https://www.rust-lang.org>`_ ``. Inline markup follows the docutils recognition rules, so `2*3*4` stays plain text.

A directive keeps its name, arguments, options, and body. The body is a raw slice of the source. Call `Directive::parse_body` to parse it as reStructuredText. The spans in the result still point into the original source.

Tables, footnotes, citations, hyperlink targets, and substitution definitions are not supported. A table becomes paragraph text. The other four start with `..`, so they become comments.

## Differences from docutils

restir treats text on the first line of a directive as arguments. docutils treats it as content for directives that take no arguments, such as `note`. Title underlines are compared by character count, so a title in Japanese needs a shorter underline than docutils requires. Definition lists are not supported, so a deeper indented line under a list item becomes a block quote. The HTML renderer does not filter link URLs, so do not render untrusted input without sanitizing the output.

## Verification

Run the unit and integration tests with `cargo test`. Every test parse also checks that each span matches its slice of the source.

The exhaustive test parses and renders every string up to 7 characters over 17 characters that the parser treats as special. Run it with this command.

```sh
RESTIR_EXHAUSTIVE_LEN=7 cargo test --release --test exhaustive -- --ignored
```

The Kani proofs in `src/proofs.rs` check the functions that compute byte offsets: list enumerators, link targets, title underlines, tab stops, and role names. Install [Kani](https://model-checking.github.io/kani/) and run `cargo kani`.

CI runs all of these on each push to `main`, with stable Rust and with Rust 1.82, the minimum supported version.

## Contributing

This repository does not accept pull requests. You can report a bug by opening an issue.

## License

restir is released under the MIT License. See the `LICENSE` file.
