//! Example of parsing a BibTeX file

use bibtex_parser::{Database, Result};
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <bibtex-file>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];
    let content = fs::read_to_string(filename)?;

    println!("Parsing {}...", filename);

    let db = Database::parser().parse(&content)?;

    println!("\nStatistics:");
    println!("  Entries: {}", db.entries().len());
    println!("  Strings: {}", db.strings().len());
    println!("  Preambles: {}", db.preambles().len());
    println!("  Comments: {}", db.comments().len());

    // Show entry types
    let mut type_counts = std::collections::HashMap::new();
    for entry in db.entries() {
        *type_counts
            .entry(entry.entry_type().to_string())
            .or_insert(0) += 1;
    }

    println!("\nEntry types:");
    for (ty, count) in type_counts {
        println!("  {}: {}", ty, count);
    }

    // Show first few entries
    println!("\nFirst entries (max 5):");
    for (i, entry) in db.entries().iter().take(5).enumerate() {
        println!("\n{}. {} ({})", i + 1, entry.key(), entry.entry_type());

        if let Some(author) = entry.get("author") {
            println!("   Author: {}", author);
        }
        if let Some(title) = entry.get("title") {
            println!("   Title: {}", title);
        }
        if let Some(year) = entry.get("year") {
            println!("   Year: {}", year);
        }
    }

    Ok(())
}
