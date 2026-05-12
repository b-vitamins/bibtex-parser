//! Example of recovering useful entries from malformed BibTeX.

use bibtex_parser::{Block, Library, Result};

fn main() -> Result<()> {
    let input = r#"
        @article{ok2026,
            author = "Jane Doe",
            title = "Good Entry",
            year = 2026
        }

        @article{broken,
            title = "Missing closing brace"

        @book{recovered2026,
            author = "John Smith",
            title = "Recovered Entry",
            year = 2026
        }
    "#;

    let library = Library::parser().tolerant().capture_source().parse(input)?;

    println!("Recovered entries: {}", library.entries().len());
    println!("Malformed blocks: {}", library.failed_blocks().len());

    for block in library.blocks() {
        match block {
            Block::Entry(entry, source) => {
                let span = source
                    .map(|span| format!("{}..{}", span.byte_start, span.byte_end))
                    .unwrap_or_else(|| "unknown span".to_string());
                println!("entry {} at {}", entry.key(), span);
            }
            Block::Failed(failed) => {
                let span = failed
                    .source
                    .map(|span| format!("{}..{}", span.byte_start, span.byte_end))
                    .unwrap_or_else(|| "unknown span".to_string());
                println!("failed block at {}", span);
            }
            _ => {}
        }
    }

    Ok(())
}
