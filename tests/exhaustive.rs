//! Bounded exhaustive check of the whole pipeline: every string up to
//! `RESTIR_EXHAUSTIVE_LEN` chars (default 5) over an alphabet of the
//! characters the parser branches on parses and renders without panicking,
//! and its spans satisfy the invariants in `common::check_document`.
//!
//! Ignored by default; run with
//! `cargo test --release --test exhaustive -- --ignored`.

mod common;

use common::check_document;
use restir::{parse, render_html};

const ALPHABET: &[char] = &[
    ' ', '\n', '\t', '*', '`', ':', '_', '-', '=', '.', '#', '1', 'a', '\\', '<', '>', '」',
];

#[test]
#[ignore = "slow; CI runs it in release mode"]
fn every_short_string_over_the_alphabet() {
    let max_len: usize = std::env::var("RESTIR_EXHAUSTIVE_LEN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let mut digits = Vec::with_capacity(max_len);
    let mut src = String::new();
    let mut checked: u64 = 0;
    loop {
        src.clear();
        src.extend(digits.iter().map(|&d| ALPHABET[d]));
        let doc = parse(&src);
        check_document(&doc);
        render_html(&doc);
        checked += 1;
        if !next(&mut digits, max_len) {
            break;
        }
    }
    let expected: u64 = (0..=max_len as u32)
        .map(|n| (ALPHABET.len() as u64).pow(n))
        .sum();
    assert_eq!(checked, expected);
}

/// Advances `digits` to the next string in length-then-lexicographic order.
fn next(digits: &mut Vec<usize>, max_len: usize) -> bool {
    for d in digits.iter_mut().rev() {
        *d += 1;
        if *d < ALPHABET.len() {
            return true;
        }
        *d = 0;
    }
    if digits.len() == max_len {
        return false;
    }
    digits.push(0);
    true
}
