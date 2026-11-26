//! No input panics, and the span invariants hold on every prefix of the
//! corpus (and every substring of its small documents).

mod common;

use common::{check_document, CORPUS};
use restir::{parse, render_html};

fn parse_render_check(src: &str) {
    let doc = parse(src);
    check_document(&doc);
    render_html(&doc);
}

fn boundaries(src: &str) -> Vec<usize> {
    let mut b: Vec<usize> = src.char_indices().map(|(i, _)| i).collect();
    b.push(src.len());
    b
}

#[test]
fn every_prefix_of_the_corpus() {
    for doc in CORPUS {
        for end in boundaries(doc) {
            parse_render_check(&doc[..end]);
        }
    }
}

#[test]
fn every_substring_of_small_documents() {
    for doc in CORPUS.iter().filter(|d| d.len() <= 120) {
        let b = boundaries(doc);
        for (n, &start) in b.iter().enumerate() {
            for &end in &b[n..] {
                parse_render_check(&doc[start..end]);
            }
        }
    }
}

#[test]
fn deep_nesting_does_not_overflow() {
    parse_render_check(&format!("{}x", "- ".repeat(5000)));
    let stairs: String = (0..3000).map(|i| format!("{}x\n", " ".repeat(i))).collect();
    parse_render_check(&stairs);
    let directives: String = (0..200)
        .map(|i| format!("{}.. note::\n\n", "   ".repeat(i)))
        .collect();
    parse_render_check(&directives);
}
