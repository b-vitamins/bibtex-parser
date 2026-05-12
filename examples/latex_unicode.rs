//! Example demonstrating LaTeX to Unicode conversion
//!
//! This example shows how to use the `latex_to_unicode` feature to convert
//! common LaTeX escape sequences to their Unicode equivalents for improved
//! readability of BibTeX data.
//!
//! Run with: cargo run --example latex_unicode --features latex_to_unicode

#[cfg(not(feature = "latex_to_unicode"))]
fn main() {
    eprintln!("This example requires the 'latex_to_unicode' feature.");
    eprintln!("Run with: cargo run --example latex_unicode --features latex_to_unicode");
    std::process::exit(1);
}

#[cfg(feature = "latex_to_unicode")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bibtex_parser::Library;

    println!("LaTeX to Unicode Conversion Demo");
    println!("=================================\n");

    // Sample BibTeX data with various LaTeX escape sequences
    let bibtex = r#"
        @article{muller2024,
            author = "Hans M\"{u}ller and Fran\c{c}ois Dupont",
            title = "Research on \alpha-decay and \beta-emission in heavy nuclei",
            journal = "Journal f\"{u}r Kernphysik",
            year = 2024,
            volume = 42,
            pages = "123--156",
            note = "See also Schr\"{o}dinger's original work \ldots"
        }
        
        @book{spanish2024,
            author = "Jos\'e Garc\'ia-Mart\'inez and Mar\'ia Gonz\'alez-L\'opez",
            title = "Ling\"{u}\'istica Espa\~nola: M\'etodos y Aplicaciones",
            publisher = "Editorial Acad\'emica",
            address = "Madrid",
            year = 2024,
            isbn = "978-84-123456-78-9"
        }
        
        @inproceedings{scandinavian2023,
            author = "Lars M\o ller and \O le Hansen",
            title = "Computational studies of phonetic variations in Scandinavian languages",
            booktitle = "Proceedings of the 12th Conference on Computational Linguistics",
            year = 2023,
            pages = "45--67",
            address = "K\o benhavn, Denmark"
        }
        
        @article{mathematical2024,
            author = "Anna M\\\"uller",
            title = "On the properties of \\alpha-stable distributions and \\beta-functions",
            journal = "Mathematical Analysis and Applications",
            year = 2024,
            volume = 15,
            number = 3,
            pages = "234--267",
            doi = "10.1000/math.2024.15.234",
            note = "Extends results from \\Gamma-function theory, see \\S 3.2 for details"
        }
        
        @misc{symbols2024,
            author = "International Physics Consortium",
            title = "Standard notation: \\alpha, \\beta, \\gamma particles \\& their interactions",
            howpublished = "Technical Report",
            year = 2024,
            note = "Covers symbols: \\leq, \\geq, \\neq, \\pm, \\times, \\div, \\rightarrow, \\infty"
        }
    "#;

    // Parse the BibTeX data
    let library = Library::parser().parse(bibtex)?;

    println!(
        "Parsed {} entries with LaTeX escape sequences.\n",
        library.entries().len()
    );

    for (i, entry) in library.entries().iter().enumerate() {
        println!("Entry {} - Key: {}", i + 1, entry.key());
        println!("Type: {}\n", entry.entry_type());

        // Show original LaTeX and converted Unicode side by side
        println!("Field Comparison (Original LaTeX → Unicode):");
        println!("{}", "─".repeat(70));

        // Get all fields for comparison
        for field in entry.fields() {
            if let Some(original) = field.value.as_str() {
                let field_name = &field.name;
                let unicode = entry.get_unicode(field_name).unwrap_or_default();

                // Only show fields that actually have LaTeX sequences
                if original != unicode {
                    println!("{}:", field_name);
                    println!("  Original: {}", original);
                    println!("  Unicode:  {}", unicode);
                    println!();
                } else if field_name == "author" || field_name == "title" {
                    // Always show author and title for context, even if no conversion
                    println!("{}:", field_name);
                    println!("  Value:    {}", original);
                    println!();
                }
            }
        }

        // Demonstrate different access methods
        println!("Method Demonstrations:");
        println!("{}", "─".repeat(30));

        // Case-sensitive vs case-insensitive
        if let Some(author) = entry.get_unicode("author") {
            println!("get_unicode(\"author\"): {}", author);
        }

        // Try uppercase field name with case-insensitive method
        if let Some(title) = entry.get_unicode_ignore_case("TITLE") {
            println!("get_unicode_ignore_case(\"TITLE\"): {}", title);
        }

        // Handle all field types (including numbers)
        if let Some(year) = entry.get_as_unicode_string("year") {
            println!("get_as_unicode_string(\"year\"): {}", year);
        }

        // Show all fields with Unicode conversion
        println!("\nAll Unicode Fields:");
        let unicode_fields = entry.fields_unicode();
        for (field_name, unicode_value) in &unicode_fields {
            if field_name != "author" && field_name != "title" {
                println!("  {}: {}", field_name, unicode_value);
            }
        }

        println!("\n{}\n", "═".repeat(80));
    }

    // Demonstrate specific conversion categories
    demonstrate_conversion_categories()?;

    Ok(())
}

#[cfg(feature = "latex_to_unicode")]
fn demonstrate_conversion_categories() -> Result<(), Box<dyn std::error::Error>> {
    use bibtex_parser::Library;

    println!("LaTeX to Unicode Conversion Categories");
    println!("=====================================\n");

    let categories_bibtex = r#"
        @misc{accent_demo,
            acute = "Caf\'e, r\'esum\'e, na\'ive",
            grave = "\\`a la carte, coll\\`ege",
            circumflex = "h\\^otel, cr\\^epe, for\\^et",
            umlaut = "M\\\"unchen, caf\\\"e, na\\\"ive",
            tilde = "Se\\~nor, Pi\\~na Colada",
            cedilla = "Fran\\c{c}ais, gar\\c{c}on",
            ring = "\\Aring ngstr\\\"om (\\AA ngstr\\\"om)",
            ligatures = "\\ae sthetic, \\oe dipus, Wei\\ss"
        }
        
        @misc{greek_demo,
            lowercase = "\\alpha \\beta \\gamma \\delta \\epsilon \\zeta \\eta \\theta \\lambda \\mu \\nu \\pi \\rho \\sigma \\tau \\phi \\chi \\psi \\omega",
            uppercase = "\\Alpha \\Beta \\Gamma \\Delta \\Epsilon \\Lambda \\Pi \\Sigma \\Phi \\Psi \\Omega",
            physics = "\\alpha-decay, \\beta-radiation, \\gamma-rays, \\Delta-function"
        }
        
        @misc{math_demo,
            relations = "a \\leq b \\leq c, x \\geq y, p \\neq q, f \\approx g",
            operators = "a \\pm b, c \\mp d, x \\times y, p \\div q",
            sets = "x \\in A, y \\notin B, A \\subset B, C \\cup D, E \\cap F",
            arrows = "A \\rightarrow B, C \\leftarrow D, P \\Rightarrow Q",
            calculus = "\\partial f / \\partial x, \\nabla \\cdot F, \\int_0^\\infty"
        }
        
        @misc{symbols_demo,
            punctuation = "Hello\\ldots world, a\\&b, 50\\% discount, \\$100, \\#hashtag",
            quotes = "\\lq single\\rq and \\lqq double\\rqq quotes",
            misc = "\\copyright 2024, temp \\degree C, \\pounds 50, \\infty",
            spacing = "word~word (non-breaking), normal spaces"
        }
    "#;

    let library = Library::parser().parse(categories_bibtex)?;

    let categories = [
        ("Accent Marks", "accent_demo"),
        ("Greek Letters", "greek_demo"),
        ("Mathematical Symbols", "math_demo"),
        ("Special Symbols", "symbols_demo"),
    ];

    for (category_name, entry_key) in &categories {
        if let Some(entry) = library.find_by_key(entry_key) {
            println!("{}", category_name);
            println!("{}", "─".repeat(category_name.len()));

            let unicode_fields = entry.fields_unicode();
            for (field_name, unicode_value) in &unicode_fields {
                println!("{}: {}", field_name, unicode_value);
            }
            println!();
        }
    }

    Ok(())
}
