use bibtex_parser::{
    DiagnosticCode, DiagnosticTarget, ParseStatus, ParsedBlock, ParsedEntryStatus, Parser,
};

#[test]
fn tolerant_document_returns_valid_entries_around_malformed_regions() {
    let input = r#"
@book{before, title = "Before"}
@article{broken, title = "Missing close"
@book{after, title = "After"}
"#;

    let document = Parser::new().tolerant().parse_document(input).unwrap();

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(document.entries().len(), 3);
    assert_eq!(document.entries()[0].key(), "before");
    assert_eq!(document.entries()[1].key(), "broken");
    assert_eq!(document.entries()[1].status, ParsedEntryStatus::Partial);
    assert_eq!(document.entries()[2].key(), "after");
    assert_eq!(document.failed_blocks().len(), 0);
    assert_eq!(document.diagnostics()[0].target, DiagnosticTarget::Entry(1));
}

#[test]
fn tolerant_document_returns_partial_entries_with_recovered_fields() {
    let input = r#"
@article{partial,
  title = "Good",
  year =
}
@book{after, title = "After"}
"#;

    let document = Parser::new().tolerant().parse_document(input).unwrap();
    let summary = document.summary();

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(summary.recovered_blocks, 1);
    assert_eq!(summary.failed_blocks, 0);
    assert_eq!(document.entries().len(), 2);

    let partial = &document.entries()[0];
    assert_eq!(partial.key(), "partial");
    assert_eq!(partial.status, ParsedEntryStatus::Partial);
    assert_eq!(partial.fields.len(), 1);
    assert_eq!(partial.fields[0].name, "title");
    assert_eq!(partial.fields[0].value.value.as_str(), Some("Good"));
    assert_eq!(
        partial.diagnostics[0].code,
        DiagnosticCode::EMPTY_FIELD_VALUE
    );
    assert_eq!(partial.diagnostics[0].target, DiagnosticTarget::Entry(0));

    assert_eq!(document.entries()[1].key(), "after");
    assert_eq!(
        document.blocks(),
        &[ParsedBlock::Entry(0), ParsedBlock::Entry(1)]
    );
}

#[test]
fn tolerant_document_keeps_unrecoverable_blocks_failed() {
    let input = r#"
@article{, title = "No key"}
@book{after, title = "After"}
"#;

    let document = Parser::new().tolerant().parse_document(input).unwrap();

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(document.entries().len(), 1);
    assert_eq!(document.entries()[0].key(), "after");
    assert_eq!(document.failed_blocks().len(), 1);
    assert!(document.failed_blocks()[0].raw.contains("@article{,"));
    assert_eq!(
        document.failed_blocks()[0].diagnostics[0].code,
        DiagnosticCode::MISSING_ENTRY_KEY
    );
}

#[test]
fn strict_parsing_still_rejects_malformed_entries() {
    let input = r#"@article{partial, title = "Good", year = }"#;

    assert!(Parser::new().parse(input).is_err());
    let strict_document = Parser::new().parse_document(input).unwrap();
    assert_eq!(strict_document.status(), ParseStatus::Failed);

    let tolerant_document = Parser::new().tolerant().parse_document(input).unwrap();
    assert_eq!(tolerant_document.status(), ParseStatus::Partial);
    assert_eq!(
        tolerant_document.entries()[0].status,
        ParsedEntryStatus::Partial
    );
}
