// benches/performance.rs - Fixed version with proper warmup

use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// Include the test fixtures module
include!("../src/fixtures.rs");

/// Benchmark parsing with our parser
fn bench_bibtex_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("bibtex_parser");
    group.measurement_time(Duration::from_secs(10));

    // CRITICAL: Warm up the process before ANY measurements
    warmup_process();

    for &size in &[10, 50, 100, 500, 1000, 5000] {
        let input = generate_realistic_bibtex(size);
        let bytes = input.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("parse", size), &input, |b, input| {
            // Additional per-benchmark warmup
            for _ in 0..10 {
                let _ = Database::parser().parse(black_box(input));
            }

            // Now measure
            b.iter(|| {
                let db = Database::parser().parse(black_box(input)).unwrap();
                black_box(db);
            });
        });
    }

    group.finish();
}

/// CRITICAL: Process-level warmup to ensure everything is loaded
fn warmup_process() {
    // Generate a medium-sized input
    let warmup_input = generate_realistic_bibtex(100);

    // Parse multiple times to ensure:
    // 1. All code paths are loaded
    // 2. Dynamic linker has resolved everything
    // 3. CPU branch predictor is trained
    // 4. Memory pages are faulted in
    for _ in 0..1000 {
        let _ = Database::parser().parse(&warmup_input);
    }

    // Also warm up with different sizes to hit various code paths
    for size in &[10, 50, 500] {
        let input = generate_realistic_bibtex(*size);
        for _ in 0..50 {
            let _ = Database::parser().parse(&input);
        }
    }

    // Force a small delay to let CPU frequency stabilize
    std::thread::sleep(Duration::from_millis(100));
}

/// Benchmark memory usage patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    group.measurement_time(Duration::from_secs(5));

    // Warm up first!
    warmup_process();

    // Test zero-copy effectiveness with borrowed data
    let input = generate_realistic_bibtex(100);

    group.bench_function("parse_and_query", |b| {
        // Per-benchmark warmup
        for _ in 0..10 {
            let db = Database::parser().parse(&input).unwrap();
            let _ = db.find_by_type("article");
        }

        b.iter(|| {
            let db = Database::parser().parse(black_box(&input)).unwrap();
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
            let db = Database::parser().parse(black_box(&complex_input)).unwrap();
            black_box(db);
        });
    });

    group.finish();
}

/// Benchmark individual operations
fn bench_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations");

    // Warm up
    warmup_process();

    // Create a database for operation benchmarks
    let input = generate_realistic_bibtex(1000);
    let db = Database::parser().parse(&input).unwrap();

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

/// Benchmark LaTeX to Unicode conversion (when feature is enabled)
#[cfg(feature = "latex_to_unicode")]
fn bench_latex_unicode(c: &mut Criterion) {
    let mut group = c.benchmark_group("latex_unicode");
    group.measurement_time(Duration::from_secs(5));

    // Warm up
    warmup_process();

    // Create test data with various LaTeX sequences
    let latex_heavy_input = generate_latex_heavy_bibtex(100);
    let plain_input = generate_realistic_bibtex(100);
    let mixed_input = generate_mixed_latex_bibtex(100);

    // Benchmark parsing with LaTeX feature enabled (should have minimal impact)
    group.bench_function("parse_latex_heavy", |b| {
        b.iter(|| {
            let db = Database::parser()
                .parse(black_box(&latex_heavy_input))
                .unwrap();
            black_box(db);
        });
    });

    group.bench_function("parse_plain_text", |b| {
        b.iter(|| {
            let db = Database::parser().parse(black_box(&plain_input)).unwrap();
            black_box(db);
        });
    });

    // Benchmark Unicode conversion operations
    let db_latex = Database::parser().parse(&latex_heavy_input).unwrap();
    let db_plain = Database::parser().parse(&plain_input).unwrap();

    group.bench_function("unicode_conversion_heavy", |b| {
        b.iter(|| {
            for entry in db_latex.entries() {
                let _ = entry.get_unicode(black_box("author"));
                let _ = entry.get_unicode(black_box("title"));
                let _ = entry.get_unicode(black_box("journal"));
                black_box(());
            }
        });
    });

    group.bench_function("unicode_conversion_plain", |b| {
        b.iter(|| {
            for entry in db_plain.entries() {
                let _ = entry.get_unicode(black_box("author"));
                let _ = entry.get_unicode(black_box("title"));
                let _ = entry.get_unicode(black_box("journal"));
                black_box(());
            }
        });
    });

    // Benchmark all fields Unicode conversion
    let single_entry_db = Database::parser().parse(&mixed_input).unwrap();

    group.bench_function("fields_unicode_all", |b| {
        b.iter(|| {
            for entry in single_entry_db.entries() {
                let unicode_fields = entry.fields_unicode();
                black_box(unicode_fields);
            }
        });
    });

    group.finish();
}

/// Generate BibTeX with heavy LaTeX usage for benchmarking
#[cfg(feature = "latex_to_unicode")]
fn generate_latex_heavy_bibtex(count: usize) -> String {
    let mut result = String::new();

    for i in 0..count {
        result.push_str(&format!(
            r#"@article{{entry{i},
    author = "Hans M\\\"uller and Fran\\c{{c}}ois Dupont and Jos\\'e Garc\\'ia",
    title = "Research on \\alpha-decay and \\beta-emission: \\gamma-ray spectroscopy of \\pi-mesons",
    journal = "Journal f\\\"ur Kernphysik \\& Quantenphysik",
    year = {year},
    volume = {vol},
    pages = "{start}--{end}",
    note = "See Schr\\\"odinger's work \\ldots extensive \\partial/\\partial t analysis"
}}

"#,
            i = i + 1,
            year = 2020 + (i % 5),
            vol = 10 + (i % 50),
            start = 100 + (i * 10),
            end = 120 + (i * 10),
        ));
    }

    result
}

/// Generate mixed LaTeX content (some entries with LaTeX, some without)
#[cfg(feature = "latex_to_unicode")]
fn generate_mixed_latex_bibtex(count: usize) -> String {
    let mut result = String::new();

    for i in 0..count {
        if i % 3 == 0 {
            // LaTeX-heavy entry
            result.push_str(&format!(
                r#"@article{{latex_entry{i},
    author = "M\\\"uller, H. and Garc\\'ia, J.",
    title = "\\alpha-particles and \\beta-decay",
    journal = "Physics \\& Mathematics",
    year = {year},
    note = "\\ldots see \\S 2.3"
}}

"#,
                i = i + 1,
                year = 2020 + (i % 5),
            ));
        } else {
            // Plain ASCII entry
            result.push_str(&format!(
                r#"@article{{plain_entry{i},
    author = "Smith, John and Doe, Jane",
    title = "Modern computational approaches in physics",
    journal = "Journal of Computational Physics",
    year = {year},
    volume = {vol}
}}

"#,
                i = i + 1,
                year = 2020 + (i % 5),
                vol = 10 + (i % 20),
            ));
        }
    }

    result
}

/// Benchmark without LaTeX feature (for comparison)
#[cfg(not(feature = "latex_to_unicode"))]
fn bench_no_latex_feature(c: &mut Criterion) {
    let mut group = c.benchmark_group("no_latex_feature");
    group.measurement_time(Duration::from_secs(5));

    // Warm up
    warmup_process();

    // Same test data but without Unicode conversion capability
    let latex_input = r#"
        @article{test,
            author = "M\\\"uller, Hans and Garc\\'ia, Jos\\'e",
            title = "\\alpha-decay and \\beta-emission studies",
            journal = "Journal f\\\"ur Kernphysik"
        }
    "#
    .repeat(100);

    group.bench_function("parse_without_unicode_feature", |b| {
        b.iter(|| {
            let db = Database::parser().parse(black_box(&latex_input)).unwrap();
            // Only standard field access available
            for entry in db.entries() {
                let _ = entry.get("author");
                let _ = entry.get("title");
                let _ = entry.get("journal");
            }
            black_box(db);
        });
    });

    group.finish();
}

/// Compare with nom-bibtex
fn bench_comparison(c: &mut Criterion) {
    use nom_bibtex::Bibtex;

    let mut group = c.benchmark_group("parser_comparison");
    group.measurement_time(Duration::from_secs(10));

    // Warm up BOTH parsers
    warmup_process();
    warmup_nom_bibtex();

    for &size in &[10, 100, 1000] {
        let input = generate_realistic_bibtex(size);
        let bytes = input.len() as u64;

        group.throughput(Throughput::Bytes(bytes));

        group.bench_with_input(
            BenchmarkId::new("bibtex-parser", size),
            &input,
            |b, input| {
                b.iter(|| {
                    let db = Database::parser().parse(black_box(input)).unwrap();
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

/// Warm up nom-bibtex parser too
fn warmup_nom_bibtex() {
    use nom_bibtex::Bibtex;

    let input = generate_realistic_bibtex(100);
    for _ in 0..100 {
        let _ = Bibtex::parse(&input);
    }
}

// Configure Criterion with more aggressive settings
#[cfg(feature = "latex_to_unicode")]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))  // Longer warmup
        .measurement_time(Duration::from_secs(10))  // Longer measurement
        .sample_size(200);  // More samples
    targets = bench_bibtex_parser, bench_memory_patterns, bench_operations, bench_comparison, bench_latex_unicode
}

#[cfg(not(feature = "latex_to_unicode"))]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))  // Longer warmup
        .measurement_time(Duration::from_secs(10))  // Longer measurement
        .sample_size(200);  // More samples
    targets = bench_bibtex_parser, bench_memory_patterns, bench_operations, bench_comparison, bench_no_latex_feature
}

criterion_main!(benches);
