use bibtex_parser::{Database, EntryType, Value};
use std::borrow::Cow;
use pretty_assertions::assert_eq;

#[test]
fn test_parse_simple_file() {
    let input = include_str!("fixtures/simple.bib");
    let db = Database::parser().parse(input).unwrap();

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
    let db = Database::parser().parse(input).unwrap();

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
    let result = Database::parser().parse(input);

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

    let db = Database::parser().parse(original).unwrap();
    let output = bibtex_parser::to_string(&db).unwrap();

    // Parse the output again
    let db2 = Database::parser().parse(&output).unwrap();

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

    let db = Database::parser().parse(input).unwrap();
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

    let db = Database::parser().parse(input).unwrap();
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

    let db = Database::parser().parse(input).unwrap();

    let einstein_papers = db.find_by_field("author", "Einstein");
    assert_eq!(einstein_papers.len(), 2);

    let papers_1905 = db.find_by_field("year", "1905");
    assert_eq!(papers_1905.len(), 1);
    assert_eq!(papers_1905[0].key(), "einstein1905");
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

    let db = Database::parser().parse(input).unwrap();

    // Verify entries parsed correctly
    assert_eq!(db.entries().len(), 2);

    let article = &db.entries()[0];
    assert_eq!(article.key(), "test2024");
    assert_eq!(article.entry_type(), &EntryType::Article);
    assert_eq!(article.get("author"), Some("Test Author"));
    assert_eq!(article.get("journal"), Some("IEEE")); // String expansion should work
    assert_eq!(article.get_as_string("year"), Some("2024".to_string()));

    let book = &db.entries()[1];
    assert_eq!(book.key(), "book2024");
    assert_eq!(book.entry_type(), &EntryType::Book);
    assert_eq!(book.get("author"), Some("Book Author"));
    assert_eq!(book.get("publisher"), Some("ACM")); // String expansion should work

    // Verify string definitions
    assert_eq!(db.strings().len(), 2);
    assert!(db.strings().contains_key("ieee"));
    assert!(db.strings().contains_key("acm"));

    // Verify preambles
    assert_eq!(db.preambles().len(), 1);

    // Verify comments
    assert_eq!(db.comments().len(), 1);
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

    let db = Database::parser().parse(input).unwrap();
    assert_eq!(db.entries().len(), 1);

    let entry = &db.entries()[0];
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

    let db = Database::parser().parse(input).unwrap();

    // Verify preambles
    assert_eq!(db.preambles().len(), 2);

    // Verify comments
    assert_eq!(db.comments().len(), 2);
}

#[test]
fn test_parenthesis_error_handling() {
    // Test mismatched delimiters
    let input = r#"@article(test, author = "John"})"#;
    let result = Database::parser().parse(input);
    assert!(result.is_err());

    // Test missing closing delimiter
    let input = r#"@article(test, author = "John""#;
    let result = Database::parser().parse(input);
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
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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
        let db = Database::parser().parse(&input).unwrap();
        let entry = &db.entries()[0];

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
        let db = Database::parser().parse(&input).unwrap();
        let entry = &db.entries()[0];

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
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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
    let result = Database::parser().parse(input);
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

    let db = Database::parser().parse(input).unwrap();

    assert_eq!(db.entries()[0].get("month"), Some("January")); // month constant
    assert_eq!(db.entries()[1].get("month"), Some("Custom February")); // user string
    assert_eq!(db.entries()[2].get("month"), Some("Custom March")); // user string
    assert_eq!(db.entries()[3].get("month"), Some("April")); // month constant
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
    let db = Database::parser().parse(input).unwrap();
    let duration = start.elapsed();

    assert_eq!(db.entries().len(), 5);

    // Should complete in well under 1ms for this small input
    assert!(
        duration.as_millis() < 10,
        "Parsing took too long: {:?}",
        duration
    );

    // Verify all months expanded correctly
    assert_eq!(db.entries()[0].get("month"), Some("January"));
    assert_eq!(db.entries()[1].get("month"), Some("February"));
    assert_eq!(db.entries()[2].get("month"), Some("March"));
    assert_eq!(db.entries()[3].get("month"), Some("April"));
    assert_eq!(db.entries()[4].get("month"), Some("May"));
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

    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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

    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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

    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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

    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];

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
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
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
    
    let db = Database::parser().parse(input).unwrap();
    let comments = db.comments();
    
    // Should have 2 % comments (% comments inside entries are not valid BibTeX)
    assert!(comments.iter().any(|c| c.contains("This is a line comment at the start")));
    assert!(comments.iter().any(|c| c.contains("Final comment at the end")));
    
    // Ensure the entry still parses correctly
    assert_eq!(db.entries().len(), 1);
    let entry = &db.entries()[0];
    assert_eq!(entry.get("author"), Some("Test Author"));
    assert_eq!(entry.get("title"), Some("Test Title"));
}

#[test]
fn test_percent_comment_not_consumed_by_whitespace() {
    let input = "   % Comment with leading whitespace\n@article{test, title=\"Test Title\"}";
    
    let db = Database::parser().parse(input).unwrap();
    assert_eq!(db.comments().len(), 1);
    assert!(db.comments()[0].contains("Comment with leading whitespace"));
    assert_eq!(db.entries().len(), 1);
}

#[test]
fn test_mixed_comment_types() {
    let input = r#"
        % Line comment
        @comment{Formal comment}
        Random text comment
        @article{test, title="Test"}
    "#;
    
    let db = Database::parser().parse(input).unwrap();
    assert!(db.comments().len() >= 3);
    
    // Verify all three types of comments are captured
    let comments = db.comments();
    assert!(comments.iter().any(|c| c.contains("Line comment")));
    assert!(comments.iter().any(|c| c.contains("Formal comment")));
    assert!(comments.iter().any(|c| c.contains("Random text comment")));
    
    assert_eq!(db.entries().len(), 1);
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
    
    let db = Database::parser().parse(input).unwrap();
    let comments = db.comments();
    
    // Should capture all percent comments including empty ones
    assert!(comments.iter().any(|c| c.contains("Simple comment")));
    assert!(comments.iter().any(|c| c.contains("Another comment without space")));
    assert!(comments.iter().any(|c| c.contains("Comment after entry")));
    
    assert_eq!(db.entries().len(), 1);
}

#[test]
fn test_percent_comment_in_complex_bibtex() {
    // Use the complex fixture which already has a % comment
    let input = include_str!("fixtures/complex.bib");
    let db = Database::parser().parse(input).unwrap();
    
    // The complex.bib file has "% Another inline comment" on line 38
    let comments = db.comments();
    assert!(comments.iter().any(|c| c.contains("Another inline comment")));
    
    // Should also have the formal @Comment entry
    assert!(comments.iter().any(|c| c.contains("This is a formal comment entry")));
    
    // And the text comment at the beginning
    assert!(comments.iter().any(|c| c.contains("This is a comment outside of any entry")));
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
    
    let db = Database::parser().parse(input).unwrap();
    assert_eq!(db.entries().len(), 1);
    
    let entry = &db.entries()[0];
    assert_eq!(entry.key(), "test1");
    
    // Pure number should remain as Number type
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    match entry.fields().iter().find(|f| f.name == "year").unwrap().value {
        Value::Number(n) => assert_eq!(n, 2024),
        _ => panic!("Expected Number value for pure number"),
    }
    
    // Mixed alphanumeric should be Literal strings
    assert_eq!(entry.get("volume"), Some("12b"));
    match entry.fields().iter().find(|f| f.name == "volume").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "12b"),
        _ => panic!("Expected Literal value for mixed alphanumeric"),
    }
    
    assert_eq!(entry.get("issue"), Some("2024a"));
    match entry.fields().iter().find(|f| f.name == "issue").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "2024a"),
        _ => panic!("Expected Literal value for year with letter"),
    }
    
    assert_eq!(entry.get("pages"), Some("123-456"));
    match entry.fields().iter().find(|f| f.name == "pages").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "123-456"),
        _ => panic!("Expected Literal value for page range"),
    }
    
    assert_eq!(entry.get("version"), Some("1.2.3"));
    match entry.fields().iter().find(|f| f.name == "version").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "1.2.3"),
        _ => panic!("Expected Literal value for dotted version"),
    }
    
    assert_eq!(entry.get("edition"), Some("2nd"));
    match entry.fields().iter().find(|f| f.name == "edition").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "2nd"),
        _ => panic!("Expected Literal value for ordinal"),
    }
    
    assert_eq!(entry.get("chapter"), Some("3rd"));
    match entry.fields().iter().find(|f| f.name == "chapter").unwrap().value {
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
    
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
    
    // All should parse successfully and return appropriate string representations
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    assert_eq!(entry.get_as_string("volume"), Some("12".to_string()));
    assert_eq!(entry.get_as_string("issue"), Some("3rd".to_string()));
    assert_eq!(entry.get_as_string("pages"), Some("100-150".to_string()));
    assert_eq!(entry.get_as_string("version"), Some("1.2.3".to_string()));
    assert_eq!(entry.get_as_string("id"), Some("abc123".to_string()));
    assert_eq!(entry.get_as_string("doi"), Some("10.1000/123".to_string()));
    assert_eq!(entry.get_as_string("isbn"), Some("978-0123456789".to_string()));
    
    // Check specific value types
    // Year should be Number
    match entry.fields().iter().find(|f| f.name == "year").unwrap().value {
        Value::Number(_) => {},
        _ => panic!("Year should be Number type"),
    }
    
    // Volume (quoted) should be Literal
    match entry.fields().iter().find(|f| f.name == "volume").unwrap().value {
        Value::Literal(_) => {},
        _ => panic!("Quoted number should be Literal type"),
    }
    
    // Issue (ordinal) should be Literal
    match entry.fields().iter().find(|f| f.name == "issue").unwrap().value {
        Value::Literal(_) => {},
        _ => panic!("Ordinal should be Literal type"),
    }
    
    // ID (quoted) should be Literal
    match entry.fields().iter().find(|f| f.name == "id").unwrap().value {
        Value::Literal(_) => {},
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
    
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
    
    assert_eq!(entry.get_as_string("year"), Some("2024".to_string()));
    assert_eq!(entry.get("volume"), Some("2024a"));
    assert_eq!(entry.get("issue"), Some("12b"));
    
    // Verify the value types
    // year should be expanded to Literal since string variables get expanded during parsing
    match entry.fields().iter().find(|f| f.name == "year").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "2024"),
        _ => panic!("Expanded string variable should be Literal type"),
    }
    
    // volume and issue should be Literals
    match entry.fields().iter().find(|f| f.name == "volume").unwrap().value {
        Value::Literal(ref s) => assert_eq!(s, "2024a"),
        _ => panic!("Digit-string should be Literal type"),
    }
    
    match entry.fields().iter().find(|f| f.name == "issue").unwrap().value {
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
    
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
    
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
        match entry.fields().iter().find(|f| f.name == field_name).unwrap().value {
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
    
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
    
    assert_eq!(entry.get_as_string("note"), Some("Version 1.2.3 released".to_string()));
    assert_eq!(entry.get_as_string("pages"), Some("100-200, 300-400".to_string()));
    assert_eq!(entry.get_as_string("year"), Some("2024a".to_string()));
    
    // Verify concatenation structure
    match entry.fields().iter().find(|f| f.name == "note").unwrap().value {
        Value::Concat(ref parts) => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], Value::Literal(Cow::Borrowed("Version ")));
            assert_eq!(parts[1], Value::Literal(Cow::Borrowed("1.2.3")));
            assert_eq!(parts[2], Value::Literal(Cow::Borrowed(" released")));
        },
        _ => panic!("Expected concatenated value"),
    }
}

#[test]
fn test_backward_compatibility_pure_numbers() {
    // Ensure pure numbers still work exactly as before
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
    
    let db = Database::parser().parse(input).unwrap();
    let entry = &db.entries()[0];
    
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
        match entry.fields().iter().find(|f| f.name == field_name).unwrap().value {
            Value::Number(n) => assert_eq!(n, expected_num),
            _ => panic!("Field {} should be Number type", field_name),
        }
    }
}
