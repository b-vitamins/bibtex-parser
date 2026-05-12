use bibtex_parser::{
    canonical_biblatex_field_alias, classify_resource_field, normalize_biblatex_field_name,
    normalize_field_name_ascii, parse_date_parts, parse_names, DateParseError, DateParts, Library,
    Parser, ResourceKind,
};

#[test]
fn name_helpers_preserve_raw_parsed_and_literal_forms() {
    let names = parse_names(
        "Ludwig van Beethoven and {The Research and Development Group} and Knuth, Jr., Donald E.",
    );

    assert_eq!(names.len(), 3);
    assert_eq!(names[0].raw, "Ludwig van Beethoven");
    assert_eq!(names[0].given, ["Ludwig"]);
    assert_eq!(names[0].prefix, ["van"]);
    assert_eq!(names[0].family, ["Beethoven"]);
    assert_eq!(names[0].display_name(), "Ludwig van Beethoven");

    assert!(names[1].is_literal());
    assert_eq!(
        names[1].literal.as_deref(),
        Some("The Research and Development Group")
    );
    assert_eq!(names[1].last, "The Research and Development Group");
    assert_eq!(
        names[1].display_name(),
        "The Research and Development Group"
    );

    assert_eq!(names[2].given, ["Donald", "E."]);
    assert_eq!(names[2].family, ["Knuth"]);
    assert_eq!(names[2].suffix, ["Jr."]);
    assert_eq!(names[2].display_name(), "Donald E. Knuth, Jr.");
}

#[test]
fn large_author_lists_parse_without_losing_order() {
    let mut input = String::new();
    for index in 0..1_000 {
        if index > 0 {
            input.push_str(" and ");
        }
        input.push_str(&format!("Given{index} Family{index}"));
    }

    let names = parse_names(&input);
    assert_eq!(names.len(), 1_000);
    assert_eq!(names[0].display_name(), "Given0 Family0");
    assert_eq!(names[999].display_name(), "Given999 Family999");
}

#[test]
fn dates_report_complete_partial_and_invalid_cases_explicitly() {
    assert_eq!(
        parse_date_parts("2026").unwrap(),
        DateParts {
            year: 2026,
            month: None,
            day: None
        }
    );
    assert_eq!(
        parse_date_parts("2026-05").unwrap(),
        DateParts {
            year: 2026,
            month: Some(5),
            day: None
        }
    );
    assert_eq!(
        parse_date_parts("2024-02-29").unwrap(),
        DateParts {
            year: 2024,
            month: Some(2),
            day: Some(29)
        }
    );
    assert_eq!(
        parse_date_parts("2023-02-29"),
        Err(DateParseError::InvalidDay)
    );
    assert_eq!(
        parse_date_parts("2026-13"),
        Err(DateParseError::InvalidMonth)
    );
    assert_eq!(
        parse_date_parts("May 2026"),
        Err(DateParseError::InvalidYear)
    );
}

#[test]
fn entry_date_helpers_use_date_fields_and_year_month_fallbacks() {
    let library = Library::parse(
        r#"
        @article{dated, date = "2026-05-13"}
        @article{fallback, year = 2024, month = mar}
        @article{invalid, date = "2026-00"}
        "#,
    )
    .unwrap();

    assert_eq!(
        library.entries()[0].date_parts().unwrap().unwrap(),
        DateParts {
            year: 2026,
            month: Some(5),
            day: Some(13)
        }
    );
    assert_eq!(
        library.entries()[1].date_parts().unwrap().unwrap(),
        DateParts {
            year: 2024,
            month: Some(3),
            day: None
        }
    );
    assert_eq!(
        library.entries()[2].date_parts().unwrap(),
        Err(DateParseError::InvalidMonth)
    );
}

#[test]
fn resource_classification_and_field_normalization_are_stable() {
    assert_eq!(normalize_field_name_ascii(" DOI "), "doi");
    assert_eq!(
        canonical_biblatex_field_alias("journaltitle"),
        Some("journal")
    );
    assert_eq!(normalize_biblatex_field_name("JournalTitle"), "journal");
    assert_eq!(classify_resource_field("PMCID"), Some(ResourceKind::Pmcid));

    let library = Library::parse(
        r#"
        @article{ids,
            doi = "https://doi.org/10.1000/XYZ.",
            url = "https://example.test/paper",
            file = "paper.pdf",
            pmid = "12345",
            pmcid = "pmc12345",
            isbn = "978-0-13-467179-6",
            issn = "1234-567X",
            archiveprefix = "arXiv",
            eprint = "arXiv:2403.12345v2",
            crossref = "parent"
        }
        "#,
    )
    .unwrap();

    let resources = library.entries()[0].resource_fields();
    let kinds = resources
        .iter()
        .map(|resource| resource.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ResourceKind::Doi,
            ResourceKind::Url,
            ResourceKind::File,
            ResourceKind::Pmid,
            ResourceKind::Pmcid,
            ResourceKind::Isbn,
            ResourceKind::Issn,
            ResourceKind::Arxiv,
            ResourceKind::Crossref,
        ]
    );
    assert_eq!(resources[0].normalized.as_deref(), Some("10.1000/xyz"));
    assert_eq!(resources[4].normalized.as_deref(), Some("PMC12345"));
    assert_eq!(resources[5].normalized.as_deref(), Some("9780134671796"));
    assert_eq!(resources[6].normalized.as_deref(), Some("1234567X"));
    assert_eq!(resources[7].normalized.as_deref(), Some("2403.12345v2"));
}

#[test]
fn parsed_entry_helpers_match_library_helpers() {
    let document = Parser::new()
        .preserve_raw()
        .parse_document(
            r#"@article{tooling,
                author = "Jane Doe and {Research Group}",
                translator = "Knuth, Donald E.",
                date = "2026-05",
                doi = "doi:10.5555/ABC"
            }"#,
        )
        .unwrap();
    let entry = &document.entries()[0];

    assert_eq!(entry.authors().len(), 2);
    assert!(entry.authors()[1].is_literal());
    assert_eq!(entry.translators()[0].family, ["Knuth"]);
    assert_eq!(
        entry.date_parts().unwrap().unwrap(),
        DateParts {
            year: 2026,
            month: Some(5),
            day: None
        }
    );
    assert_eq!(entry.doi(), Some("10.5555/abc".to_string()));
    assert_eq!(entry.resource_fields()[0].kind, ResourceKind::Doi);
}
