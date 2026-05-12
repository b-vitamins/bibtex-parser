use bibtex_parser::{EntryDelimiter, ParsedBlock, Parser, ValueDelimiter};

#[test]
fn raw_preservation_is_opt_in() {
    let input = r#"@article{paper, title = "Fast"}"#;

    let plain = Parser::new().parse_document(input).unwrap();
    assert!(plain.entries()[0].raw.is_none());
    assert!(plain.entries()[0].fields[0].raw.is_none());
    assert!(plain.entries()[0].fields[0].value.raw.is_none());

    let preserved = Parser::new().preserve_raw().parse_document(input).unwrap();
    assert_eq!(preserved.entries()[0].raw.as_deref(), Some(input));
    assert_eq!(
        preserved.entries()[0].fields[0].raw.as_deref(),
        Some("title = \"Fast\"")
    );
    assert_eq!(
        preserved.entries()[0].fields[0].value.raw.as_deref(),
        Some("\"Fast\"")
    );
}

#[test]
fn raw_entry_field_and_value_text_are_exact_source_slices() {
    let input = "@custom(entry-key,\n  quoted = \"A, B\",\n  braced = {A {Nested} Value},\n  bare = jan,\n  number = 2026,\n  concat = \"A\" # mid # {C},\n)";
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let entry = &document.entries()[0];

    assert_eq!(entry.raw.as_deref(), Some(input));
    assert_eq!(entry.delimiter, Some(EntryDelimiter::Parentheses));

    let expected = [
        ("quoted = \"A, B\",", "\"A, B\"", ValueDelimiter::Quotes),
        (
            "braced = {A {Nested} Value},",
            "{A {Nested} Value}",
            ValueDelimiter::Braces,
        ),
        ("bare = jan,", "jan", ValueDelimiter::Bare),
        ("number = 2026,", "2026", ValueDelimiter::Bare),
        (
            "concat = \"A\" # mid # {C},",
            "\"A\" # mid # {C}",
            ValueDelimiter::Concatenation,
        ),
    ];

    for (field, (raw_field, raw_value, delimiter)) in entry.fields.iter().zip(expected) {
        assert_eq!(field.raw.as_deref(), Some(raw_field));
        assert_eq!(field.value.raw.as_deref(), Some(raw_value));
        assert_eq!(field.value.delimiter, Some(delimiter));
    }
}

#[test]
fn non_entry_blocks_preserve_raw_text_and_source_order() {
    let input = "@string{venue = \"VLDB\"}\n@preamble{\"prefix\" # venue}\n% keep me\n@comment{formal}\n@article{paper, title = venue}";
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();

    assert_eq!(
        document.blocks(),
        &[
            ParsedBlock::String(0),
            ParsedBlock::Preamble(0),
            ParsedBlock::Comment(0),
            ParsedBlock::Comment(1),
            ParsedBlock::Entry(0),
        ]
    );
    assert_eq!(
        document.strings()[0].raw.as_deref(),
        Some("@string{venue = \"VLDB\"}")
    );
    assert_eq!(document.strings()[0].value.raw.as_deref(), Some("\"VLDB\""));
    assert_eq!(
        document.preambles()[0].raw.as_deref(),
        Some("@preamble{\"prefix\" # venue}")
    );
    assert_eq!(
        document.preambles()[0].value.raw.as_deref(),
        Some("\"prefix\" # venue")
    );
    assert_eq!(document.comments()[0].raw.as_deref(), Some("% keep me\n"));
    assert_eq!(
        document.comments()[1].raw.as_deref(),
        Some("@comment{formal}")
    );
}

#[test]
fn failed_blocks_always_retain_raw_text() {
    let input = "@article{, title = \"Missing key\"}";
    let document = Parser::new()
        .tolerant()
        .preserve_raw()
        .parse_document(input)
        .unwrap();

    assert_eq!(document.failed_blocks()[0].raw.as_ref(), input);
}
