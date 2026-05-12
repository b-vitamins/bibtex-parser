use bibtex_parser::{
    document_to_string, EntryType, Field, Library, Parser, RawWriteMode, TrailingComma, Value,
    Writer, WriterConfig,
};
use std::borrow::Cow;

#[test]
fn unchanged_raw_backed_document_round_trips_blocks_byte_for_byte() {
    let input = "@string{venue = \"VLDB\"}\n@comment{keep}\n@article{paper, title = venue}";
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();

    assert_eq!(document_to_string(&document).unwrap(), input);
}

#[test]
fn one_field_edit_preserves_surrounding_entry_text() {
    let input = "@article{paper,\n  title = \"Old\",\n  year = 2024\n}";
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let title = &mut document.entries_mut()[0].fields[0];

    title.value.value = Value::Literal(Cow::Borrowed("New"));
    title.value.raw = None;

    let output = document_to_string(&document).unwrap();
    assert_eq!(
        output,
        "@article{paper,\n  title = {New},\n  year = 2024\n}"
    );
    assert!(Library::parse(&output).is_ok());
}

#[test]
fn normalized_document_output_is_explicitly_configured() {
    let input = "@article{paper,title=\"Fast\",year=2026}";
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let mut output = Vec::new();
    let config = WriterConfig {
        raw_write_mode: RawWriteMode::Normalize,
        trailing_comma: TrailingComma::Always,
        ..WriterConfig::default()
    };

    Writer::with_config(&mut output, config)
        .write_document(&document)
        .unwrap();
    let output = String::from_utf8(output).unwrap();

    assert_ne!(output, input);
    assert!(output.contains("title = {Fast},"));
    assert!(output.contains("year = 2026,"));
    assert!(Library::parse(&output).is_ok());
}

#[test]
fn parse_write_parse_preserves_counts_for_unchanged_documents() {
    let input = "@string{venue = \"VLDB\"}\n@preamble{\"prefix\"}\n@comment{keep}\n@article{paper, title = venue}";
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
fn library_writer_still_writes_programmatic_entries() {
    let mut library = Library::new();
    library.add_entry(bibtex_parser::Entry {
        ty: EntryType::Article,
        key: Cow::Borrowed("paper"),
        fields: vec![Field::new("title", Value::Literal(Cow::Borrowed("Fast")))],
    });

    let output = bibtex_parser::to_string(&library).unwrap();
    assert!(output.contains("@article{paper,"));
}
