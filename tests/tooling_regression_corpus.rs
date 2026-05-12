use bibtex_parser::{
    document_to_string, parse_bibtex, CorpusSource, DiagnosticCode, EntryType, ParseEvent,
    ParseFlow, ParseStatus, Parser, Value,
};

#[test]
fn regression_fixture_covers_common_bibtex_shapes() {
    let input = format!(
        r#"@string{{conf = "ToolConf"}}
@online{{unicode,
  author = "{}",
  title = "Café {{nested {{braces}}}} and escaped \{{ brace",
  date = "2026-05-13",
  url = "https://example.test",
  month = may
}}
@software{{concat,
  author = "Jane Doe and {{Literal Research Lab}}",
  title = "Part " # "One",
  journaltitle = conf,
  year = 2026
}}"#,
        author_list(64)
    );

    let library = Parser::new().parse(&input).unwrap();
    assert_eq!(library.entries().len(), 2);
    assert_eq!(library.entries()[0].ty, EntryType::Online);
    assert_eq!(library.entries()[1].ty, EntryType::Software);
    assert_eq!(library.entries()[0].authors().len(), 64);
    assert_eq!(
        library.entries()[0].date_parts().unwrap().unwrap().month,
        Some(5)
    );
    assert_eq!(
        library.entries()[0].url().as_deref(),
        Some("https://example.test")
    );

    let raw_items = parse_bibtex(&input).unwrap();
    let concat_entry = raw_items.iter().find_map(|item| {
        if let bibtex_parser::ParsedItem::Entry(entry) = item {
            (entry.key() == "concat").then_some(entry)
        } else {
            None
        }
    });
    let title = concat_entry
        .unwrap()
        .field("title")
        .expect("title field should exist");
    assert!(matches!(title.value, Value::Concat(_)));
}

#[test]
fn malformed_recovery_regression_asserts_diagnostics() {
    let input = r#"@article{bad title = "missing comma"}
@article{partial, title = "Recovered"
@book{valid, title = "After"}"#;

    let document = Parser::new()
        .tolerant()
        .preserve_raw()
        .parse_document(input)
        .unwrap();

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(
        document
            .entries()
            .iter()
            .map(|entry| entry.key())
            .collect::<Vec<_>>(),
        ["partial", "valid"]
    );
    assert_eq!(document.failed_blocks().len(), 1);
    assert!(document
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::MISSING_FIELD_SEPARATOR));
    assert!(document
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.source.is_some()));
}

#[test]
fn parse_modes_agree_on_successful_entries() {
    let input = r#"% comment
@article{a, title = "A", year = 2024}
@book{b, title = {B}, year = 2025}"#;

    let strict = Parser::new().parse(input).unwrap();
    let tolerant = Parser::new().tolerant().parse_document(input).unwrap();
    let preserved = Parser::new().preserve_raw().parse_document(input).unwrap();
    let mut streamed = Vec::new();
    Parser::new()
        .parse_events(input, |event| {
            if let ParseEvent::Entry(entry) = event {
                streamed.push((entry.ty, entry.key.into_owned(), entry.fields.len()));
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    let strict_entries = strict
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.ty.clone(),
                entry.key().to_string(),
                entry.fields().len(),
            )
        })
        .collect::<Vec<_>>();
    let tolerant_entries = tolerant
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.ty.clone(),
                entry.key().to_string(),
                entry.fields.len(),
            )
        })
        .collect::<Vec<_>>();
    let preserved_entries = preserved
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.ty.clone(),
                entry.key().to_string(),
                entry.fields.len(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(strict_entries, tolerant_entries);
    assert_eq!(strict_entries, preserved_entries);
    assert_eq!(strict_entries, streamed);
}

#[test]
fn source_preserving_parse_write_parse_keeps_representative_counts() {
    let input = include_str!("fixtures/complex.bib");
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let output = document_to_string(&document).unwrap();
    let reparsed = Parser::new()
        .preserve_raw()
        .parse_document(&output)
        .unwrap();

    assert_eq!(reparsed.entries().len(), document.entries().len());
    assert_eq!(reparsed.strings().len(), document.strings().len());
    assert_eq!(reparsed.preambles().len(), document.preambles().len());
    assert_eq!(reparsed.comments().len(), document.comments().len());
}

#[test]
fn practical_large_file_streaming_and_corpus_regression() {
    let input = synthetic_entries(2_000);
    let strict = Parser::new().parse(&input).unwrap();
    let mut streamed = 0usize;
    Parser::new()
        .parse_events(&input, |event| {
            if matches!(event, ParseEvent::Entry(_)) {
                streamed += 1;
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    let first_half = synthetic_entries(1_000);
    let second_half = synthetic_entries_from(1_000, 1_000);
    let sources = [
        CorpusSource::new("a.bib", &first_half),
        CorpusSource::new("b.bib", &second_half),
    ];
    let corpus = Parser::new().parse_sources(&sources).unwrap();

    assert_eq!(strict.entries().len(), 2_000);
    assert_eq!(streamed, 2_000);
    assert_eq!(corpus.entries().count(), 2_000);
    assert!(corpus.duplicate_keys().is_empty());
}

#[test]
fn huge_author_list_regression() {
    let input = format!(
        "@article{{many, author = \"{}\", title = \"Many\", year = 2026}}",
        author_list(2_000)
    );
    let library = Parser::new().parse(&input).unwrap();

    let authors = library.entries()[0].authors();
    assert_eq!(authors.len(), 2_000);
    assert_eq!(authors[0].display_name(), "Given0 Family0");
    assert_eq!(authors[1_999].display_name(), "Given1999 Family1999");
}

#[test]
#[ignore = "release stress: set aside for explicit pre-release scale verification"]
fn release_stress_hundred_thousand_entries() {
    let input = synthetic_entries(100_000);
    let library = Parser::new().parse(&input).unwrap();
    assert_eq!(library.entries().len(), 100_000);
}

#[test]
#[ignore = "release stress: set aside for explicit pre-release scale verification"]
fn release_stress_million_entry_streaming() {
    let input = synthetic_entries(1_000_000);
    let mut entries = 0usize;
    Parser::new()
        .parse_events(&input, |event| {
            if matches!(event, ParseEvent::Entry(_)) {
                entries += 1;
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();
    assert_eq!(entries, 1_000_000);
}

fn synthetic_entries(count: usize) -> String {
    synthetic_entries_from(0, count)
}

fn synthetic_entries_from(start: usize, count: usize) -> String {
    let mut input = String::with_capacity(count * 56);
    for index in start..start + count {
        input.push_str("@article{key");
        input.push_str(&index.to_string());
        input.push_str(", title = \"Title ");
        input.push_str(&index.to_string());
        input.push_str("\", year = 2026}\n");
    }
    input
}

fn author_list(count: usize) -> String {
    let mut authors = String::with_capacity(count * 24);
    for index in 0..count {
        if index > 0 {
            authors.push_str(" and ");
        }
        authors.push_str("Given");
        authors.push_str(&index.to_string());
        authors.push_str(" Family");
        authors.push_str(&index.to_string());
    }
    authors
}
