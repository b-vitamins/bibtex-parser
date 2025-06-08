use bibtex_parser::{Database, EntryType};
use pretty_assertions::assert_eq;

#[test]
fn test_parse_simple_file() {
    let input = include_str!("fixtures/simple.bib");
    let db = Database::parse(input).unwrap();

    assert_eq!(db.entries().len(), 2);
    assert_eq!(db.strings().len(), 2);

    // Check first entry
    let entry = &db.entries()[0];
    assert_eq!(entry.key(), "einstein1905");
    assert_eq!(entry.entry_type(), &EntryType::Article);
    assert_eq!(entry.get("author"), Some("Albert Einstein"));
    assert_eq!(
        entry.get("title"),
        Some("Zur Elektrodynamik bewegter Körper")
    );
    assert_eq!(entry.get("journal"), Some("Annalen der Physik"));
    assert_eq!(entry.get_as_string("year"), Some("1905".to_string()));

    // Check string expansion
    let entry2 = &db.entries()[1];
    assert_eq!(entry2.get("author"), Some("Donald E. Knuth"));
}

#[test]
fn test_parse_complex_file() {
    let input = include_str!("fixtures/complex.bib");
    let db = Database::parse(input).unwrap();

    // Should handle various entry types
    let articles = db.find_by_type("article");
    let books = db.find_by_type("book");
    let misc = db.find_by_type("misc");

    assert!(!articles.is_empty());
    assert!(!books.is_empty());
    assert!(!misc.is_empty());

    // Check preambles
    assert!(!db.preambles().is_empty());

    // Check comments
    assert!(!db.comments().is_empty());
}

#[test]
fn test_malformed_file_errors() {
    let input = include_str!("fixtures/malformed.bib");
    let result = Database::parse(input);

    assert!(result.is_err());

    match result {
        Err(e) => {
            // Should provide helpful error message
            let error_msg = e.to_string();
            assert!(error_msg.contains("Parse error"));
            assert!(error_msg.contains("line"));
            assert!(error_msg.contains("column"));
        }
        Ok(_) => panic!("Expected parse error"),
    }
}

#[test]
fn test_round_trip() {
    let original = r#"@article{test2023,
  author = "John Doe",
  title = "Test Article",
  year = 2023
}"#;

    let db = Database::parse(original).unwrap();
    let output = bibtex_parser::to_string(&db).unwrap();

    // Parse the output again
    let db2 = Database::parse(&output).unwrap();

    // Should have same content
    assert_eq!(db.entries().len(), db2.entries().len());
    assert_eq!(
        db.entries()[0].get("author"),
        db2.entries()[0].get("author")
    );
}

#[test]
fn test_variable_expansion() {
    let input = r#"
        @string{me = "John Doe"}
        @string{inst = "MIT"}
        @string{full = me # ", " # inst}
        
        @article{test,
            author = full,
            title = "Test"
        }
    "#;

    let db = Database::parse(input).unwrap();
    let entry = &db.entries()[0];

    assert_eq!(entry.get("author"), Some("John Doe, MIT"));
}

#[test]
fn test_case_insensitive_entry_types() {
    let input = r#"
        @ARTICLE{test1, title = "Test 1"}
        @Article{test2, title = "Test 2"}
        @ArTiClE{test3, title = "Test 3"}
    "#;

    let db = Database::parse(input).unwrap();
    assert_eq!(db.entries().len(), 3);

    for entry in db.entries() {
        assert_eq!(entry.entry_type(), &EntryType::Article);
    }
}

#[test]
fn test_find_by_field() {
    let input = r#"
        @article{einstein1905, author = "Einstein", year = 1905}
        @article{einstein1915, author = "Einstein", year = 1915}
        @article{bohr1913, author = "Bohr", year = 1913}
    "#;

    let db = Database::parse(input).unwrap();

    let einstein_papers = db.find_by_field("author", "Einstein");
    assert_eq!(einstein_papers.len(), 2);

    let papers_1905 = db.find_by_field("year", "1905");
    assert_eq!(papers_1905.len(), 1);
    assert_eq!(papers_1905[0].key(), "einstein1905");
}
