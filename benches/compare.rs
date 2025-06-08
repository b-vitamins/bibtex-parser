use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Generate realistic BibTeX content with various entry types and fields
fn generate_realistic_bibtex(n_entries: usize) -> String {
    let mut bib = String::with_capacity(n_entries * 300);

    // Add some string definitions that would be commonly used
    bib.push_str(
        r#"% Generated BibTeX file for benchmarking
@string{jan = "January"}
@string{feb = "February"}
@string{mar = "March"}
@string{apr = "April"}
@string{ieee = "IEEE Transactions"}
@string{acm = "ACM Computing Surveys"}
@string{springer = "Springer-Verlag"}
@string{mit = "MIT Press"}

@preamble{"This bibliography was generated for benchmarking purposes"}

"#,
    );

    // Mix of different entry types
    let entry_types = ["article", "book", "inproceedings", "techreport", "misc"];
    let journals = [
        "Nature",
        "Science",
        "Physical Review",
        "Communications of the ACM",
    ];
    let publishers = [
        "Addison-Wesley",
        "O'Reilly",
        "Wiley",
        "Cambridge University Press",
    ];

    for i in 0..n_entries {
        let entry_type = entry_types[i % entry_types.len()];

        match entry_type {
            "article" => {
                let entry = format!(
                    r#"@article{{entry{},
    author = "Author {} and Coauthor {} and Third Author",
    title = {{A Comprehensive Study of {} in Modern Computing Systems}},
    journal = "{}",
    year = {},
    volume = {},
    number = {},
    pages = "{}-{}",
    month = mar,
    doi = "10.1234/journal.{}.{}",
    abstract = {{This paper presents a comprehensive analysis of various aspects 
                 related to the topic under investigation. We propose novel methods
                 and validate them through extensive experimentation.}},
    keywords = {{algorithms, performance, benchmarking}}
}}

"#,
                    i,
                    i % 100,
                    i % 50,
                    format!("Topic {}", i % 20),
                    journals[i % journals.len()],
                    2000 + (i % 25),
                    i % 50 + 1,
                    i % 12 + 1,
                    i * 10,
                    i * 10 + 9,
                    2024,
                    i
                );
                bib.push_str(&entry);
            }
            "book" => {
                let entry = format!(
                    r#"@book{{book{},
    author = "Book Author {} and Editor {}",
    title = {{Advanced Techniques in {}: A Practitioner's Guide}},
    publisher = "{}",
    year = {},
    edition = "{}",
    isbn = "978-0-{}-{}-{}",
    pages = {},
    address = "New York, NY"
}}

"#,
                    i,
                    i % 100,
                    i % 50,
                    format!("Field {}", i % 15),
                    publishers[i % publishers.len()],
                    2005 + (i % 20),
                    match i % 4 {
                        0 => "1st",
                        1 => "2nd",
                        2 => "3rd",
                        _ => "4th",
                    },
                    100000 + i,
                    10000 + (i % 90000),
                    i % 10,
                    200 + (i % 500)
                );
                bib.push_str(&entry);
            }
            "inproceedings" => {
                let entry = format!(
                    r#"@inproceedings{{conf{},
    author = "Presenter {} and Co-author {} and Team Member {}",
    title = "Innovative Approaches to {} Using Machine Learning",
    booktitle = "Proceedings of the {}th International Conference on {}",
    year = {},
    pages = "{}-{}",
    location = "San Francisco, CA",
    publisher = acm,
    month = apr
}}

"#,
                    i,
                    i % 100,
                    i % 75,
                    i % 25,
                    format!("Problem {}", i % 30),
                    i % 50 + 1,
                    format!("Technology {}", i % 10),
                    2010 + (i % 15),
                    i * 5,
                    i * 5 + 4
                );
                bib.push_str(&entry);
            }
            _ => {
                let entry = format!(
                    r#"@misc{{misc{},
    author = "Various Authors",
    title = "Technical Note on {}",
    howpublished = "\url{{https://example.com/note{}}}",
    year = {},
    note = "Accessed: 2024-01-01"
}}

"#,
                    i,
                    format!("Subject {}", i % 40),
                    i,
                    2020 + (i % 5)
                );
                bib.push_str(&entry);
            }
        }

        // Add some comments between entries occasionally
        if i % 10 == 0 {
            bib.push_str(&format!("% Section {} entries\n\n", i / 10));
        }
    }

    bib
}

/// Benchmark parsing with our parser
fn bench_bibtex_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("bibtex_parser");
    group.measurement_time(Duration::from_secs(10));

    for &size in &[10, 50, 100, 500, 1000, 5000] {
        let input = generate_realistic_bibtex(size);
        let bytes = input.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("parse", size), &input, |b, input| {
            b.iter(|| {
                let db = Database::parse(black_box(input)).unwrap();
                black_box(db);
            });
        });
    }

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    group.measurement_time(Duration::from_secs(5));

    // Test zero-copy effectiveness with borrowed data
    let input = generate_realistic_bibtex(100);

    group.bench_function("parse_and_query", |b| {
        b.iter(|| {
            let db = Database::parse(black_box(&input)).unwrap();
            // Simulate typical usage patterns
            let _ = db.find_by_type("article");
            let _ = db.find_by_field("year", "2020");
            let _ = db.find_by_key("entry50");
            black_box(db);
        });
    });

    // Test string expansion overhead
    let complex_input = r#"
        @string{a = "Part A"}
        @string{b = "Part B"}
        @string{c = "Part C"}
        @string{combined = a # " and " # b # " and " # c}
        @article{test,
            title = combined # " with more text",
            author = a # " et al."
        }
    "#
    .repeat(100);

    group.bench_function("string_expansion", |b| {
        b.iter(|| {
            let db = Database::parse(black_box(&complex_input)).unwrap();
            black_box(db);
        });
    });

    group.finish();
}

/// Benchmark individual operations
fn bench_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations");

    // Create a database for operation benchmarks
    let input = generate_realistic_bibtex(1000);
    let db = Database::parse(&input).unwrap();

    group.bench_function("find_by_key_hit", |b| {
        b.iter(|| {
            let entry = db.find_by_key(black_box("entry500"));
            black_box(entry);
        });
    });

    group.bench_function("find_by_key_miss", |b| {
        b.iter(|| {
            let entry = db.find_by_key(black_box("nonexistent"));
            black_box(entry);
        });
    });

    group.bench_function("find_by_type_common", |b| {
        b.iter(|| {
            let entries = db.find_by_type(black_box("article"));
            black_box(entries);
        });
    });

    group.bench_function("find_by_type_rare", |b| {
        b.iter(|| {
            let entries = db.find_by_type(black_box("phdthesis"));
            black_box(entries);
        });
    });

    group.bench_function("find_by_field", |b| {
        b.iter(|| {
            let entries = db.find_by_field(black_box("year"), black_box("2020"));
            black_box(entries);
        });
    });

    group.finish();
}

/// Compare with nom-bibtex if available
#[cfg(feature = "compare_nom_bibtex")]
fn bench_comparison(c: &mut Criterion) {
    use nom_bibtex::Bibtex;

    let mut group = c.benchmark_group("parser_comparison");
    group.measurement_time(Duration::from_secs(10));

    for &size in &[10, 100, 1000] {
        let input = generate_realistic_bibtex(size);
        let bytes = input.len() as u64;

        group.throughput(Throughput::Bytes(bytes));

        group.bench_with_input(
            BenchmarkId::new("bibtex-parser", size),
            &input,
            |b, input| {
                b.iter(|| {
                    let db = Database::parse(black_box(input)).unwrap();
                    black_box(db);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("nom-bibtex", size), &input, |b, input| {
            b.iter(|| {
                let bib = Bibtex::parse(black_box(input)).unwrap();
                black_box(bib);
            });
        });
    }

    group.finish();
}

// Configure criterion groups based on available features
#[cfg(not(feature = "compare_nom_bibtex"))]
criterion_group!(
    benches,
    bench_bibtex_parser,
    bench_memory_patterns,
    bench_operations
);

#[cfg(feature = "compare_nom_bibtex")]
criterion_group!(
    benches,
    bench_bibtex_parser,
    bench_memory_patterns,
    bench_operations,
    bench_comparison
);

criterion_main!(benches);
