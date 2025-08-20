use bibtex_parser::{Database, ValidationLevel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bibtex = r#"
        @article{valid2024,
            author = "John Doe",
            title = "A Valid Article",
            journal = "Nature",
            year = 2024
        }
        
        @article{invalid2024,
            author = "Jane Doe",
            title = "Missing Journal"
        }
        
        @book{book2024,
            title = "A Book Without Author or Editor",
            publisher = "Publisher",
            year = 2024
        }
        
        @article{format_issues,
            author = "Bob Smith",
            title = "Article with Format Issues",
            journal = "Test Journal",
            year = 999,
            pages = "12 to 34",
            doi = "not-a-doi",
            url = "ftp://invalid-scheme",
            isbn = "123",
            month = "invalid-month"
        }
        
        @article{dup_key, title="First Duplicate"}
        @article{dup_key, title="Second Duplicate"}
        
        @misc{empty_entry, title="Minimal Entry"}
    "#;
    
    let db = Database::parser().parse(bibtex)?;
    
    println!("=== BibTeX Validation Report ===\n");
    
    // Basic validation statistics
    println!("Total entries: {}", db.entries().len());
    println!("Total strings: {}", db.strings().len());
    println!("Total preambles: {}", db.preambles().len());
    println!("Total comments: {}\n", db.comments().len());
    
    // Demonstrate different validation levels
    println!("=== Validation by Level ===");
    
    for (level_name, level) in [
        ("Minimal", ValidationLevel::Minimal),
        ("Standard", ValidationLevel::Standard), 
        ("Strict", ValidationLevel::Strict)
    ] {
        println!("\n--- {} Validation ---", level_name);
        let invalid = db.validate(level);
        
        if invalid.is_empty() {
            println!("✓ All entries are valid!");
        } else {
            println!("✗ Found {} entries with issues:", invalid.len());
            for (index, entry, errors) in &invalid {
                println!("  Entry {} ({}): {} issue(s)", index, entry.key(), errors.len());
                for error in errors {
                    let field = error.field.as_deref().unwrap_or("<entry>");
                    println!("    [{:?}] {}: {}", error.severity, field, error.message);
                }
            }
        }
    }
    
    // Comprehensive validation report
    println!("\n=== Comprehensive Validation Report ===");
    let report = db.validate_comprehensive(ValidationLevel::Standard);
    
    if report.is_valid() {
        println!("✓ Database is completely valid!");
    } else {
        let summary = report.issue_summary();
        println!("✗ Found {} total issues:", report.total_issues());
        println!("  - {} errors", summary.errors);
        println!("  - {} warnings", summary.warnings);
        println!("  - {} info messages", summary.infos);
        
        // Show duplicate keys
        if !report.duplicate_keys.is_empty() {
            println!("\n🔄 Duplicate Keys:");
            for key in &report.duplicate_keys {
                println!("  - {}", key);
            }
        }
        
        // Show empty entries
        if !report.empty_entries.is_empty() {
            println!("\n📝 Empty Entries:");
            for (index, entry) in &report.empty_entries {
                println!("  - Entry {} ({}): no fields", index, entry.key());
            }
        }
        
        // Show validation issues by entry
        if !report.invalid_entries.is_empty() {
            println!("\n⚠️  Validation Issues:");
            for (index, entry, errors) in &report.invalid_entries {
                println!("\n  Entry {} ({}):", index, entry.key());
                for error in errors {
                    let field = error.field.as_deref().unwrap_or("<entry>");
                    println!("    [{:?}] {}: {}", error.severity, field, error.message);
                }
            }
        }
    }
    
    // Individual entry validation examples
    println!("\n=== Individual Entry Validation ===");
    
    for (i, entry) in db.entries().iter().enumerate() {
        println!("\nEntry {}: {}", i, entry.key());
        
        // Quick check
        if entry.is_valid() {
            println!("  ✓ Has all required fields");
        } else {
            println!("  ✗ Missing required fields");
        }
        
        // Detailed validation
        match entry.validate(ValidationLevel::Strict) {
            Ok(()) => println!("  ✓ Passes strict validation"),
            Err(errors) => {
                println!("  ✗ {} validation issue(s):", errors.len());
                for error in &errors[..3.min(errors.len())] { // Show up to 3 errors
                    let field = error.field.as_deref().unwrap_or("<entry>");
                    println!("    - {}: {}", field, error.message);
                }
                if errors.len() > 3 {
                    println!("    ... and {} more", errors.len() - 3);
                }
            }
        }
    }
    
    // Show some valid entries for comparison
    println!("\n=== Valid Entry Examples ===");
    for entry in db.entries() {
        if entry.validate(ValidationLevel::Standard).is_ok() {
            println!("\n✓ {} ({})", entry.key(), entry.entry_type());
            if let Some(author) = entry.get("author") {
                println!("  Author: {}", author);
            }
            if let Some(title) = entry.get("title") {
                println!("  Title: {}", title);
            }
            if let Some(year) = entry.get_as_string("year") {
                println!("  Year: {}", year);
            }
        }
    }
    
    println!("\n=== Performance Note ===");
    println!("Validation is opt-in and has zero cost when not used.");
    println!("Parsing performance remains unaffected: ~700 MB/s throughput maintained.");
    
    Ok(())
}