use bibtex_parser::{
    normalize_doi, parse_bibtex, parse_names, EntryType, Library, ParsedItem, ValidationError,
    ValidationLevel, ValidationSeverity, Value,
};
use pretty_assertions::assert_eq;
use std::borrow::Cow;

#[test]
fn test_parse_simple_file() {
    let input = include_str!("fixtures/simple.bib");
    let library = Library::parser().parse(input).unwrap();

    assert_eq!(library.entries().len(), 2);
    assert_eq!(library.strings().len(), 2);

    // Check first entry
    let entry = &library.entries()[0];
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
    let entry2 = &library.entries()[1];
    assert_eq!(entry2.get("author"), Some("Donald E. Knuth"));
}

#[test]
fn test_parse_complex_file() {
    let input = include_str!("fixtures/complex.bib");
    let library = Library::parser().parse(input).unwrap();

    // Should handle various entry types
    let articles = library.find_by_type("article");
    let books = library.find_by_type("book");
    let misc = library.find_by_type("misc");

    assert!(!articles.is_empty());
    assert!(!books.is_empty());
    assert!(!misc.is_empty());

    // Check preambles
    assert!(!library.preambles().is_empty());

    // Check comments
    assert!(!library.comments().is_empty());
}

#[test]
fn test_malformed_file_errors() {
    let input = include_str!("fixtures/malformed.bib");
    let result = Library::parser().parse(input);

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

    let library = Library::parser().parse(original).unwrap();
    let output = bibtex_parser::to_string(&library).unwrap();

    // Parse the output again
    let library2 = Library::parser().parse(&output).unwrap();

    // Should have same content
    assert_eq!(library.entries().len(), library2.entries().len());
    assert_eq!(
        library.entries()[0].get("author"),
        library2.entries()[0].get("author")
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

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(entry.get("author"), Some("John Doe, MIT"));
}

#[test]
fn test_case_insensitive_entry_types() {
    let input = r#"
        @ARTICLE{test1, title = "Test 1"}
        @Article{test2, title = "Test 2"}
        @ArTiClE{test3, title = "Test 3"}
    "#;

    let library = Library::parser().parse(input).unwrap();
    assert_eq!(library.entries().len(), 3);

    for entry in library.entries() {
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

    let library = Library::parser().parse(input).unwrap();

    let einstein_papers = library.find_by_field("author", "Einstein");
    assert_eq!(einstein_papers.len(), 2);

    let papers_1905 = library.find_by_field("year", "1905");
    assert_eq!(papers_1905.len(), 1);
    assert_eq!(papers_1905[0].key(), "einstein1905");
}

#[test]
fn test_extended_biblatex_entry_types_and_validation_aliases() {
    let input = r#"
        @online{rust2024,
            title = "Rust",
            url = "https://www.rust-lang.org",
            date = 2024
        }
        @software{parser2026,
            author = "Ada Lovelace",
            title = "Parser Toolkit",
            version = "1.0.0"
        }
        @incollection{chapter2020,
            author = "Grace Hopper",
            title = "Compiler Notes",
            booktitle = "Programming History",
            publisher = "ACM",
            date = 2020
        }
        @article{journal_alias,
            author = "Donald Knuth",
            title = "Literate Programming",
            journaltitle = "The Computer Journal",
            date = 1984
        }
    "#;

    let library = Library::parser().parse(input).unwrap();

    assert_eq!(library.entries()[0].entry_type(), &EntryType::Online);
    assert_eq!(library.entries()[1].entry_type(), &EntryType::Software);
    assert_eq!(library.entries()[2].entry_type(), &EntryType::InCollection);
    assert!(library.entries()[0].entry_type().is_extended());
    assert!(EntryType::Article.is_classic_bibtex());

    for entry in library.entries() {
        assert!(
            entry.validate(ValidationLevel::Minimal).is_ok(),
            "{} should validate with aliases",
            entry.key()
        );
    }

    assert_eq!(
        EntryType::parse("conference"),
        EntryType::InProceedings,
        "classic alias should canonicalize"
    );
}

#[test]
fn test_entry_field_doi_and_name_helpers() {
    let input = r#"
        @article{people2024,
            Author = "John von Neumann and {The Unicode Consortium} and Knuth, Donald E.",
            title = "Names and Identifiers",
            doi = "https://doi.org/10.1000/XYZ.",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert!(entry.field_ignore_case("author").is_some());
    assert_eq!(
        entry.get_any_ignore_case(&["shorttitle", "title"]),
        Some("Names and Identifiers")
    );
    assert_eq!(entry.doi(), Some("10.1000/xyz".to_string()));
    assert_eq!(
        normalize_doi("doi:10.1000/XYZ"),
        Some("10.1000/xyz".to_string())
    );

    let authors = entry.authors();
    assert_eq!(authors.len(), 3);
    assert_eq!(authors[0].first, "John");
    assert_eq!(authors[0].von, "von");
    assert_eq!(authors[0].last, "Neumann");
    assert_eq!(authors[1].last, "The Unicode Consortium");
    assert_eq!(authors[2].first, "Donald E.");
    assert_eq!(authors[2].last, "Knuth");
    assert_eq!(authors[2].display_name(), "Donald E. Knuth");
}

#[test]
fn test_library_case_insensitive_and_doi_search_helpers() {
    let input = r#"
        @article{Smith2024,
            author = "Jane Smith",
            title = "Fast Parsing",
            doi = "10.5555/ABC",
            year = 2024
        }
        @article{smith2024,
            author = "JANE SMITH",
            title = "Fast Parsing Extended",
            doi = "https://doi.org/10.5555/abc",
            year = 2024
        }
        @article{other2024,
            author = "Other Author",
            title = "Other Work",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();

    assert!(library.contains_key("Smith2024"));
    assert_eq!(
        library.find_by_key_ignore_case("SMITH2024").unwrap().key(),
        "Smith2024"
    );
    assert_eq!(
        library
            .find_by_field_ignore_case("AUTHOR", "jane smith")
            .len(),
        2
    );
    assert_eq!(library.find_by_doi("doi:10.5555/ABC").len(), 2);

    let duplicate_keys = library.find_duplicate_keys_ignore_case();
    assert_eq!(duplicate_keys, vec!["smith2024".to_string()]);

    let duplicate_dois = library.find_duplicate_dois();
    assert_eq!(duplicate_dois.len(), 1);
    assert_eq!(duplicate_dois[0].0, "10.5555/abc");
    assert_eq!(duplicate_dois[0].1.len(), 2);
}

#[test]
fn test_parse_names_respects_braced_and_literals() {
    let names = parse_names("Ludwig van Beethoven and {The Research and Development Group}");

    assert_eq!(names.len(), 2);
    assert_eq!(names[0].first, "Ludwig");
    assert_eq!(names[0].von, "van");
    assert_eq!(names[0].last, "Beethoven");
    assert_eq!(names[1].last, "The Research and Development Group");
}

#[test]
fn test_parenthesis_delimiters() {
    let input = r#"
        @string(ieee = "IEEE")
        @string{acm = "ACM"}
        
        @article(test2024,
            author = "Test Author",
            journal = ieee,
            year = 2024
        )

        @book{book2024,
            author = "Book Author",  
            publisher = acm,
            year = 2024
        }

        @preamble("This is a preamble with parentheses")

        @comment(This is a comment with parentheses)
    "#;

    let library = Library::parser().parse(input).unwrap();

    // Verify entries parsed correctly
    assert_eq!(library.entries().len(), 2);

    let article = &library.entries()[0];
    assert_eq!(article.key(), "test2024");
    assert_eq!(article.entry_type(), &EntryType::Article);
    assert_eq!(article.get("author"), Some("Test Author"));
    assert_eq!(article.get("journal"), Some("IEEE")); // String expansion should work
    assert_eq!(article.get_as_string("year"), Some("2024".to_string()));

    let book = &library.entries()[1];
    assert_eq!(book.key(), "book2024");
    assert_eq!(book.entry_type(), &EntryType::Book);
    assert_eq!(book.get("author"), Some("Book Author"));
    assert_eq!(book.get("publisher"), Some("ACM")); // String expansion should work

    // Verify string definitions
    assert_eq!(library.strings().len(), 2);
    assert!(library.string("ieee").is_some());
    assert!(library.string("acm").is_some());

    // Verify preambles
    assert_eq!(library.preambles().len(), 1);

    // Verify comments
    assert_eq!(library.comments().len(), 1);
}

#[test]
fn test_mixed_delimiters_within_entry() {
    // Test that nested delimiters work correctly
    let input = r#"
        @article(mixed2024,
            author = "John Doe",
            title = {A Title with {Nested} Braces},
            note = "A note with (parentheses) in quotes"
        )
    "#;

    let library = Library::parser().parse(input).unwrap();
    assert_eq!(library.entries().len(), 1);

    let entry = &library.entries()[0];
    assert_eq!(entry.key(), "mixed2024");
    assert_eq!(entry.get("author"), Some("John Doe"));
    assert_eq!(entry.get("title"), Some("A Title with {Nested} Braces"));
    assert_eq!(
        entry.get("note"),
        Some("A note with (parentheses) in quotes")
    );
}

#[test]
fn test_multiple_preambles_and_comments() {
    // Test preambles with different delimiter styles
    let input = r#"
        @preamble("This is a preamble with parentheses")
        @preamble{"This is a preamble with braces"}
        
        @comment(This is a comment with parentheses)
        @comment{This is a comment with braces}
    "#;

    let library = Library::parser().parse(input).unwrap();

    // Verify preambles
    assert_eq!(library.preambles().len(), 2);

    // Verify comments
    assert_eq!(library.comments().len(), 2);
}

#[test]
fn test_parenthesis_error_handling() {
    // Test mismatched delimiters
    let input = r#"@article(test, author = "John"})"#;
    let result = Library::parser().parse(input);
    assert!(result.is_err());

    // Test missing closing delimiter
    let input = r#"@article(test, author = "John""#;
    let result = Library::parser().parse(input);
    assert!(result.is_err());
}

#[test]
fn test_month_abbreviations_basic() {
    let input = r#"
        @article{test2024,
            author = "John Doe",
            month = jan,
            year = 2024
        }
    "#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Month abbreviation should expand to full name
    assert_eq!(entry.get("month"), Some("January"));
    assert_eq!(entry.get_as_string("month"), Some("January".to_string()));
}

#[test]
fn test_all_month_abbreviations() {
    let months = [
        ("jan", "January"),
        ("feb", "February"),
        ("mar", "March"),
        ("apr", "April"),
        ("may", "May"),
        ("jun", "June"),
        ("jul", "July"),
        ("aug", "August"),
        ("sep", "September"),
        ("oct", "October"),
        ("nov", "November"),
        ("dec", "December"),
    ];

    for (abbrev, full_name) in &months {
        let input = format!(r#"@article{{test, month = {}}}"#, abbrev);
        let library = Library::parser().parse(&input).unwrap();
        let entry = &library.entries()[0];

        assert_eq!(
            entry.get("month"),
            Some(*full_name),
            "Month abbreviation '{}' should expand to '{}'",
            abbrev,
            full_name
        );
    }
}

#[test]
fn test_month_abbreviations_case_insensitive() {
    let variations = ["jan", "Jan", "JAN", "jAn"];

    for variation in &variations {
        let input = format!(r#"@article{{test, month = {}}}"#, variation);
        let library = Library::parser().parse(&input).unwrap();
        let entry = &library.entries()[0];

        assert_eq!(
            entry.get("month"),
            Some("January"),
            "Month variation '{}' should expand to 'January'",
            variation
        );
    }
}

#[test]
fn test_user_strings_override_month_constants() {
    let input = r#"
        @string{jan = "Custom January"}
        @article{test,
            month = jan
        }
    "#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // User-defined string should override month constant
    assert_eq!(entry.get("month"), Some("Custom January"));
}

#[test]
fn test_month_in_concatenation() {
    let input = r#"
        @article{test,
            note = "Published in " # jan # " 2024"
        }
    "#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Month should expand in concatenation
    assert_eq!(entry.get("note"), Some("Published in January 2024"));
}

#[test]
fn test_month_in_complex_concatenation() {
    let input = r#"
        @string{year = "2024"}
        @article{test,
            note = "Published " # jan # " " # year # " in IEEE"
        }
    "#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Both month constant and user string should work in same concatenation
    assert_eq!(entry.get("note"), Some("Published January 2024 in IEEE"));
}

#[test]
fn test_undefined_variable_with_user_strings_errors() {
    // When user strings exist, all variables are expanded and undefined ones error
    let input = r#"
        @string{defined_var = "test"}
        @article{test,
            publisher = unknown_variable
        }
    "#;

    // Should error for undefined variables when user strings exist
    let result = Library::parser().parse(input);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error
        .to_string()
        .contains("Undefined string variable 'unknown_variable'"));
}

#[test]
fn test_month_with_user_string_precedence() {
    let input = r#"
        @string{feb = "Custom February"}
        @string{mar = "Custom March"}
        
        @article{test1, month = jan}         # Should use month constant
        @article{test2, month = feb}         # Should use user string
        @article{test3, month = mar}         # Should use user string
        @article{test4, month = apr}         # Should use month constant
    "#;

    let library = Library::parser().parse(input).unwrap();

    assert_eq!(library.entries()[0].get("month"), Some("January")); // month constant
    assert_eq!(library.entries()[1].get("month"), Some("Custom February")); // user string
    assert_eq!(library.entries()[2].get("month"), Some("Custom March")); // user string
    assert_eq!(library.entries()[3].get("month"), Some("April")); // month constant
}

#[test]
fn test_month_constants_performance() {
    // Test that month constants don't significantly impact performance
    let input = r#"
        @article{test1, month = jan, title = "Test 1"}
        @article{test2, month = feb, title = "Test 2"}
        @article{test3, month = mar, title = "Test 3"}
        @article{test4, month = apr, title = "Test 4"}
        @article{test5, month = may, title = "Test 5"}
    "#;

    // This should parse quickly without performance regression
    let start = std::time::Instant::now();
    let library = Library::parser().parse(input).unwrap();
    let duration = start.elapsed();

    assert_eq!(library.entries().len(), 5);

    // Should complete in well under 1ms for this small input
    assert!(
        duration.as_millis() < 10,
        "Parsing took too long: {:?}",
        duration
    );

    // Verify all months expanded correctly
    assert_eq!(library.entries()[0].get("month"), Some("January"));
    assert_eq!(library.entries()[1].get("month"), Some("February"));
    assert_eq!(library.entries()[2].get("month"), Some("March"));
    assert_eq!(library.entries()[3].get("month"), Some("April"));
    assert_eq!(library.entries()[4].get("month"), Some("May"));
}

#[test]
fn test_case_insensitive_field_access() {
    let input = r#"
        @article{test2024,
            Author = "John Doe",
            TITLE = "Test Article",
            year = 2024,
            Journal = "Test Journal"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Test case-sensitive access (should match exact case)
    assert_eq!(entry.get("Author"), Some("John Doe"));
    assert_eq!(entry.get("author"), None); // Should not find lowercase
    assert_eq!(entry.get("TITLE"), Some("Test Article"));
    assert_eq!(entry.get("title"), None); // Should not find lowercase
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    assert_eq!(entry.get_as_string("Year"), None); // Should not find capitalized

    // Test case-insensitive access with various casings
    assert_eq!(entry.get_ignore_case("author"), Some("John Doe"));
    assert_eq!(entry.get_ignore_case("AUTHOR"), Some("John Doe"));
    assert_eq!(entry.get_ignore_case("Author"), Some("John Doe"));
    assert_eq!(entry.get_ignore_case("aUtHoR"), Some("John Doe"));

    assert_eq!(entry.get_ignore_case("title"), Some("Test Article"));
    assert_eq!(entry.get_ignore_case("TITLE"), Some("Test Article"));
    assert_eq!(entry.get_ignore_case("Title"), Some("Test Article"));

    assert_eq!(entry.get_ignore_case("journal"), Some("Test Journal"));
    assert_eq!(entry.get_ignore_case("JOURNAL"), Some("Test Journal"));
    assert_eq!(entry.get_ignore_case("Journal"), Some("Test Journal"));

    // Test get_as_string_ignore_case for numbers
    assert_eq!(
        entry.get_as_string_ignore_case("YEAR"),
        Some("2024".to_string())
    );
    assert_eq!(
        entry.get_as_string_ignore_case("Year"),
        Some("2024".to_string())
    );
    assert_eq!(
        entry.get_as_string_ignore_case("year"),
        Some("2024".to_string())
    );

    // Test non-existent field
    assert_eq!(entry.get_ignore_case("nonexistent"), None);
    assert_eq!(entry.get_as_string_ignore_case("nonexistent"), None);
}

#[test]
fn test_case_insensitive_with_string_variables() {
    let input = r#"
        @string{ieee = "IEEE Transactions"}
        @article{test,
            Author = "Jane Doe",
            JOURNAL = ieee
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Variables should work with case-insensitive access
    assert_eq!(
        entry.get_as_string_ignore_case("journal"),
        Some("IEEE Transactions".to_string())
    );
    assert_eq!(
        entry.get_as_string_ignore_case("JOURNAL"),
        Some("IEEE Transactions".to_string())
    );
    assert_eq!(
        entry.get_as_string_ignore_case("Journal"),
        Some("IEEE Transactions".to_string())
    );

    // Case-sensitive should still work for exact match
    assert_eq!(
        entry.get_as_string("JOURNAL"),
        Some("IEEE Transactions".to_string())
    );
    assert_eq!(entry.get_as_string("journal"), None); // No exact match
}

#[test]
fn test_case_insensitive_validation() {
    let input = r#"
        @article{test,
            Author = "Test Author",
            TITLE = "Test Title",
            journal = "Test Journal",
            YEAR = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // The is_valid() method should work with case-insensitive checking
    // since required fields are lowercase but our entry uses mixed case
    assert!(entry.is_valid());
}

#[test]
fn test_case_sensitive_vs_insensitive_performance() {
    let input = r#"
        @article{test,
            Author = "Test Author",
            Title = "Test Title",
            Year = 2024,
            Journal = "Test Journal",
            Pages = "1-10",
            Volume = 42,
            Number = 1,
            Month = "January",
            Note = "Test note",
            Publisher = "Test Publisher"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Measure case-sensitive access time
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = entry.get("Author");
        let _ = entry.get("Title");
        let _ = entry.get("Year");
    }
    let case_sensitive_time = start.elapsed();

    // Measure case-insensitive access time
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = entry.get_ignore_case("author");
        let _ = entry.get_ignore_case("title");
        let _ = entry.get_ignore_case("year");
    }
    let case_insensitive_time = start.elapsed();

    // Case-insensitive should not be significantly slower (within 10x)
    assert!(
        case_insensitive_time <= case_sensitive_time * 10,
        "Case-insensitive access too slow: {:?} vs {:?}",
        case_insensitive_time,
        case_sensitive_time
    );
}

#[test]
fn test_field_name_eq_ignore_case() {
    let input = r#"@article{test, Author = "John Doe"}"#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];
    let field = &entry.fields[0];

    // Test the helper method
    assert!(field.name_eq_ignore_case("author"));
    assert!(field.name_eq_ignore_case("AUTHOR"));
    assert!(field.name_eq_ignore_case("Author"));
    assert!(field.name_eq_ignore_case("aUtHoR"));
    assert!(!field.name_eq_ignore_case("title"));
}

#[test]
fn test_percent_line_comments() {
    let input = r#"
        % This is a line comment at the start
        @article{test2024,
            author = "Test Author",
            title = "Test Title",
            year = 2024
        }
        % Final comment at the end
    "#;

    let library = Library::parser().parse(input).unwrap();
    let comments = library.comments();

    // Should have 2 % comments (% comments inside entries are not valid BibTeX)
    assert!(comments
        .iter()
        .any(|c| c.contains("This is a line comment at the start")));
    assert!(comments
        .iter()
        .any(|c| c.contains("Final comment at the end")));

    // Ensure the entry still parses correctly
    assert_eq!(library.entries().len(), 1);
    let entry = &library.entries()[0];
    assert_eq!(entry.get("author"), Some("Test Author"));
    assert_eq!(entry.get("title"), Some("Test Title"));
}

#[test]
fn test_percent_comment_not_consumed_by_whitespace() {
    let input = "   % Comment with leading whitespace\n@article{test, title=\"Test Title\"}";

    let library = Library::parser().parse(input).unwrap();
    assert_eq!(library.comments().len(), 1);
    assert!(library.comments()[0].contains("Comment with leading whitespace"));
    assert_eq!(library.entries().len(), 1);
}

#[test]
fn test_mixed_comment_types() {
    let input = r#"
        % Line comment
        @comment{Formal comment}
        Random text comment
        @article{test, title="Test"}
    "#;

    let library = Library::parser().parse(input).unwrap();
    assert!(library.comments().len() >= 3);

    // Verify all three types of comments are captured
    let comments = library.comments();
    assert!(comments.iter().any(|c| c.contains("Line comment")));
    assert!(comments.iter().any(|c| c.contains("Formal comment")));
    assert!(comments.iter().any(|c| c.contains("Random text comment")));

    assert_eq!(library.entries().len(), 1);
}

#[test]
fn test_percent_comment_variations() {
    let input = r#"
        % Simple comment
        %Another comment without space
        %
        % Empty comment line above
        @article{test, title="Test"}
        % Comment after entry
    "#;

    let library = Library::parser().parse(input).unwrap();
    let comments = library.comments();

    // Should capture all percent comments including empty ones
    assert!(comments.iter().any(|c| c.contains("Simple comment")));
    assert!(comments
        .iter()
        .any(|c| c.contains("Another comment without space")));
    assert!(comments.iter().any(|c| c.contains("Comment after entry")));

    assert_eq!(library.entries().len(), 1);
}

#[test]
fn test_percent_comment_in_complex_bibtex() {
    // Use the complex fixture which already has a % comment
    let input = include_str!("fixtures/complex.bib");
    let library = Library::parser().parse(input).unwrap();

    // The complex.bib file has "% Another inline comment" on line 38
    let comments = library.comments();
    assert!(comments
        .iter()
        .any(|c| c.contains("Another inline comment")));

    // Should also have the formal @Comment entry
    assert!(comments
        .iter()
        .any(|c| c.contains("This is a formal comment entry")));

    // And the text comment at the beginning
    assert!(comments
        .iter()
        .any(|c| c.contains("This is a comment outside of any entry")));
}

#[test]
fn test_digit_string_fallback() {
    // Test that values starting with digits but containing non-numeric characters
    // are parsed as string literals instead of failing
    let input = r#"
        @article{test1,
            year = 2024,
            volume = 12b,
            issue = 2024a,
            pages = 123-456,
            version = 1.2.3,
            edition = 2nd,
            chapter = 3rd
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    assert_eq!(library.entries().len(), 1);

    let entry = &library.entries()[0];
    assert_eq!(entry.key(), "test1");

    // Pure number should remain as Number type
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "year")
        .unwrap()
        .value
    {
        Value::Number(n) => assert_eq!(n, 2024),
        _ => panic!("Expected Number value for pure number"),
    }

    // Mixed alphanumeric should be Literal strings
    assert_eq!(entry.get("volume"), Some("12b"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "volume")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "12b"),
        _ => panic!("Expected Literal value for mixed alphanumeric"),
    }

    assert_eq!(entry.get("issue"), Some("2024a"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "issue")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "2024a"),
        _ => panic!("Expected Literal value for year with letter"),
    }

    assert_eq!(entry.get("pages"), Some("123-456"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "pages")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "123-456"),
        _ => panic!("Expected Literal value for page range"),
    }

    assert_eq!(entry.get("version"), Some("1.2.3"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "version")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "1.2.3"),
        _ => panic!("Expected Literal value for dotted version"),
    }

    assert_eq!(entry.get("edition"), Some("2nd"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "edition")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "2nd"),
        _ => panic!("Expected Literal value for ordinal"),
    }

    assert_eq!(entry.get("chapter"), Some("3rd"));
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "chapter")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "3rd"),
        _ => panic!("Expected Literal value for ordinal"),
    }
}

#[test]
fn test_mixed_value_types() {
    // Test various value types in a single entry to ensure they coexist properly
    let input = r#"
        @article{mixed,
            year = 2024,
            volume = "12",
            issue = 3rd,
            pages = 100-150,
            version = 1.2.3,
            id = "abc123",
            doi = "10.1000/123",
            isbn = 978-0123456789
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // All should parse successfully and return appropriate string representations
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    assert_eq!(entry.get_as_string("volume"), Some("12".to_string()));
    assert_eq!(entry.get_as_string("issue"), Some("3rd".to_string()));
    assert_eq!(entry.get_as_string("pages"), Some("100-150".to_string()));
    assert_eq!(entry.get_as_string("version"), Some("1.2.3".to_string()));
    assert_eq!(entry.get_as_string("id"), Some("abc123".to_string()));
    assert_eq!(entry.get_as_string("doi"), Some("10.1000/123".to_string()));
    assert_eq!(
        entry.get_as_string("isbn"),
        Some("978-0123456789".to_string())
    );

    // Check specific value types
    // Year should be Number
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "year")
        .unwrap()
        .value
    {
        Value::Number(_) => {}
        _ => panic!("Year should be Number type"),
    }

    // Volume (quoted) should be Literal
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "volume")
        .unwrap()
        .value
    {
        Value::Literal(_) => {}
        _ => panic!("Quoted number should be Literal type"),
    }

    // Issue (ordinal) should be Literal
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "issue")
        .unwrap()
        .value
    {
        Value::Literal(_) => {}
        _ => panic!("Ordinal should be Literal type"),
    }

    // ID (quoted) should be Literal
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "id")
        .unwrap()
        .value
    {
        Value::Literal(_) => {}
        _ => panic!("Quoted identifier should be Literal type"),
    }
}

#[test]
fn test_string_variable_vs_literal_digit() {
    // Test that string variables and digit literals work correctly together
    let input = r#"
        @string{year2024 = "2024"}
        @article{test,
            year = year2024,
            volume = 2024a,
            issue = 12b
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    assert_eq!(entry.get("volume"), Some("2024a"));
    assert_eq!(entry.get("issue"), Some("12b"));

    // Verify the value types
    // year should be expanded to Literal since string variables get expanded during parsing
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "year")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "2024"),
        _ => panic!("Expanded string variable should be Literal type"),
    }

    // volume and issue should be Literals
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "volume")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "2024a"),
        _ => panic!("Digit-string should be Literal type"),
    }

    match entry
        .fields()
        .iter()
        .find(|f| f.name == "issue")
        .unwrap()
        .value
    {
        Value::Literal(ref s) => assert_eq!(s, "12b"),
        _ => panic!("Digit-string should be Literal type"),
    }
}

#[test]
fn test_edge_case_digit_strings() {
    // Test edge cases for digit string parsing
    let input = r#"
        @misc{edge_cases,
            number1 = 1st,
            number2 = 21st,
            number3 = 2nd,
            number4 = 42nd,
            number5 = 3rd,
            number6 = 123rd,
            version1 = 1.0,
            version2 = 2.1.3,
            version3 = 10.15.2.1,
            range1 = 1-10,
            range2 = 100-200,
            range3 = "1-10,15-20",
            mixed1 = 2024Spring,
            mixed2 = 42alpha,
            mixed3 = 1beta2,
            code1 = 123ABC,
            code2 = 456def
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // All should parse as string literals
    let expected_values = [
        ("number1", "1st"),
        ("number2", "21st"),
        ("number3", "2nd"),
        ("number4", "42nd"),
        ("number5", "3rd"),
        ("number6", "123rd"),
        ("version1", "1.0"),
        ("version2", "2.1.3"),
        ("version3", "10.15.2.1"),
        ("range1", "1-10"),
        ("range2", "100-200"),
        ("range3", "1-10,15-20"),
        ("mixed1", "2024Spring"),
        ("mixed2", "42alpha"),
        ("mixed3", "1beta2"),
        ("code1", "123ABC"),
        ("code2", "456def"),
    ];

    for (field_name, expected_value) in expected_values {
        assert_eq!(
            entry.get(field_name),
            Some(expected_value),
            "Field {} should have value '{}'",
            field_name,
            expected_value
        );

        // Verify all are Literal types
        match entry
            .fields()
            .iter()
            .find(|f| f.name == field_name)
            .unwrap()
            .value
        {
            Value::Literal(ref s) => assert_eq!(s, expected_value),
            _ => panic!("Field {} should be Literal type", field_name),
        }
    }
}

#[test]
fn test_digit_string_in_concatenation() {
    // Test that digit strings work in concatenations
    let input = r#"
        @article{concat_test,
            note = "Version " # 1.2.3 # " released",
            pages = 100-200 # ", " # 300-400,
            year = 2024 # "a"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(
        entry.get_as_string("note"),
        Some("Version 1.2.3 released".to_string())
    );
    assert_eq!(
        entry.get_as_string("pages"),
        Some("100-200, 300-400".to_string())
    );
    assert_eq!(entry.get_as_string("year"), Some("2024a".to_string()));

    // Verify concatenation structure
    match entry
        .fields()
        .iter()
        .find(|f| f.name == "note")
        .unwrap()
        .value
    {
        Value::Concat(ref parts) => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], Value::Literal(Cow::Borrowed("Version ")));
            assert_eq!(parts[1], Value::Literal(Cow::Borrowed("1.2.3")));
            assert_eq!(parts[2], Value::Literal(Cow::Borrowed(" released")));
        }
        _ => panic!("Expected concatenated value"),
    }
}

#[test]
fn test_pure_number_values() {
    // Ensure pure numbers parse as numeric values.
    let input = r#"
        @article{numbers,
            year = 2024,
            volume = 42,
            number = 1,
            pages_start = 100,
            pages_end = 200,
            negative = -5,
            zero = 0
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // All should be parsed as Number type
    let number_fields = [
        ("year", 2024),
        ("volume", 42),
        ("number", 1),
        ("pages_start", 100),
        ("pages_end", 200),
        ("negative", -5),
        ("zero", 0),
    ];

    for (field_name, expected_num) in number_fields {
        assert_eq!(
            entry.get_as_string(field_name),
            Some(expected_num.to_string()),
            "Field {} should convert to string '{}'",
            field_name,
            expected_num
        );

        // Verify all are Number types
        match entry
            .fields()
            .iter()
            .find(|f| f.name == field_name)
            .unwrap()
            .value
        {
            Value::Number(n) => assert_eq!(n, expected_num),
            _ => panic!("Field {} should be Number type", field_name),
        }
    }
}

// Raw API Tests - Testing the low-level parse_bibtex function

#[test]
fn test_raw_parse_api_basic() {
    let input = r#"
        @string{ieee = "IEEE"}
        @preamble{"Test preamble"}
        % Line comment
        @comment{Formal comment}
        @article{test2024,
            author = "John Doe",
            title = ieee # " Article",
            year = 2024
        }
    "#;

    let items = parse_bibtex(input).unwrap();

    // Count different item types
    let mut entries = 0;
    let mut strings = 0;
    let mut preambles = 0;
    let mut comments = 0;

    for item in &items {
        match item {
            ParsedItem::Entry(_) => entries += 1,
            ParsedItem::String(_, _) => strings += 1,
            ParsedItem::Preamble(_) => preambles += 1,
            ParsedItem::Comment(_) => comments += 1,
        }
    }

    assert_eq!(entries, 1);
    assert_eq!(strings, 1);
    assert_eq!(preambles, 1);
    assert_eq!(comments, 2); // Line comment + formal comment
}

#[test]
fn test_raw_api_no_expansion() {
    let input = r#"
        @string{name = "John"}
        @article{test, author = name}
    "#;

    let items = parse_bibtex(input).unwrap();

    // Find the entry
    let entry = items
        .iter()
        .find_map(|item| {
            if let ParsedItem::Entry(e) = item {
                Some(e)
            } else {
                None
            }
        })
        .unwrap();

    // The author field should still be a variable reference, not expanded
    let author_field = entry.fields.iter().find(|f| f.name == "author").unwrap();

    match &author_field.value {
        Value::Variable(var_name) => {
            assert_eq!(var_name.as_ref(), "name");
        }
        _ => panic!("Expected variable reference, not expanded value"),
    }
}

#[test]
fn test_raw_api_preserves_structure() {
    let input = r#"
        @article{test,
            title = "Part 1" # " and " # "Part 2",
            year = 2024
        }
    "#;

    let items = parse_bibtex(input).unwrap();
    let entry = match &items[0] {
        ParsedItem::Entry(e) => e,
        _ => panic!("Expected entry"),
    };

    // Check that concatenation is preserved in raw form
    let title_field = entry.fields.iter().find(|f| f.name == "title").unwrap();

    // Should be a Concat value with 3 parts
    match &title_field.value {
        Value::Concat(parts) => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], Value::Literal(Cow::Borrowed("Part 1")));
            assert_eq!(parts[1], Value::Literal(Cow::Borrowed(" and ")));
            assert_eq!(parts[2], Value::Literal(Cow::Borrowed("Part 2")));
        }
        _ => panic!("Expected concatenated value"),
    }
}

#[test]
fn test_raw_api_comment_types() {
    let input = r#"
        % Line comment
        @comment{Formal comment}
        Random text before entry
        @article{test, title = "Test"}
        Another text comment
    "#;

    let items = parse_bibtex(input).unwrap();

    let comments: Vec<&str> = items
        .iter()
        .filter_map(|item| {
            if let ParsedItem::Comment(text) = item {
                Some(*text)
            } else {
                None
            }
        })
        .collect();

    // Should capture all types of comments
    assert!(comments.iter().any(|c| c.contains("Line comment")));
    assert!(comments.iter().any(|c| c.contains("Formal comment")));
    assert!(comments
        .iter()
        .any(|c| c.contains("Random text before entry")));
    assert!(comments.iter().any(|c| c.contains("Another text comment")));

    // Should still parse the entry
    let entries: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let ParsedItem::Entry(e) = item {
                Some(e)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key(), "test");
}

#[test]
fn test_raw_api_string_definitions() {
    let input = r#"
        @string{name = "John Doe"}
        @string{institution = "MIT"}
        @string{full = name # ", " # institution}
        @article{test, author = full}
    "#;

    let items = parse_bibtex(input).unwrap();

    // Find string definitions
    let strings: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let ParsedItem::String(name, value) = item {
                Some((name, value))
            } else {
                None
            }
        })
        .collect();

    assert_eq!(strings.len(), 3);
    assert_eq!(*strings[0].0, "name");
    assert_eq!(*strings[1].0, "institution");
    assert_eq!(*strings[2].0, "full");

    // Check that 'full' string contains concatenation
    match strings[2].1 {
        Value::Concat(ref parts) => {
            assert_eq!(parts.len(), 3);
            // Parts should be variable references and literal
            assert!(matches!(parts[0], Value::Variable(_)));
            assert_eq!(parts[1], Value::Literal(Cow::Borrowed(", ")));
            assert!(matches!(parts[2], Value::Variable(_)));
        }
        _ => panic!("Expected concatenated value for 'full' string"),
    }

    // Find the entry and verify it has unexpanded variable reference
    let entry = items
        .iter()
        .find_map(|item| {
            if let ParsedItem::Entry(e) = item {
                Some(e)
            } else {
                None
            }
        })
        .unwrap();

    let author_field = entry.fields.iter().find(|f| f.name == "author").unwrap();

    match &author_field.value {
        Value::Variable(var_name) => {
            assert_eq!(var_name.as_ref(), "full");
        }
        _ => panic!("Expected variable reference in entry"),
    }
}

#[test]
fn test_raw_api_vs_library_api() {
    let input = r#"
        @string{conference = "VLDB"}
        @article{test,
            title = "Library " # conference,
            year = 2024
        }
    "#;

    // Parse with raw API
    let raw_items = parse_bibtex(input).unwrap();

    // Parse with Library API
    let library = Library::parser().parse(input).unwrap();

    // Raw API should have unexpanded variables
    let raw_entry = raw_items
        .iter()
        .find_map(|item| {
            if let ParsedItem::Entry(e) = item {
                Some(e)
            } else {
                None
            }
        })
        .unwrap();

    let raw_title = raw_entry.fields.iter().find(|f| f.name == "title").unwrap();

    match &raw_title.value {
        Value::Concat(parts) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0], Value::Literal(Cow::Borrowed("Library ")));
            assert!(matches!(parts[1], Value::Variable(_)));
        }
        _ => panic!("Expected concatenated value with variable reference"),
    }

    // Library API should have expanded variables
    let library_entry = &library.entries()[0];
    assert_eq!(library_entry.get("title"), Some("Library VLDB"));
}

#[test]
fn test_raw_api_preambles() {
    let input = r#"
        @string{style = "LaTeX style"}
        @preamble{"Basic preamble"}
        @preamble{style # " preamble"}
        @article{test, title = "Test"}
    "#;

    let items = parse_bibtex(input).unwrap();

    let preambles: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let ParsedItem::Preamble(value) = item {
                Some(value)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(preambles.len(), 2);

    // First preamble should be simple literal
    match preambles[0] {
        Value::Literal(ref text) => {
            assert_eq!(text.as_ref(), "Basic preamble");
        }
        _ => panic!("Expected literal preamble"),
    }

    // Second preamble should be concatenation with variable reference
    match preambles[1] {
        Value::Concat(ref parts) => {
            assert_eq!(parts.len(), 2);
            assert!(matches!(parts[0], Value::Variable(_)));
            assert_eq!(parts[1], Value::Literal(Cow::Borrowed(" preamble")));
        }
        _ => panic!("Expected concatenated preamble"),
    }
}

#[test]
fn test_raw_api_maintains_order() {
    let input = r#"
        % Comment 1
        @string{var1 = "Value 1"}
        @preamble{"Preamble 1"}
        @article{entry1, title = "Entry 1"}
        % Comment 2
        @string{var2 = "Value 2"}
        @article{entry2, title = "Entry 2"}
        @preamble{"Preamble 2"}
        % Comment 3
    "#;

    let items = parse_bibtex(input).unwrap();

    // Verify items are in parse order
    assert!(matches!(items[0], ParsedItem::Comment(_))); // Comment 1
    assert!(matches!(items[1], ParsedItem::String(_, _))); // var1
    assert!(matches!(items[2], ParsedItem::Preamble(_))); // Preamble 1
    assert!(matches!(items[3], ParsedItem::Entry(_))); // entry1
    assert!(matches!(items[4], ParsedItem::Comment(_))); // Comment 2
    assert!(matches!(items[5], ParsedItem::String(_, _))); // var2
    assert!(matches!(items[6], ParsedItem::Entry(_))); // entry2
    assert!(matches!(items[7], ParsedItem::Preamble(_))); // Preamble 2
    assert!(matches!(items[8], ParsedItem::Comment(_))); // Comment 3

    // Verify specific content
    if let ParsedItem::String(name, _) = &items[1] {
        assert_eq!(*name, "var1");
    }

    if let ParsedItem::Entry(entry) = &items[3] {
        assert_eq!(entry.key(), "entry1");
    }

    if let ParsedItem::String(name, _) = &items[5] {
        assert_eq!(*name, "var2");
    }

    if let ParsedItem::Entry(entry) = &items[6] {
        assert_eq!(entry.key(), "entry2");
    }
}

#[test]
fn test_raw_api_complex_file() {
    let input = include_str!("fixtures/complex.bib");
    let items = parse_bibtex(input).unwrap();

    // Should parse the complex file without errors
    assert!(!items.is_empty());

    // Count different types
    let mut entry_count = 0;
    let mut string_count = 0;
    let mut preamble_count = 0;
    let mut comment_count = 0;

    for item in &items {
        match item {
            ParsedItem::Entry(_) => entry_count += 1,
            ParsedItem::String(_, _) => string_count += 1,
            ParsedItem::Preamble(_) => preamble_count += 1,
            ParsedItem::Comment(_) => comment_count += 1,
        }
    }

    // Should have various types of items
    assert!(entry_count > 0);
    assert!(string_count > 0);
    assert!(preamble_count > 0);
    assert!(comment_count > 0);

    // Compare with Library API to ensure same parsing capability
    let library = Library::parser().parse(input).unwrap();
    assert_eq!(entry_count, library.entries().len());
    assert_eq!(string_count, library.strings().len());
    assert_eq!(preamble_count, library.preambles().len());
    assert_eq!(comment_count, library.comments().len());
}

#[test]
fn test_raw_api_error_handling() {
    let malformed_input = r#"@article{unclosed, title = "No closing brace""#;

    let result = parse_bibtex(malformed_input);
    assert!(result.is_err());

    // Error should contain helpful information
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(error_msg.contains("Parse error"));
    assert!(error_msg.contains("line"));
    assert!(error_msg.contains("column"));
}

#[test]
fn test_raw_api_month_constants() {
    let input = r#"
        @article{test,
            month = jan,
            year = 2024
        }
    "#;

    let items = parse_bibtex(input).unwrap();
    let entry = items
        .iter()
        .find_map(|item| {
            if let ParsedItem::Entry(e) = item {
                Some(e)
            } else {
                None
            }
        })
        .unwrap();

    // Month should be a variable reference in raw API
    let month_field = entry.fields.iter().find(|f| f.name == "month").unwrap();

    match &month_field.value {
        Value::Variable(var_name) => {
            assert_eq!(var_name.as_ref(), "jan");
        }
        _ => panic!("Expected variable reference for month constant"),
    }

    // Compare with Library API which should expand month constants
    let library = Library::parser().parse(input).unwrap();
    let library_entry = &library.entries()[0];
    assert_eq!(library_entry.get("month"), Some("January"));
}

#[test]
fn test_raw_api_performance() {
    // Test that raw API maintains high performance
    let input = r#"
        @string{conference = "VLDB"}
        @string{year = "2024"}
        @preamble{"Library Conference Proceedings"}
    "#
    .repeat(100)
        + &"@article{test, title = \"Performance Test\", year = 2024}".repeat(1000);

    let start = std::time::Instant::now();
    let items = parse_bibtex(&input).unwrap();
    let duration = start.elapsed();

    // Should complete quickly
    assert!(
        duration.as_millis() < 100,
        "Raw API parsing took too long: {:?}",
        duration
    );

    // Should parse all items
    assert!(!items.is_empty());

    let entry_count = items
        .iter()
        .filter(|item| matches!(item, ParsedItem::Entry(_)))
        .count();
    assert_eq!(entry_count, 1000);
}

#[test]
fn test_parsed_item_debug() {
    let input = r#"@article{test, title = "Debug Test"}"#;
    let items = parse_bibtex(input).unwrap();

    // Verify ParsedItem implements Debug
    let debug_str = format!("{:?}", items[0]);
    assert!(debug_str.contains("Entry"));
    assert!(debug_str.contains("test"));
}

#[test]
fn test_parsed_item_clone() {
    let input = r#"@string{name = "Clone Test"}"#;
    let items = parse_bibtex(input).unwrap();

    // Verify ParsedItem implements Clone
    let cloned_item = items[0].clone();

    match (&items[0], &cloned_item) {
        (ParsedItem::String(name1, _), ParsedItem::String(name2, _)) => {
            assert_eq!(*name1, *name2);
        }
        _ => panic!("Clone failed"),
    }
}

#[test]
fn test_parsed_item_partial_eq() {
    let input = r#"@string{name = "PartialEq Test"}"#;
    let items1 = parse_bibtex(input).unwrap();
    let items2 = parse_bibtex(input).unwrap();

    // Verify ParsedItem implements PartialEq
    assert_eq!(items1[0], items2[0]);
}

// VALIDATION TESTS

#[test]
fn test_validation_levels() {
    let input = r#"
        @article{valid2024,
            author = "John Doe",
            title = "Test Article",
            journal = "Test Journal",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Should pass all validation levels
    assert!(entry.validate(ValidationLevel::Minimal).is_ok());
    assert!(entry.validate(ValidationLevel::Standard).is_ok());
    assert!(entry.validate(ValidationLevel::Strict).is_ok());
}

#[test]
fn test_missing_required_fields() {
    let input = r#"
        @article{incomplete,
            author = "John Doe",
            title = "Test Article"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    let result = entry.validate(ValidationLevel::Minimal);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 2); // Missing journal and year
    assert!(errors
        .iter()
        .any(|e| e.field == Some("journal".to_string())));
    assert!(errors.iter().any(|e| e.field == Some("year".to_string())));

    // All should be error-level
    for error in &errors {
        assert_eq!(error.severity, ValidationSeverity::Error);
    }
}

#[test]
fn test_validation_warnings() {
    let input = r#"
        @article{warnings_test,
            author = "John Doe",
            title = "Test",
            journal = "Journal",
            year = 999,
            pages = "12 to 34",
            doi = "not-a-doi"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    let result = entry.validate(ValidationLevel::Strict);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    let warnings = errors
        .iter()
        .filter(|e| e.severity == ValidationSeverity::Warning)
        .count();
    assert!(warnings >= 3); // Year, pages, DOI warnings
}

#[test]
fn test_minimal_entry_validity_helper() {
    let input = r#"
        @article{valid,
            author = "A", title = "T", journal = "J", year = 2024
        }
        @article{invalid,
            author = "A", title = "T"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    assert!(library.entries()[0].is_valid());
    assert!(!library.entries()[1].is_valid());
}

#[test]
fn test_library_validation() {
    let input = r#"
        @article{valid, author="A", title="T", journal="J", year=2024}
        @article{invalid, author="A", title="T"}
        @article{valid2, author="B", title="T2", journal="J2", year=2024}
    "#;

    let library = Library::parser().parse(input).unwrap();
    let invalid = library.validate(ValidationLevel::Minimal);

    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].0, 1); // Index of invalid entry
    assert_eq!(invalid[0].1.key(), "invalid");
}

#[test]
fn test_duplicate_keys() {
    let input = r#"
        @article{dup, title="First"}
        @article{unique, title="Unique"}  
        @article{dup, title="Second"}
    "#;

    let library = Library::parser().parse(input).unwrap();
    let duplicates = library.find_duplicate_keys();

    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0], "dup");
}

#[test]
fn test_comprehensive_validation_report() {
    let input = r#"
        @article{dup, author="A", title="T", journal="J", year=2024}
        @article{invalid, author="A", title="T"}  
        @article{dup, author="B", title="T2", journal="J2", year=1}
        @misc{empty_entry, title="Empty Entry"}
    "#;

    let library = Library::parser().parse(input).unwrap();
    let report = library.validate_comprehensive(ValidationLevel::Standard);

    assert!(!report.is_valid());
    assert_eq!(report.total_entries, 4);
    assert_eq!(report.duplicate_keys.len(), 1);
    assert_eq!(report.empty_entries.len(), 0); // misc entry has title so not empty
    assert!(report.invalid_entries.len() >= 2); // invalid + year=1 warning

    let summary = report.issue_summary();
    assert!(summary.errors > 0); // Duplicates + empty + missing required fields
    assert!(summary.warnings > 0); // Year=1 warning
}

#[test]
fn test_validation_error_display() {
    let error = ValidationError::error(Some("title"), "Missing required field");
    let display = format!("{}", error);
    assert!(display.contains("title"));
    assert!(display.contains("Missing required field"));
    assert!(display.contains("Error"));

    let warning = ValidationError::warning(None, "Entry-level warning");
    let display = format!("{}", warning);
    assert!(display.contains("<entry>"));
    assert!(display.contains("Entry-level warning"));
    assert!(display.contains("Warning"));
}

#[test]
fn test_validation_field_formats() {
    let input = r#"
        @article{format_test,
            author = "John Doe",
            title = "Test",
            journal = "Journal",
            year = 2024,
            doi = "not-a-doi",
            url = "ftp://invalid-scheme",
            isbn = "123",
            month = "invalid-month",
            volume = "not-a-number"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    let result = entry.validate(ValidationLevel::Strict);
    assert!(result.is_err());

    let errors = result.unwrap_err();

    // Check for specific format warnings (in strict mode)
    assert!(errors.iter().any(|e| e.field == Some("doi".to_string())));
    assert!(errors.iter().any(|e| e.field == Some("url".to_string())));
    assert!(errors.iter().any(|e| e.field == Some("isbn".to_string())));
    assert!(errors.iter().any(|e| e.field == Some("month".to_string())));
    // Note: volume format check is info-level and may not appear depending on implementation
}

#[test]
fn test_validation_book_author_editor() {
    let input = r#"
        @book{no_author_editor,
            title = "Book Title",
            publisher = "Publisher",
            year = 2024
        }
        @book{with_author,
            author = "Author Name",
            title = "Book Title",
            publisher = "Publisher",
            year = 2024
        }
        @book{with_editor,
            editor = "Editor Name",
            title = "Book Title",
            publisher = "Publisher",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();

    // First book should have error about missing author/editor
    let result1 = library.entries()[0].validate(ValidationLevel::Standard);
    assert!(result1.is_err());
    let errors1 = result1.unwrap_err();
    // Should error because no author OR editor
    assert!(errors1
        .iter()
        .any(|e| e.message.contains("either 'author' or 'editor'")));

    // Second book should be valid (has author and all required fields)
    let result2 = library.entries()[1].validate(ValidationLevel::Standard);
    assert!(result2.is_ok());

    // Third book should be valid (has editor and all required fields)
    let result3 = library.entries()[2].validate(ValidationLevel::Standard);
    assert!(result3.is_ok());
}

#[test]
fn test_validation_empty_fields() {
    let input = r#"
        @article{empty_fields,
            author = "",
            title = "   ",
            journal = "Journal",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    let result = entry.validate(ValidationLevel::Standard);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    let empty_field_warnings = errors
        .iter()
        .filter(|e| e.message.contains("empty value"))
        .count();
    assert_eq!(empty_field_warnings, 2); // author and title are empty
}

#[test]
fn test_validation_crossref() {
    let input = r#"
        @article{valid_crossref,
            author = "Author",
            title = "Title",
            journal = "Journal",
            year = 2024,
            crossref = "some_reference"
        }
        @article{empty_crossref,
            author = "Author", 
            title = "Title",
            journal = "Journal",
            year = 2024,
            crossref = "   "
        }
    "#;

    let library = Library::parser().parse(input).unwrap();

    // Valid crossref should pass strict validation
    let result1 = library.entries()[0].validate(ValidationLevel::Strict);
    assert!(result1.is_ok());

    // Empty crossref should fail strict validation
    let result2 = library.entries()[1].validate(ValidationLevel::Strict);
    assert!(result2.is_err());
    let errors2 = result2.unwrap_err();
    assert!(errors2.iter().any(
        |e| e.field == Some("crossref".to_string()) && e.severity == ValidationSeverity::Error
    ));
}

#[test]
fn test_validation_case_insensitive_field_checking() {
    let input = r#"
        @article{case_test,
            Author = "John Doe",
            TITLE = "Test Article", 
            journal = "Test Journal",
            YEAR = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Should pass all validation levels with case-insensitive field checking
    assert!(entry.validate(ValidationLevel::Minimal).is_ok());
    assert!(entry.validate(ValidationLevel::Standard).is_ok());
    assert!(entry.validate(ValidationLevel::Strict).is_ok());
}

#[test]
fn test_validation_standard_vs_strict() {
    let input = r#"
        @article{detailed_test,
            author = "John Doe",
            title = "Test",
            journal = "Journal",
            year = 2024,
            doi = "not-a-doi",
            month = "invalid-month"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Standard should pass (only basic checks)
    let result_standard = entry.validate(ValidationLevel::Standard);
    assert!(result_standard.is_ok());

    // Strict should fail (format checks)
    let result_strict = entry.validate(ValidationLevel::Strict);
    assert!(result_strict.is_err());

    let errors = result_strict.unwrap_err();
    assert!(errors.iter().any(|e| e.field == Some("doi".to_string())));
    assert!(errors.iter().any(|e| e.field == Some("month".to_string())));
}

#[test]
fn test_validation_year_checks() {
    let input = r#"
        @article{future_year, author="A", title="T", journal="J", year=3000}
        @article{ancient_year, author="A", title="T", journal="J", year=500}
        @article{normal_year, author="A", title="T", journal="J", year=2024}
        @article{string_year, author="A", title="T", journal="J", year="not-a-year"}
    "#;

    let library = Library::parser().parse(input).unwrap();

    // Future year should have warning
    let result1 = library.entries()[0].validate(ValidationLevel::Standard);
    assert!(result1.is_err());
    let errors1 = result1.unwrap_err();
    assert!(errors1
        .iter()
        .any(|e| e.field == Some("year".to_string()) && e.message.contains("unlikely")));

    // Ancient year should have warning
    let result2 = library.entries()[1].validate(ValidationLevel::Standard);
    assert!(result2.is_err());
    let errors2 = result2.unwrap_err();
    assert!(errors2
        .iter()
        .any(|e| e.field == Some("year".to_string()) && e.message.contains("unlikely")));

    // Normal year should pass
    let result3 = library.entries()[2].validate(ValidationLevel::Standard);
    assert!(result3.is_ok());

    // String year should have warning
    let result4 = library.entries()[3].validate(ValidationLevel::Standard);
    assert!(result4.is_err());
    let errors4 = result4.unwrap_err();
    assert!(errors4
        .iter()
        .any(|e| e.field == Some("year".to_string()) && e.message.contains("number")));
}

#[test]
fn test_validation_pages_format() {
    let input = r#"
        @article{good_pages1, author="A", title="T", journal="J", year=2024, pages="12-34"}
        @article{good_pages2, author="A", title="T", journal="J", year=2024, pages="12--34"}
        @article{good_pages3, author="A", title="T", journal="J", year=2024, pages="12"}
        @article{good_pages4, author="A", title="T", journal="J", year=2024, pages="12-34,56-78"}
        @article{bad_pages1, author="A", title="T", journal="J", year=2024, pages="12 to 34"}
        @article{bad_pages2, author="A", title="T", journal="J", year=2024, pages="twelve"}
    "#;

    let library = Library::parser().parse(input).unwrap();

    // Good page formats should pass
    for i in 0..4 {
        let result = library.entries()[i].validate(ValidationLevel::Standard);
        assert!(result.is_ok(), "Entry {} should have valid pages", i);
    }

    // Bad page formats should have warnings
    for i in 4..6 {
        let result = library.entries()[i].validate(ValidationLevel::Standard);
        assert!(result.is_err(), "Entry {} should have invalid pages", i);
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == Some("pages".to_string())));
    }
}

#[test]
fn test_validation_performance() {
    // Test that validation doesn't significantly impact performance
    let input = r#"
        @article{perf_test,
            author = "John Doe",
            title = "Performance Test Article",
            journal = "Performance Journal",
            year = 2024,
            volume = 42,
            number = 1,
            pages = "1-10",
            doi = "10.1000/123456",
            url = "https://example.com",
            month = "January"
        }
    "#
    .repeat(100);

    let library = Library::parser().parse(&input).unwrap();

    // Measure validation time
    let start = std::time::Instant::now();
    for entry in library.entries() {
        let _ = entry.validate(ValidationLevel::Strict);
    }
    let duration = start.elapsed();

    // Should complete quickly - validation is opt-in so should have minimal impact
    assert!(
        duration.as_millis() < 50,
        "Validation took too long: {:?}",
        duration
    );
}

#[test]
fn test_validation_level_defaults() {
    let level = ValidationLevel::default();
    assert_eq!(level, ValidationLevel::Standard);
}

#[test]
fn test_issue_summary() {
    let input = r#"
        @article{missing_required,
            author = "Test Author",
            title = "Test"
        }
        @article{format_issues,
            author = "",
            title = "Test",
            journal = "Journal", 
            year = 999,
            doi = "bad-doi",
            month = "bad-month"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let report = library.validate_comprehensive(ValidationLevel::Strict);

    let summary = report.issue_summary();
    assert!(summary.errors > 0);
    assert!(summary.warnings > 0);
}

#[test]
fn test_validation_zero_cost_when_not_used() {
    // Test that parsing performance is not affected when validation is not used
    let input = "@article{test, author=\"A\", title=\"T\", journal=\"J\", year=2024}".repeat(1000);

    let start = std::time::Instant::now();
    let library = Library::parser().parse(&input).unwrap();
    let parse_duration = start.elapsed();

    // Parsing should still be fast
    assert!(
        parse_duration.as_millis() < 100,
        "Parsing with validation code present took too long: {:?}",
        parse_duration
    );
    assert_eq!(library.entries().len(), 1000);
}

// LATEX TO UNICODE CONVERSION TESTS

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_basic_accents() {
    let input = r#"
        @article{test2024,
            author = "Fran\c{c}ois R\'emi",
            title = "M\"{u}ller and Schr\"{o}dinger's work",
            journal = "Nature",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Test unicode conversion
    assert_eq!(
        entry.get_unicode("author"),
        Some("François Rémi".to_string())
    );

    assert_eq!(
        entry.get_unicode("title"),
        Some("Müller and Schrödinger's work".to_string())
    );

    // Original should still be available
    assert_eq!(entry.get("author"), Some("Fran\\c{c}ois R\\'emi"));

    assert_eq!(
        entry.get("title"),
        Some("M\\\"{u}ller and Schr\\\"{o}dinger's work")
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_case_insensitive() {
    let input = r#"
        @article{test2024,
            AUTHOR = "Jos\'e Garc\'ia",
            Title = "\\alpha and \\beta particles"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Test case-insensitive unicode conversion
    assert_eq!(
        entry.get_unicode_ignore_case("author"),
        Some("José García".to_string())
    );

    assert_eq!(
        entry.get_unicode_ignore_case("TITLE"),
        Some("α and β particles".to_string())
    );

    assert_eq!(
        entry.get_unicode_ignore_case("title"),
        Some("α and β particles".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_all_field_types() {
    let input = r#"
        @article{test,
            author = "Jos\'e Garc\'ia",
            title = "\\alpha and \\beta",
            year = 2024,
            note = "See M\\\"uller's work \\ldots"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Test get_as_unicode_string variants (handles all value types)
    assert_eq!(
        entry.get_as_unicode_string("author"),
        Some("José García".to_string())
    );

    assert_eq!(
        entry.get_as_unicode_string("title"),
        Some("α and β".to_string())
    );

    // Should work with numbers too
    assert_eq!(
        entry.get_as_unicode_string("year"),
        Some("2024".to_string())
    );

    assert_eq!(
        entry.get_as_unicode_string("note"),
        Some("See Müller's work …".to_string())
    );

    // Case-insensitive version
    assert_eq!(
        entry.get_as_unicode_string_ignore_case("AUTHOR"),
        Some("José García".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_all_fields() {
    let input = r#"
        @article{test,
            author = "Jos\'e Garc\'ia",
            title = "\\alpha and \\beta particles", 
            note = "See also: M\\\"uller \\ldots",
            journal = "Journal of \\gamma-ray Physics",
            year = 2024
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    let unicode_fields = entry.fields_unicode();

    // Should only include string literal fields (excludes year=2024 which is Number)
    assert_eq!(unicode_fields.len(), 4);

    let author = unicode_fields
        .iter()
        .find(|(k, _)| k == "author")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert_eq!(author, "José García");

    let title = unicode_fields
        .iter()
        .find(|(k, _)| k == "title")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert_eq!(title, "α and β particles");

    let note = unicode_fields
        .iter()
        .find(|(k, _)| k == "note")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert_eq!(note, "See also: Müller …");

    let journal = unicode_fields
        .iter()
        .find(|(k, _)| k == "journal")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert_eq!(journal, "Journal of γ-ray Physics");
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_comprehensive_accents() {
    let input = r#"
        @article{accents_test,
            acute = "\\'{a}\\'{e}\\'{i}\\'{o}\\'{u}",
            grave = "\\`{a}\\`{e}\\`{i}\\`{o}\\`{u}",
            circumflex = "\\^{a}\\^{e}\\^{i}\\^{o}\\^{u}",
            umlaut = "\\\"{a}\\\"{e}\\\"{i}\\\"{o}\\\"{u}",
            tilde = "\\~{a}\\~{n}\\~{o}",
            cedilla = "\\c{c}\\c{C}",
            ring = "\\r{a}\\aa\\AA",
            ligatures = "\\ae\\AE\\oe\\ss\\o\\O"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(entry.get_unicode("acute"), Some("áéíóú".to_string()));
    assert_eq!(entry.get_unicode("grave"), Some("àèìòù".to_string()));
    assert_eq!(entry.get_unicode("circumflex"), Some("âêîôû".to_string()));
    assert_eq!(entry.get_unicode("umlaut"), Some("äëïöü".to_string()));
    assert_eq!(entry.get_unicode("tilde"), Some("ãñõ".to_string()));
    assert_eq!(entry.get_unicode("cedilla"), Some("çÇ".to_string()));
    assert_eq!(entry.get_unicode("ring"), Some("ååÅ".to_string())); // Multiple representations
    assert_eq!(entry.get_unicode("ligatures"), Some("æÆœßøØ".to_string()));
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_greek_letters() {
    let input = r#"
        @article{greek_test,
            lowercase = "\\alpha \\beta \\gamma \\delta \\epsilon \\lambda \\mu \\pi \\sigma \\omega",
            uppercase = "\\Gamma \\Delta \\Lambda \\Pi \\Sigma \\Omega",
            math_context = "The \\alpha-particle has energy \\sim 5 MeV"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(
        entry.get_unicode("lowercase"),
        Some("α β γ δ ε λ μ π σ ω".to_string())
    );

    assert_eq!(
        entry.get_unicode("uppercase"),
        Some("Γ Δ Λ Π Σ Ω".to_string())
    );

    assert_eq!(
        entry.get_unicode("math_context"),
        Some("The α-particle has energy ∼ 5 MeV".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_mathematical_symbols() {
    let input = r#"
        @article{math_test,
            inequalities = "\\leq \\geq \\neq \\approx",
            operators = "\\pm \\mp \\times \\div",
            sets = "\\in \\notin \\subset \\cup \\cap",
            arrows = "\\rightarrow \\leftarrow \\Rightarrow",
            misc = "\\infty \\partial \\nabla \\ldots"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(
        entry.get_unicode("inequalities"),
        Some("≤ ≥ ≠ ≈".to_string())
    );

    assert_eq!(entry.get_unicode("operators"), Some("± ∓ × ÷".to_string()));

    assert_eq!(entry.get_unicode("sets"), Some("∈ ∉ ⊂ ∪ ∩".to_string()));

    assert_eq!(entry.get_unicode("arrows"), Some("→ ← ⇒".to_string()));

    assert_eq!(entry.get_unicode("misc"), Some("∞ ∂ ∇ …".to_string()));
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_symbols_and_punctuation() {
    let input = r#"
        @article{symbols_test,
            escape_chars = "\\& \\% \\$ \\# \\\\ \\{ \\}",
            quotes = "\\lq test\\rq and \\lqq nested\\rqq",
            misc_symbols = "\\copyright \\textregistered \\texttrademark \\degree",
            currency = "\\pounds \\textsterling",
            tildes = "non~breaking~spaces"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    assert_eq!(
        entry.get_unicode("escape_chars"),
        Some("& % $ # \\\\ { }".to_string())
    );

    assert_eq!(
        entry.get_unicode("quotes"),
        Some("'test' and \u{201c}nested\u{201d}".to_string())
    );

    assert_eq!(
        entry.get_unicode("misc_symbols"),
        Some("© ® ™ °".to_string())
    );

    assert_eq!(entry.get_unicode("currency"), Some("£ £".to_string()));

    // Non-breaking spaces (tildes) should become regular spaces
    assert_eq!(
        entry.get_unicode("tildes"),
        Some("non breaking spaces".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_mixed_content() {
    let input = r#"
        @article{mixed_test,
            complex_title = "Schr\"{o}dinger's equation: $i\\hbar\\frac{\\partial}{\\partial t}\\psi = \\hat{H}\\psi$",
            author_names = "M\\\"uller, Hans and Garc\\'ia, Jos\\'e and M\\o ller, \\O le",
            institution = "Institut f\\\"ur Theoretische Physik, Universit\\\"at M\\\"unchen"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Complex scientific text with mixed LaTeX
    assert_eq!(
        entry.get_unicode("complex_title"),
        Some("Schrödinger's equation: $iℏ∂/∂t ψ = Ĥψ$".to_string())
    );

    // Multiple name formats
    assert_eq!(
        entry.get_unicode("author_names"),
        Some("Müller, Hans and García, José and Møller, Øle".to_string())
    );

    // German institution with umlauts
    assert_eq!(
        entry.get_unicode("institution"),
        Some("Institut für Theoretische Physik, Universität München".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_no_conversion() {
    let input = r#"
        @article{plain_test,
            plain_title = "This is plain ASCII text",
            no_latex = "No LaTeX sequences here",
            mixed = "Some plain text with \\alpha mixed in"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Plain text should be unchanged (fast path)
    assert_eq!(
        entry.get_unicode("plain_title"),
        Some("This is plain ASCII text".to_string())
    );

    assert_eq!(
        entry.get_unicode("no_latex"),
        Some("No LaTeX sequences here".to_string())
    );

    // Mixed content should have partial conversion
    assert_eq!(
        entry.get_unicode("mixed"),
        Some("Some plain text with α mixed in".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_edge_cases() {
    let input = r#"
        @article{edge_test,
            incomplete_sequences = "\\\\ \\\\' incomplete",
            unknown_commands = "\\unknown{test} \\xyz",
            malformed = "\\' incomplete \\c",
            empty = "",
            backslashes = "C:\\\\path\\\\to\\\\file"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // Incomplete/unknown sequences should be left unchanged
    // BibTeX stores "\\\\ \\\\'" as 4 backslashes + space + 2 backslashes + apostrophe
    // The 4 backslashes get normalized to 2
    assert_eq!(
        entry.get_unicode("incomplete_sequences"),
        Some("\\\\ \\\\' incomplete".to_string())
    );

    // Unknown commands: BibTeX stores with doubled backslashes
    // but they're not converted since they're not recognized LaTeX
    assert_eq!(
        entry.get_unicode("unknown_commands"),
        Some("\\\\unknown{test} \\\\xyz".to_string())
    );

    // Malformed sequences: BibTeX stores with doubled backslashes
    assert_eq!(
        entry.get_unicode("malformed"),
        Some("\\\\' incomplete \\\\c".to_string())
    );

    assert_eq!(entry.get_unicode("empty"), Some("".to_string()));

    // Windows paths: BibTeX stores with 4 backslashes per backslash
    // These get normalized to 2 backslashes each
    assert_eq!(
        entry.get_unicode("backslashes"),
        Some("C:\\\\path\\\\to\\\\file".to_string())
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_performance_regression() {
    // Test that enabling unicode feature doesn't significantly slow down parsing
    let input = r#"
        @article{performance_test,
            author = "Hans M\\\"uller and Fran\\c{c}ois Dupont",
            title = "Research on \\alpha-decay and \\beta-emission",
            journal = "Journal f\\\"ur Kernphysik",
            year = 2024,
            note = "See Schr\\\"odinger's work \\ldots"
        }
    "#
    .repeat(100);

    let start = std::time::Instant::now();
    let library = Library::parser().parse(&input).unwrap();
    let parse_time = start.elapsed();

    // Parsing should still be fast even with unicode feature enabled
    assert!(
        parse_time.as_millis() < 100,
        "Parsing with unicode feature took too long: {:?}",
        parse_time
    );

    assert_eq!(library.entries().len(), 100);

    // Test that unicode conversion itself is reasonably fast
    let start = std::time::Instant::now();
    for entry in library.entries() {
        let _ = entry.get_unicode("author");
        let _ = entry.get_unicode("title");
        let _ = entry.get_unicode("note");
    }
    let unicode_time = start.elapsed();

    assert!(
        unicode_time.as_millis() < 50,
        "Unicode conversion took too long: {:?}",
        unicode_time
    );
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_with_string_expansion() {
    let input = r#"
        @string{institution = "Institut f\\\"ur Physik"}
        @string{title_prefix = "\\alpha-particle studies at "}
        
        @article{expanded_test,
            author = "M\\\"uller, Hans",
            title = title_prefix # institution,
            note = "See also \\ldots"
        }
    "#;

    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // String expansion happens before unicode conversion
    assert_eq!(
        entry.get_unicode("author"),
        Some("Müller, Hans".to_string())
    );

    assert_eq!(
        entry.get_unicode("title"),
        Some("α-particle studies at Institut für Physik".to_string())
    );

    assert_eq!(entry.get_unicode("note"), Some("See also …".to_string()));
}

#[cfg(feature = "latex_to_unicode")]
#[test]
fn test_latex_to_unicode_doctest_examples() {
    // Test examples from the documentation
    let input1 = r#"@article{test, author = "Jos\'e Garc\'ia"}"#;
    let db1 = Library::parser().parse(input1).unwrap();
    let entry1 = &db1.entries()[0];
    assert_eq!(
        entry1.get_unicode("author"),
        Some("José García".to_string())
    );

    let input2 = r#"@article{test, TITLE = "M\\\"uller's work"}"#;
    let library2 = Library::parser().parse(input2).unwrap();
    let entry2 = &library2.entries()[0];
    assert_eq!(
        entry2.get_unicode_ignore_case("title"),
        Some("Müller's work".to_string())
    );
}

// Test that methods don't exist when feature is disabled
#[cfg(not(feature = "latex_to_unicode"))]
#[test]
fn test_latex_to_unicode_methods_not_available() {
    let input = r#"@article{test, author = "Jos\'e"}"#;
    let library = Library::parser().parse(input).unwrap();
    let entry = &library.entries()[0];

    // These methods should not exist when feature is disabled
    // This is a compile-time test - if this compiles, the feature-gating worked

    // entry.get_unicode("author"); // Should not compile
    // entry.get_unicode_ignore_case("author"); // Should not compile
    // entry.fields_unicode(); // Should not compile

    // Regular methods should still work
    assert_eq!(entry.get("author"), Some("Jos\\'e"));
}

#[test]
fn test_library_blocks_and_source_capture() {
    let input = r#"
        % leading comment
        @string{venue = "VLDB"}
        @article{a2024, title = venue}
    "#;

    let library = Library::parser().capture_source().parse(input).unwrap();
    let blocks = library.blocks();

    assert_eq!(library.entries().len(), 1);
    assert_eq!(library.strings().len(), 1);
    assert_eq!(blocks.len(), 3);

    match blocks[1] {
        bibtex_parser::Block::String(definition) => {
            assert_eq!(definition.name(), "venue");
            assert!(definition.source.is_some());
        }
        _ => panic!("expected string block"),
    }

    match blocks[2] {
        bibtex_parser::Block::Entry(entry, source) => {
            assert_eq!(entry.key(), "a2024");
            assert_eq!(entry.get("title"), Some("VLDB"));
            assert!(source.is_some());
        }
        _ => panic!("expected entry block"),
    }
}

#[test]
fn test_tolerant_parse_retains_failed_blocks() {
    let input = r#"
        @article{ok2024, title = "Good", year = 2024}
        @article{broken, title = "Missing close"
        @book{book2024, title = "Recovered", year = 2024}
    "#;

    let library = Library::parser()
        .tolerant()
        .capture_source()
        .parse(input)
        .unwrap();

    assert_eq!(library.entries().len(), 2);
    assert_eq!(library.failed_blocks().len(), 1);
    assert!(library.failed_blocks()[0].raw.contains("broken"));
    assert!(library.failed_blocks()[0].source.is_some());
    assert_eq!(library.entries()[1].key(), "book2024");
}

#[test]
fn test_typed_transforms_and_entry_editing() {
    let input = r#"
        @article{z2024,
            journaltitle = "Journal",
            date = 2024,
            doi = "https://doi.org/10.1000/XYZ.",
            month = "January",
            keywords = "rust; parsing, bibtex"
        }
    "#;

    let mut library = Library::parse(input).unwrap();
    library.normalize_doi_fields();
    library.normalize_months(bibtex_parser::MonthStyle::Abbrev);
    library.normalize_fields(bibtex_parser::FieldNormalizeOptions {
        name_case: bibtex_parser::FieldNameCase::Lowercase,
        biblatex_aliases: true,
    });

    let entry = &mut library.entries_mut()[0];
    assert_eq!(entry.doi(), Some("10.1000/xyz".to_string()));
    assert_eq!(entry.get("month"), Some("jan"));
    assert_eq!(entry.journal(), Some("Journal".to_string()));
    assert_eq!(entry.year(), Some("2024".to_string()));
    assert_eq!(
        entry.keywords(),
        vec![
            "rust".to_string(),
            "parsing".to_string(),
            "bibtex".to_string()
        ]
    );

    entry.set_literal("note", "edited");
    assert_eq!(entry.get("note"), Some("edited"));
    assert_eq!(entry.rename_field("note", "annotation"), 1);
    assert_eq!(entry.remove("annotation").len(), 1);
}

#[test]
fn test_writer_preserves_block_order_by_default() {
    let input = r#"
        @comment{front}
        @string{venue = "VLDB"}
        @article{a, title = venue}
        @preamble{"plain preamble"}
        @book{b, title = "Book"}
    "#;

    let library = Library::parse(input).unwrap();
    let output = bibtex_parser::to_string(&library).unwrap();

    let comment = output.find("@comment{front}").unwrap();
    let string = output.find("@string{venue").unwrap();
    let article = output.find("@article{a").unwrap();
    let preamble = output.find("@preamble").unwrap();
    let book = output.find("@book{b").unwrap();

    assert!(comment < string);
    assert!(string < article);
    assert!(article < preamble);
    assert!(preamble < book);
}
