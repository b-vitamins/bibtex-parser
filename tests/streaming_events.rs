use bibtex_parser::{
    ParseEvent, ParseFlow, ParseStatus, ParsedEntryStatus, Parser, Value, ValueDelimiter,
};

fn collect_events<'a>(parser: &Parser, input: &'a str) -> Vec<ParseEvent<'a>> {
    let mut events = Vec::new();
    parser
        .parse_events(input, |event| {
            events.push(event);
            Ok(ParseFlow::Continue)
        })
        .unwrap();
    events
}

#[test]
fn streaming_events_follow_collected_document_source_order() {
    let input =
        "@string{venue = \"VLDB\"}\n@preamble{\"prefix\"}\n% keep\n@article{paper, title = venue}";
    let parser = Parser::new().preserve_raw();
    let events = collect_events(&parser, input);
    let document = parser.parse_document(input).unwrap();

    assert_eq!(events.len(), document.blocks().len());
    assert!(matches!(events[0], ParseEvent::String(_)));
    assert!(matches!(events[1], ParseEvent::Preamble(_)));
    assert!(matches!(events[2], ParseEvent::Comment(_)));
    assert!(matches!(events[3], ParseEvent::Entry(_)));

    let ParseEvent::Entry(entry) = &events[3] else {
        panic!("expected entry event");
    };
    assert_eq!(entry.key(), "paper");
    assert_eq!(entry.raw.as_deref(), Some("@article{paper, title = venue}"));
    assert_eq!(entry.fields[0].value.delimiter, Some(ValueDelimiter::Bare));
    assert!(matches!(entry.fields[0].value.value, Value::Variable(_)));
    assert_eq!(entry, &document.entries()[0]);
}

#[test]
fn streaming_summary_counts_source_order_blocks() {
    let input = "@string{s = \"S\"}\n@article{a, title = s}\n@book{b, title = \"B\"}";
    let mut keys = Vec::new();
    let summary = Parser::new()
        .parse_events(input, |event| {
            if let ParseEvent::Entry(entry) = event {
                keys.push(entry.key().to_string());
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(keys, ["a", "b"]);
    assert_eq!(summary.status, ParseStatus::Ok);
    assert_eq!(summary.entries, 2);
    assert_eq!(summary.strings, 1);
    assert_eq!(summary.failed_blocks, 0);
    assert!(!summary.stopped);
}

#[test]
fn tolerant_streaming_emits_failed_blocks_and_diagnostics() {
    let input = "@article{bad title = \"Missing comma\"}\n@book{ok, title = \"Recovered\"}";
    let mut events = Vec::new();
    let summary = Parser::new()
        .tolerant()
        .preserve_raw()
        .parse_events(input, |event| {
            events.push(event);
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(summary.status, ParseStatus::Partial);
    assert_eq!(summary.entries, 1);
    assert_eq!(summary.failed_blocks, 1);
    assert_eq!(summary.errors, 1);
    assert!(events
        .iter()
        .any(|event| matches!(event, ParseEvent::Failed(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event, ParseEvent::Diagnostic(_))));
    assert!(events
        .iter()
        .any(|event| { matches!(event, ParseEvent::Entry(entry) if entry.key() == "ok") }));
}

#[test]
fn tolerant_streaming_emits_partial_entries_when_recoverable() {
    let input = "@article{partial, title = \"Recovered\"\n@book{ok, title = \"Next\"}";
    let mut entries = Vec::new();
    let mut diagnostics = 0usize;
    let summary = Parser::new()
        .tolerant()
        .preserve_raw()
        .parse_events(input, |event| {
            match event {
                ParseEvent::Entry(entry) => entries.push(entry),
                ParseEvent::Diagnostic(_) => diagnostics += 1,
                _ => {}
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(summary.entries, 2);
    assert_eq!(summary.recovered_blocks, 1);
    assert_eq!(summary.failed_blocks, 0);
    assert_eq!(diagnostics, 1);
    assert_eq!(entries[0].status, ParsedEntryStatus::Partial);
    assert_eq!(entries[0].key(), "partial");
    assert_eq!(entries[0].fields[0].name, "title");
    assert_eq!(entries[1].key(), "ok");
}

#[test]
fn callback_can_stop_streaming_after_current_event() {
    let input = "@article{a, title = \"A\"}\n@article{b, title = \"B\"}";
    let mut seen = Vec::new();
    let summary = Parser::new()
        .parse_events(input, |event| {
            if let ParseEvent::Entry(entry) = event {
                seen.push(entry.key().to_string());
                return Ok(ParseFlow::Stop);
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(seen, ["a"]);
    assert!(summary.stopped);
    assert_eq!(summary.entries, 1);
}

#[test]
fn strict_streaming_returns_parse_errors() {
    let error = Parser::new()
        .parse_events("@article{bad title = \"Missing comma\"}", |_| {
            Ok(ParseFlow::Continue)
        })
        .unwrap_err();

    assert!(error.to_string().contains("Failed to parse entry"));
}

#[test]
fn named_source_streaming_attaches_source_ids_to_events() {
    let input = "@article{paper, title = \"A\"}";
    let mut source = None;
    let summary = Parser::new()
        .parse_source_events("refs/main.bib", input, |event| {
            if let ParseEvent::Entry(entry) = event {
                source = entry.source;
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(summary.entries, 1);
    assert!(source.is_some_and(|span| span.source.is_some()));
}
