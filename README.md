# bibtex-parser

Yet another BibTeX parser written in Rust.

## Features

- Zero-copy parsing using winnow
- Support for string concatenation and variable expansion
- Error messages with line/column information
- Handles standard entry types, preambles, comments, and string variables
- Writer functionality for generating BibTeX files

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bibtex-parser = "0.1"
```

## Usage

Parse a BibTeX string:

```rust
use bibtex_parser::{Database, Result};

fn main() -> Result<()> {
    let bibtex = r#"
        @article{einstein1905,
            author = "Albert Einstein",
            title = "Zur Elektrodynamik bewegter Körper",
            journal = "Annalen der Physik",
            year = 1905
        }
    "#;
    
    let db = Database::parse(bibtex)?;
    
    for entry in db.entries() {
        println!("{}: {}", entry.key(), entry.get("title").unwrap_or("No title"));
    }
    
    Ok(())
}
```

### Parallel Parsing

For batch processing, enable the `parallel` feature:

```toml
[dependencies]
bibtex-parser = { version = "0.1", features = ["parallel"] }
```

Then use the builder API:

```rust
// Parse with explicit thread count
let db = Database::parser()
    .threads(8)
    .parse(input)?;

// Parse multiple files in parallel
let db = Database::parser()
    .threads(None)  // Use all available cores
    .parse_files(&["file1.bib", "file2.bib", "file3.bib"])?;
```

## Examples

### Query Entries

```rust
// Find entries by type
let articles = db.find_by_type("article");

// Find entries by field value
let einstein_papers = db.find_by_field("author", "Einstein");

// Get specific entry
if let Some(entry) = db.find_by_key("einstein1905") {
    println!("Title: {}", entry.get("title").unwrap());
}
```

### String Variables

```rust
let bibtex = r#"
    @string{me = "John Doe"}
    @string{inst = "MIT"}
    
    @article{doe2023,
        author = me # " and Jane Smith",
        institution = inst
    }
"#;

let db = Database::parse(bibtex)?;
// Variables are expanded during parsing
assert_eq!(db.entries()[0].get("author"), Some("John Doe and Jane Smith"));
```

### Write BibTeX

```rust
use bibtex_parser::writer::{Writer, WriterConfig};

let config = WriterConfig {
    indent: "  ".to_string(),
    align_values: true,
    sort_entries: true,
    ..Default::default()
};

let mut output = Vec::new();
let mut writer = Writer::with_config(&mut output, config);
writer.write_database(&db)?;
```

## Supported Entry Types

- `@article` - Journal article
- `@book` - Book with publisher
- `@inbook` - Part of a book
- `@inproceedings` / `@conference` - Conference paper
- `@proceedings` - Conference proceedings
- `@mastersthesis` - Master's thesis
- `@phdthesis` - PhD thesis
- `@techreport` - Technical report
- `@unpublished` - Unpublished work
- `@misc` - Miscellaneous

Custom entry types are also supported.

## Error Handling

```rust
match Database::parse(input) {
    Ok(db) => { /* use database */ },
    Err(e) => {
        eprintln!("Error: {}", e);
        // Error: Parse error at line 5, column 12: Expected '=' after field name
    }
}
```

## Dependencies

- winnow - Parser combinator library
- ahash - Fast hashing
- thiserror - Error handling
- memchr - String searching

## License

MIT license ([LICENSE](LICENSE))
