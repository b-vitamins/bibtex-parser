use bibtex_parser::{DiagnosticCode, DiagnosticSeverity, DiagnosticTarget, ParseStatus, Parser};

fn first_code(input: &str) -> DiagnosticCode {
    let document = Parser::new().tolerant().parse_document(input).unwrap();
    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(
        document.diagnostics()[0].target,
        DiagnosticTarget::FailedBlock(0)
    );
    document.diagnostics()[0].code.clone()
}

#[test]
fn diagnostic_code_inventory_is_stable() {
    let ok = "\n@book{ok, title = \"Recovered\"}";
    let cases = [
        (
            format!("@article{{, title = \"A\"}}{ok}"),
            DiagnosticCode::MISSING_ENTRY_KEY,
        ),
        (
            format!("@article{{bad, title \"A\"}}{ok}"),
            DiagnosticCode::MISSING_FIELD_SEPARATOR,
        ),
        (
            format!("@article{{bad, = \"A\"}}{ok}"),
            DiagnosticCode::EXPECTED_FIELD_NAME,
        ),
        (
            format!("@article{{bad, title = , year = 2020}}{ok}"),
            DiagnosticCode::EMPTY_FIELD_VALUE,
        ),
        (
            format!("@article{{bad, title = # \"A\"}}{ok}"),
            DiagnosticCode::EXPECTED_VALUE_ATOM,
        ),
        (
            format!("@article{{bad, title = \"A\" year = 2020}}{ok}"),
            DiagnosticCode::BAD_FIELD_BOUNDARY,
        ),
        (
            format!("@article{{bad, title = \"A\" # , year = 2020}}{ok}"),
            DiagnosticCode::BAD_VALUE_BOUNDARY,
        ),
        (
            format!("@article{{bad, title = \"A\"\n{ok}"),
            DiagnosticCode::UNCLOSED_ENTRY,
        ),
        (
            format!("@article{{bad, title = {{A\n{ok}"),
            DiagnosticCode::UNCLOSED_BRACED_VALUE,
        ),
        (
            format!("@article{{bad, title = \"A\n{ok}"),
            DiagnosticCode::UNCLOSED_QUOTED_VALUE,
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(first_code(&input), expected);
    }
}

#[test]
fn strict_document_parse_returns_failed_status_with_structured_diagnostic() {
    let input = "@article{bad, title = \"A\"";

    assert!(Parser::new().parse(input).is_err());
    let document = Parser::new().parse_document(input).unwrap();
    let summary = document.summary();

    assert_eq!(document.status(), ParseStatus::Failed);
    assert_eq!(summary.status, ParseStatus::Failed);
    assert_eq!(summary.entries, 0);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.failed_blocks, 1);
    assert_eq!(document.blocks().len(), 1);
    assert_eq!(
        document.diagnostics()[0].code,
        DiagnosticCode::UNCLOSED_ENTRY
    );
    assert_eq!(
        document.diagnostics()[0].severity,
        DiagnosticSeverity::Error
    );
    assert!(document.diagnostics()[0].source.is_some());
}

#[test]
fn snippets_cover_beginning_middle_multiline_unicode_and_eof() {
    let beginning = Parser::new()
        .tolerant()
        .parse_document("@article{, title = \"A\"}\n@book{ok, title = \"B\"}")
        .unwrap();
    assert!(beginning.diagnostics()[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("@article"));

    let middle = Parser::new()
        .tolerant()
        .parse_document(
            "@book{good, title = \"B\"}\n@article{bad, title = }\n@book{ok, title = \"B\"}",
        )
        .unwrap();
    assert!(middle.diagnostics()[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("@article{bad"));

    let multiline = Parser::new()
        .tolerant()
        .parse_document(
            "@article{bad,\n  title = {Line one\n    Line two\n@book{ok, title = \"B\"}",
        )
        .unwrap();
    assert!(multiline.diagnostics()[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("Line one"));

    let unicode = Parser::new()
        .tolerant()
        .parse_document("@article{bad, title = \"Café\n@book{ok, title = \"B\"}")
        .unwrap();
    assert!(unicode.diagnostics()[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("Café"));

    let eof = Parser::new()
        .parse_document("@article{bad, title = \"A\"")
        .unwrap();
    assert!(eof.diagnostics()[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("@article{bad"));
}
