use bibtex_parser::{
    DiagnosticCode, DiagnosticSeverity, DiagnosticTarget, ParseStatus, ParsedBlock, ParsedEntry,
    ParsedEntryStatus, Parser,
};

#[test]
fn parsed_document_preserves_library_relationship_and_block_order() {
    let input = r#"
@string{venue = "VLDB"}
@preamble{"preface"}
% retained comment
@article{paper,
  title = "Example Paper",
  journal = venue,
  year = 2026
}
"#;

    let document = Parser::new()
        .capture_source()
        .parse_document(input)
        .unwrap();

    assert_eq!(document.status(), ParseStatus::Ok);
    assert_eq!(document.library().entries().len(), 1);
    assert_eq!(document.entries().len(), 1);
    assert_eq!(document.strings().len(), 1);
    assert_eq!(document.preambles().len(), 1);
    assert_eq!(document.comments().len(), 1);
    assert_eq!(document.failed_blocks().len(), 0);
    assert_eq!(document.diagnostics().len(), 0);
    assert_eq!(document.sources().len(), 1);
    assert!(document.sources()[0].is_anonymous());

    assert_eq!(
        document.blocks(),
        &[
            ParsedBlock::String(0),
            ParsedBlock::Preamble(0),
            ParsedBlock::Comment(0),
            ParsedBlock::Entry(0),
        ]
    );

    let entry = &document.entries()[0];
    assert_eq!(entry.key(), "paper");
    assert_eq!(entry.status, ParsedEntryStatus::Complete);
    assert!(entry.source.is_some());
    assert!(entry.raw.is_none());
    assert_eq!(entry.fields.len(), 3);
    assert_eq!(entry.fields[0].name, "title");
    assert!(entry.fields[0].raw.is_none());
}

#[test]
fn parsed_document_exposes_failed_blocks_and_diagnostics() {
    let input = r#"
@article{, title = "Missing key"}
@book{ok, title = "Recovered"}
"#;

    let document = Parser::new()
        .tolerant()
        .capture_source()
        .parse_document(input)
        .unwrap();

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(document.library().entries().len(), 1);
    assert_eq!(document.entries()[0].key(), "ok");
    assert_eq!(document.failed_blocks().len(), 1);
    assert_eq!(document.diagnostics().len(), 1);
    assert_eq!(document.failed_blocks()[0].diagnostics.len(), 1);
    assert!(document.failed_blocks()[0].raw.contains("@article{,"));
    assert_eq!(
        document.diagnostics()[0].severity,
        DiagnosticSeverity::Error
    );
    assert_eq!(
        document.diagnostics()[0].code,
        DiagnosticCode::MISSING_ENTRY_KEY
    );
    assert_eq!(
        document.diagnostics()[0].target,
        DiagnosticTarget::FailedBlock(0)
    );
    assert!(document.diagnostics()[0].source.is_some());
}

#[test]
fn parsed_entry_round_trips_to_structured_entry() {
    let input = r#"@article{paper, title = "Example Paper", year = 2026}"#;
    let document = Parser::new().parse_document(input).unwrap();

    let parsed_entry: ParsedEntry<'_> = document.entries()[0].clone();
    let structured = parsed_entry.into_entry();

    assert_eq!(structured.key(), "paper");
    assert_eq!(structured.fields().len(), 2);
    assert_eq!(structured.get("title"), Some("Example Paper"));
}
