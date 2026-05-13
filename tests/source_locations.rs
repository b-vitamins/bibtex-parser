use bibtex_parser::{Parser, SourceId, SourceMap};

#[test]
fn parsed_document_tracks_anonymous_and_named_sources() {
    let input = r#"@article{paper, title = "Example Paper"}"#;

    let anonymous = Parser::new().parse_document(input).unwrap();
    assert_eq!(anonymous.sources()[0].id, SourceId::new(0));
    assert!(anonymous.sources()[0].name.is_none());
    assert_eq!(
        anonymous.entries()[0].source.unwrap().source,
        Some(SourceId::new(0))
    );

    let named = Parser::new().parse_source("refs/main.bib", input).unwrap();
    assert_eq!(named.sources()[0].id, SourceId::new(0));
    assert_eq!(named.sources()[0].name.as_deref(), Some("refs/main.bib"));
    assert_eq!(
        named.entries()[0].source.unwrap().source,
        Some(SourceId::new(0))
    );
}

#[test]
fn entry_and_field_locations_cover_ascii_tokens() {
    let input = "@article{key,\n  title = \"A\",\n  year = 2026\n}";
    let document = Parser::new().parse_document(input).unwrap();
    let entry = &document.entries()[0];
    let title = &entry.fields[0];
    let year = &entry.fields[1];

    let entry_span = entry.source.unwrap();
    assert_eq!(entry_span.byte_start, 0);
    assert_eq!(entry_span.byte_end, input.len());
    assert_eq!((entry_span.line, entry_span.column), (1, 1));
    assert_eq!((entry_span.end_line, entry_span.end_column), (4, 2));

    let entry_type = entry.entry_type_source.unwrap();
    assert_eq!(
        &input[entry_type.byte_start..entry_type.byte_end],
        "article"
    );
    assert_eq!((entry_type.line, entry_type.column), (1, 2));
    assert_eq!((entry_type.end_line, entry_type.end_column), (1, 9));

    let key = entry.key_source.unwrap();
    assert_eq!(&input[key.byte_start..key.byte_end], "key");
    assert_eq!((key.line, key.column), (1, 10));
    assert_eq!((key.end_line, key.end_column), (1, 13));

    let title_name = title.name_source.unwrap();
    assert_eq!(&input[title_name.byte_start..title_name.byte_end], "title");
    assert_eq!((title_name.line, title_name.column), (2, 3));
    assert_eq!((title_name.end_line, title_name.end_column), (2, 8));

    let title_value = title.value_source.unwrap();
    assert_eq!(
        &input[title_value.byte_start..title_value.byte_end],
        "\"A\""
    );
    assert_eq!((title_value.line, title_value.column), (2, 11));
    assert_eq!((title_value.end_line, title_value.end_column), (2, 14));

    let title_field = title.source.unwrap();
    assert_eq!(
        &input[title_field.byte_start..title_field.byte_end],
        "title = \"A\","
    );

    let year_value = year.value_source.unwrap();
    assert_eq!(&input[year_value.byte_start..year_value.byte_end], "2026");
    assert_eq!((year_value.line, year_value.column), (3, 10));
    assert_eq!((year_value.end_line, year_value.end_column), (3, 14));
}

#[test]
fn locations_count_unicode_columns_not_bytes() {
    let input = "@article{paper,\n  title = \"Café\"\n}";
    let document = Parser::new().parse_document(input).unwrap();
    let value = document.entries()[0].fields[0].value_source.unwrap();

    assert_eq!(&input[value.byte_start..value.byte_end], "\"Café\"");
    assert_eq!((value.line, value.column), (2, 11));
    assert_eq!((value.end_line, value.end_column), (2, 17));
    assert_eq!(value.byte_end - value.byte_start, "\"Café\"".len());
}

#[test]
fn multiline_value_locations_end_on_the_final_line() {
    let input = "@article{paper,\n  title = {Line one\n    Line two}\n}";
    let document = Parser::new().parse_document(input).unwrap();
    let value = document.entries()[0].fields[0].value_source.unwrap();

    assert_eq!(
        &input[value.byte_start..value.byte_end],
        "{Line one\n    Line two}"
    );
    assert_eq!((value.line, value.column), (2, 11));
    assert_eq!((value.end_line, value.end_column), (3, 14));
}

#[test]
fn source_map_handles_end_of_file_positions() {
    let input = "α\nbeta";
    let map = SourceMap::new(Some(SourceId::new(7)), Some("unicode.bib".into()), input);
    let span = map.span(0, input.len());

    assert_eq!(span.source, Some(SourceId::new(7)));
    assert_eq!(map.name(), Some("unicode.bib"));
    assert_eq!((span.line, span.column), (1, 1));
    assert_eq!((span.end_line, span.end_column), (2, 5));
    assert_eq!(map.slice(span), Some(input));
}

#[test]
fn diagnostics_carry_named_source_locations_when_available() {
    let input = "@article{bad, title = \"Missing close\"\n@book{ok, title = \"Recovered\"}";
    let document = Parser::new()
        .tolerant()
        .parse_source("broken.bib", input)
        .unwrap();
    let diagnostic = &document.diagnostics()[0];
    let span = diagnostic.source.unwrap();

    assert_eq!(span.source, Some(SourceId::new(0)));
    assert_eq!(document.sources()[0].name.as_deref(), Some("broken.bib"));
    assert_eq!((span.line, span.column), (2, 1));
    assert_eq!(span.byte_start, span.byte_end);
    assert!(diagnostic
        .snippet
        .as_deref()
        .unwrap()
        .contains("@article{bad"));
}
