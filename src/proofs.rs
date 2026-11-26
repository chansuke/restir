//! Kani proofs for the kernels that do index arithmetic on raw lines.
//!
//! Each harness covers every input up to `N` bytes over a small alphabet of
//! the bytes the kernel branches on. Whole-parser soundness for short inputs
//! is checked by exhaustive enumeration in `tests/exhaustive.rs`, because
//! bounded model checking of the full parser does not finish in CI time.
//! Run with `cargo kani`.

use crate::block::{column, enumerator, is_adornment, TAB_WIDTH};
use crate::inline::{is_simple_name, split_reference};

fn symbolic<const N: usize>(alphabet: &'static [u8]) -> ([u8; N], usize) {
    let mut bytes = [0u8; N];
    for b in bytes.iter_mut() {
        let i: usize = kani::any();
        kani::assume(i < alphabet.len());
        *b = alphabet[i];
    }
    let len: usize = kani::any();
    kani::assume(len <= N);
    (bytes, len)
}

/// The alphabets are ASCII, so every prefix is valid UTF-8.
fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap()
}

#[kani::proof]
#[kani::unwind(13)]
fn enumerator_matches_its_grammar() {
    let (bytes, len) = symbolic::<11>(b"09#. a");
    let text = as_str(&bytes[..len]);
    if let Some(e) = enumerator(text) {
        let b = text.as_bytes();
        assert!(e.len >= 2 && e.len < b.len());
        assert!(b[e.len - 1] == b'.' && b[e.len] == b' ');
        match e.value {
            None => assert!(e.len == 2 && b[0] == b'#'),
            Some(v) => {
                let mut expected: u64 = 0;
                for &d in &b[..e.len - 1] {
                    assert!(d.is_ascii_digit());
                    expected = expected * 10 + u64::from(d - b'0');
                }
                assert!(u64::from(v) == expected);
            }
        }
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn split_reference_stays_in_bounds() {
    let (bytes, len) = symbolic::<6>(b" <>a");
    let content = as_str(&bytes[..len]);
    let (label_end, target) = split_reference(content);
    assert!(label_end <= content.len());
    if let Some(t) = target {
        assert!(label_end < t.start && t.start < t.end && t.end < content.len());
        let b = content.as_bytes();
        assert!(b[t.start - 1] == b'<' && b[t.end] == b'>' && t.end == content.len() - 1);
    } else {
        assert!(label_end == content.len());
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn adornment_is_one_repeated_punctuation() {
    let (bytes, len) = symbolic::<6>(b"=-* a");
    let text = as_str(&bytes[..len]);
    if is_adornment(text) {
        let b = text.as_bytes();
        assert!(!b.is_empty() && b[0].is_ascii_punctuation());
        assert!(b.iter().all(|&c| c == b[0]));
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn column_expands_tabs_to_stops() {
    let (bytes, len) = symbolic::<6>(b" \ta");
    let raw = as_str(&bytes[..len]);
    let cut: usize = kani::any();
    kani::assume(cut <= len);
    let col = column(raw, cut);
    let before = column(raw, cut.saturating_sub(1));
    assert!(col >= cut && col <= cut * TAB_WIDTH);
    if cut > 0 && bytes[cut - 1] == b'\t' {
        assert!(col % TAB_WIDTH == 0 && col > before);
    } else if cut > 0 {
        assert!(col == before + 1);
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn simple_name_has_no_edge_or_double_punctuation() {
    let (bytes, len) = symbolic::<6>(b"a1-:_ ");
    let name = as_str(&bytes[..len]);
    if is_simple_name(name) {
        let b = name.as_bytes();
        assert!(!b.is_empty());
        assert!(b[0].is_ascii_alphanumeric() && b[b.len() - 1].is_ascii_alphanumeric());
        assert!(b
            .windows(2)
            .all(|w| w[0].is_ascii_alphanumeric() || w[1].is_ascii_alphanumeric()));
    }
}
