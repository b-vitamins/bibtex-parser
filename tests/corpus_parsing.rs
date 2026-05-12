use bibtex_parser::{
    CorpusEvent, CorpusSource, ParseEvent, ParseFlow, ParseStatus, Parser, SourceId,
};

#[test]
fn corpus_entries_keep_distinct_source_identity() {
    let sources = [
        CorpusSource::new("refs/a.bib", "@article{a, title = \"A\"}"),
        CorpusSource::new("refs/b.bib", "@book{b, title = \"B\"}"),
    ];

    let corpus = Parser::new()
        .preserve_raw()
        .parse_sources(&sources)
        .unwrap();

    assert_eq!(corpus.status(), ParseStatus::Ok);
    assert_eq!(corpus.sources().len(), 2);
    assert_eq!(corpus.sources()[0].id, SourceId::new(0));
    assert_eq!(corpus.sources()[1].id, SourceId::new(1));

    let entries = corpus.entries().collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].source.unwrap().source, Some(SourceId::new(0)));
    assert_eq!(entries[1].source.unwrap().source, Some(SourceId::new(1)));
    assert_eq!(
        entries[0].raw.as_deref(),
        Some("@article{a, title = \"A\"}")
    );
    assert_eq!(entries[1].raw.as_deref(), Some("@book{b, title = \"B\"}"));
}

#[test]
fn corpus_non_entry_blocks_keep_source_identity() {
    let sources = [
        CorpusSource::new(
            "refs/a.bib",
            "@string{venue = \"VLDB\"}\n@preamble{\"prefix\"}\n@comment{keep}",
        ),
        CorpusSource::new("refs/b.bib", "% note\n@article{b, title = \"B\"}"),
    ];

    let corpus = Parser::new()
        .preserve_raw()
        .parse_sources(&sources)
        .unwrap();

    let first = &corpus.documents()[0];
    assert_eq!(
        first.strings()[0].source.unwrap().source,
        Some(SourceId::new(0))
    );
    assert_eq!(
        first.preambles()[0].source.unwrap().source,
        Some(SourceId::new(0))
    );
    assert_eq!(
        first.comments()[0].source.unwrap().source,
        Some(SourceId::new(0))
    );

    let second = &corpus.documents()[1];
    assert_eq!(
        second.comments()[0].source.unwrap().source,
        Some(SourceId::new(1))
    );
}

#[test]
fn duplicate_keys_distinguish_same_source_and_cross_source_cases() {
    let sources = [
        CorpusSource::new(
            "refs/a.bib",
            r#"@article{same, title = "A1"}
@book{same, title = "A2"}
@misc{cross, title = "A3"}"#,
        ),
        CorpusSource::new("refs/b.bib", r#"@article{cross, title = "B"}"#),
    ];

    let corpus = Parser::new().parse_sources(&sources).unwrap();
    let duplicates = corpus.duplicate_keys();

    assert_eq!(duplicates.len(), 2);
    let cross = duplicates
        .iter()
        .find(|group| group.key == "cross")
        .unwrap();
    assert!(cross.cross_source);
    assert_eq!(
        cross
            .occurrences
            .iter()
            .map(|occurrence| occurrence.source)
            .collect::<Vec<_>>(),
        vec![SourceId::new(0), SourceId::new(1)]
    );

    let same = duplicates.iter().find(|group| group.key == "same").unwrap();
    assert!(same.is_same_source());
    assert!(same
        .occurrences
        .iter()
        .all(|occurrence| occurrence.source == SourceId::new(0)));
}

#[test]
fn corpus_diagnostics_retain_file_identity() {
    let sources = [
        CorpusSource::new("refs/a.bib", "@article{bad title = \"A\"}"),
        CorpusSource::new("refs/b.bib", "@book{bad title = \"B\"}"),
    ];

    let corpus = Parser::new().tolerant().parse_sources(&sources).unwrap();

    let diagnostics = corpus.diagnostics().collect::<Vec<_>>();
    assert_eq!(corpus.status(), ParseStatus::Failed);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].source.unwrap().source,
        Some(SourceId::new(0))
    );
    assert_eq!(
        diagnostics[1].source.unwrap().source,
        Some(SourceId::new(1))
    );
}

#[test]
fn corpus_events_stream_in_corpus_order_with_source_markers() {
    let sources = [
        CorpusSource::new("refs/a.bib", "@article{a, title = \"A\"}"),
        CorpusSource::new("refs/b.bib", "@book{b, title = \"B\"}"),
    ];
    let mut labels = Vec::new();

    let summary = Parser::new()
        .parse_corpus_events(&sources, |event| {
            match event {
                CorpusEvent::SourceStart(source) => {
                    labels.push(format!("start:{}", source.name.unwrap()));
                }
                CorpusEvent::Event { source, event } => {
                    if let ParseEvent::Entry(entry) = *event {
                        labels.push(format!("entry:{}:{}", source.index(), entry.key()));
                    }
                }
                CorpusEvent::SourceEnd(source) => {
                    labels.push(format!("end:{}", source.name.unwrap()));
                }
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(summary.entries, 2);
    assert_eq!(
        labels,
        [
            "start:refs/a.bib",
            "entry:0:a",
            "end:refs/a.bib",
            "start:refs/b.bib",
            "entry:1:b",
            "end:refs/b.bib",
        ]
    );
}

#[test]
fn corpus_event_stream_can_stop_between_files() {
    let sources = [
        CorpusSource::new("refs/a.bib", "@article{a, title = \"A\"}"),
        CorpusSource::new("refs/b.bib", "@book{b, title = \"B\"}"),
    ];
    let mut entries = Vec::new();

    let summary = Parser::new()
        .parse_corpus_events(&sources, |event| {
            if let CorpusEvent::Event { event, .. } = event {
                if let ParseEvent::Entry(entry) = *event {
                    entries.push(entry.key().to_string());
                    return Ok(ParseFlow::Stop);
                }
            }
            Ok(ParseFlow::Continue)
        })
        .unwrap();

    assert_eq!(entries, ["a"]);
    assert!(summary.stopped);
    assert_eq!(summary.entries, 1);
}
