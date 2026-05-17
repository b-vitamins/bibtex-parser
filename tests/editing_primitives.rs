use bibtex_parser::{
    document_to_string, selected_entries_to_string, EntryType, Library, Parser, Value,
};
use std::borrow::Cow;

#[test]
fn key_type_field_and_value_edits_preserve_unrelated_raw_text() {
    let input = "@Article{old_key,\n  TITLE = \"Old\",\n  year = 2024\n}";
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let entry = &mut document.entries_mut()[0];

    entry.rename_key("new_key");
    entry.set_entry_type(EntryType::Book);
    assert_eq!(entry.rename_field("TITLE", "title"), 1);
    assert!(entry.replace_field_value("title", Value::from_plain_string("New")));

    let output = document_to_string(&document).unwrap();
    assert_eq!(output, "@book{new_key,\n  title = {New},\n  year = 2024\n}");
    assert!(Library::parse(&output).is_ok());
}

#[test]
fn duplicate_field_occurrences_are_addressable_without_collapsing() {
    let input = "@article{paper,\n  tag = \"a\",\n  tag = \"b\",\n  tag = \"c\"\n}";
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();
    assert!(document.entries_mut()[0].replace_field_value_at(
        "tag",
        1,
        Value::from_plain_string("beta")
    ));

    let output = document_to_string(&document).unwrap();
    assert_eq!(
        output,
        "@article{paper,\n  tag = \"a\",\n  tag = {beta},\n  tag = \"c\"\n}"
    );

    assert!(document.entries_mut()[0].remove_field_at("tag", 0));
    let output = document_to_string(&document).unwrap();
    assert!(output.contains("tag = {beta}"));
    assert!(output.contains("tag = \"c\""));
    assert!(!output.contains("tag = {a}"));
    assert_eq!(
        Library::parse(&output).unwrap().entries()[0].fields.len(),
        2
    );
}

#[test]
fn add_and_remove_fields_write_a_valid_structured_entry() {
    let input = "@article{paper, title = \"Fast\", file = \"local.pdf\"}";
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let entry = &mut document.entries_mut()[0];

    entry.add_field("note", Value::Literal(Cow::Borrowed("accepted")));
    assert_eq!(entry.remove_field("file"), 1);

    let output = document_to_string(&document).unwrap();
    let reparsed = Library::parse(&output).unwrap();
    let entry = &reparsed.entries()[0];
    assert_eq!(entry.get("title"), Some("Fast"));
    assert_eq!(entry.get("note"), Some("accepted"));
    assert!(entry.field("file").is_none());
}

#[test]
fn configured_export_fields_can_be_removed_across_document() {
    let input = r#"@article{a, title = "A", File = "a.pdf"}
@book{b, title = "B", timestamp = "2026"}"#;
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();

    assert_eq!(document.remove_export_fields(&["file", "timestamp"]), 2);

    let output = document_to_string(&document).unwrap();
    let reparsed = Library::parse(&output).unwrap();
    assert!(reparsed.entries()[0].field_ignore_case("file").is_none());
    assert!(reparsed.entries()[1]
        .field_ignore_case("timestamp")
        .is_none());
}

#[test]
fn document_level_key_rename_finds_entries_by_key() {
    let input = "@article{old, title = \"A\"}";
    let mut document = Parser::new().preserve_raw().parse_document(input).unwrap();

    assert!(document.rename_key("old", "new"));
    assert!(document.entry_mut_by_key("new").is_some());

    let output = document_to_string(&document).unwrap();
    assert!(output.starts_with("@article{new,"));
    assert!(Library::parse(&output)
        .unwrap()
        .find_by_key("new")
        .is_some());
}

#[test]
fn selected_entries_are_serialized_in_source_order() {
    let input = r#"@article{a, title = "A"}
@comment{skip}
@book{b, title = "B"}
@misc{c, title = "C"}"#;
    let document = Parser::new().preserve_raw().parse_document(input).unwrap();

    let output = selected_entries_to_string(&document, &["c", "a"]).unwrap();
    assert_eq!(
        output,
        r#"@article{a, title = "A"}
@misc{c, title = "C"}"#
    );
}
