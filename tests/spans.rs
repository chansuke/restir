//! The span invariants over every corpus document, in full.

mod common;

use common::{parse_checked, CORPUS};

#[test]
fn corpus_span_invariants() {
    for doc in CORPUS {
        parse_checked(doc);
    }
}

#[test]
fn every_corpus_document_parses_to_something_or_is_blank() {
    for doc in CORPUS {
        let parsed = parse_checked(doc);
        assert_eq!(parsed.blocks.is_empty(), doc.trim().is_empty(), "{doc:?}");
    }
}
