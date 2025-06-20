//! Example of querying BibTeX entries

use bibtex_parser::{Database, Result};

fn main() -> Result<()> {
    let bibtex = r#"
        @article{einstein1905,
            author = "Albert Einstein",
            title = "Zur Elektrodynamik bewegter Körper",
            journal = "Annalen der Physik",
            year = 1905
        }
        
        @article{einstein1915,
            author = "Albert Einstein",
            title = "Die Feldgleichungen der Gravitation",
            journal = "Sitzungsberichte der Preussischen Akademie der Wissenschaften",
            year = 1915
        }
        
        @book{hawking1988,
            author = "Stephen Hawking",
            title = "A Brief History of Time",
            publisher = "Bantam Books",
            year = 1988
        }
        
        @inproceedings{turing1950,
            author = "Alan Turing",
            title = "Computing Machinery and Intelligence",
            booktitle = "Mind",
            year = 1950
        }
    "#;

    let db = Database::parser().parse(bibtex)?;

    // Find all articles
    println!("Articles:");
    for entry in db.find_by_type("article") {
        println!(
            "  - {} by {}",
            entry.get("title").unwrap_or("Unknown"),
            entry.get("author").unwrap_or("Unknown")
        );
    }

    // Find Einstein's papers
    println!("\nEinstein's papers:");
    for entry in db.find_by_field("author", "Einstein") {
        println!(
            "  - {} ({})",
            entry.get("title").unwrap_or("Unknown"),
            entry.get("year").unwrap_or("Unknown")
        );
    }

    // Find papers from 1950
    println!("\nPapers from 1950:");
    for entry in db.find_by_field("year", "1950") {
        println!(
            "  - {} by {}",
            entry.get("title").unwrap_or("Unknown"),
            entry.get("author").unwrap_or("Unknown")
        );
    }

    // Find specific entry by key
    if let Some(entry) = db.find_by_key("hawking1988") {
        println!("\nFound Hawking's book:");
        println!("  Type: {}", entry.entry_type());
        println!("  Author: {}", entry.get("author").unwrap_or("Unknown"));
        println!("  Title: {}", entry.get("title").unwrap_or("Unknown"));
        println!(
            "  Publisher: {}",
            entry.get("publisher").unwrap_or("Unknown")
        );
    }

    Ok(())
}
