use bibtex_parser::{ExpansionOptions, Library, Parser, UnresolvedVariablePolicy, Value};
use std::borrow::Cow;

#[test]
fn parsed_document_exposes_raw_parsed_and_requested_expanded_views() {
    let input = r#"
@string{venue = "VLDB"}
@article{paper,
  title = venue # " 2026",
  month = jan,
  note = missing
}
"#;

    let document = Parser::new().preserve_raw().parse_document(input).unwrap();
    let entry = &document.entries()[0];
    let title = &entry.fields[0].value;
    let month = &entry.fields[1].value;
    let note = &entry.fields[2].value;

    assert_eq!(title.raw_text(), Some(r#"venue # " 2026""#));
    assert!(matches!(title.parsed(), Value::Concat(_)));
    assert_eq!(title.expanded_text(), None);
    assert_eq!(
        document
            .expand_value(title.parsed(), ExpansionOptions::default())
            .unwrap(),
        "VLDB 2026"
    );
    assert_eq!(
        document
            .expand_value(month.parsed(), ExpansionOptions::default())
            .unwrap(),
        "January"
    );

    let preserve = ExpansionOptions {
        unresolved_variables: UnresolvedVariablePolicy::Preserve,
        ..ExpansionOptions::default()
    };
    assert_eq!(
        document.expand_value(note.parsed(), preserve).unwrap(),
        "missing"
    );

    let placeholder = ExpansionOptions {
        unresolved_variables: UnresolvedVariablePolicy::Placeholder,
        ..ExpansionOptions::default()
    };
    assert_eq!(
        document.expand_value(note.parsed(), placeholder).unwrap(),
        "{undefined:missing}"
    );
    assert!(document
        .expand_value(note.parsed(), ExpansionOptions::default())
        .is_err());
}

#[test]
fn parser_can_populate_expanded_values_on_request() {
    let input = r#"
@string{venue = "VLDB"}
@article{paper, title = venue # " 2026", month = jan}
"#;

    let document = Parser::new()
        .preserve_raw()
        .expand_values()
        .parse_document(input)
        .unwrap();
    let entry = &document.entries()[0];

    assert_eq!(entry.fields[0].value.expanded_text(), Some("VLDB 2026"));
    assert_eq!(entry.fields[1].value.expanded_text(), Some("January"));
    assert_eq!(entry.fields[0].value.raw_text(), Some(r#"venue # " 2026""#));
}

#[test]
fn public_value_text_projections_cover_core_value_shapes() {
    let literal = Value::Literal(Cow::Borrowed("A Title"));
    let number = Value::Number(2026);
    let variable = Value::Variable(Cow::Borrowed("venue"));
    let concat = Value::Concat(
        vec![
            Value::Variable(Cow::Borrowed("venue")),
            Value::Literal(Cow::Borrowed(" 2026")),
        ]
        .into_boxed_slice(),
    );

    assert_eq!(literal.to_plain_string(), "A Title");
    assert_eq!(number.to_plain_string(), "2026");
    assert_eq!(variable.to_plain_string(), "venue");
    assert_eq!(variable.to_lossy_string(), "{venue}");
    assert_eq!(concat.to_plain_string(), "venue 2026");
    assert_eq!(Value::from_plain_string("plain").as_str(), Some("plain"));
    assert_eq!(concat.to_bibtex_source(), "venue # { 2026}");
}

#[test]
fn library_expansion_defaults_remain_behavior_preserving() {
    let library = Library::parse(
        r#"
@string{venue = "VLDB"}
@article{paper, title = venue, month = jan}
"#,
    )
    .unwrap();
    let entry = &library.entries()[0];

    assert_eq!(entry.get_as_string("title"), Some("VLDB".to_string()));
    assert_eq!(entry.get_as_string("month"), Some("January".to_string()));
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn value_text_can_be_latex_to_unicode_normalized() {
    let value = Value::Literal(Cow::Borrowed(r#"Jos\'e"#));
    assert_eq!(value.to_unicode_plain_string(), "José");
}
