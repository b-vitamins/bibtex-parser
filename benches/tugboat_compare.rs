// benches/tugboat_compare.rs - Comprehensive comparison with serde_bibtex and nom-bibtex
// Using tugboat.bib (2.6MB) for fair comparison across all parsers

#![allow(clippy::too_many_lines)]
#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::borrow::Cow;
use std::time::Duration;

/// Load tugboat.bib once at startup
fn load_tugboat_bib() -> String {
    std::fs::read_to_string("tests/fixtures/tugboat.bib")
        .expect("tugboat.bib should be available in tests/fixtures/")
}

/// Fair comparison benchmark using tugboat.bib across all parsers
fn bench_tugboat_comparison(c: &mut Criterion) {
    use biblatex::RawBibliography as BiblatexRawBib;
    use bibtex_parser::Database;
    use nom_bibtex::Bibtex;
    use serde::de::IgnoredAny;
    use serde::Deserialize;
    use serde_bibtex::entry::{BorrowEntry, Entry};
    use serde_bibtex::error::Result as SerdeBibResult;
    use serde_bibtex::{de::Deserializer, MacroDictionary};

    // Define TugboatEntry struct similar to serde_bibtex benchmark
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Fields<'r> {
        #[serde(borrow)]
        author: Option<Cow<'r, str>>,
        #[serde(borrow)]
        title: Option<Cow<'r, str>>,
        #[serde(borrow)]
        journal: Option<Cow<'r, str>>,
        #[serde(borrow)]
        volume: Option<Cow<'r, str>>,
        #[serde(borrow)]
        number: Option<Cow<'r, str>>,
        #[serde(borrow)]
        pages: Option<Cow<'r, str>>,
        #[serde(borrow)]
        year: Option<Cow<'r, str>>,
        #[serde(borrow)]
        #[serde(rename = "ISSN")]
        issn: Option<Cow<'r, str>>,
        #[serde(borrow)]
        #[serde(rename = "ISSN-L")]
        issn_l: Option<Cow<'r, str>>,
        #[serde(borrow)]
        bibdate: Option<Cow<'r, str>>,
        #[serde(borrow)]
        bibsource: Option<Cow<'r, str>>,
        #[serde(borrow)]
        #[serde(rename = "URL")]
        url: Option<Cow<'r, str>>,
        #[serde(borrow)]
        acknowledgement: Option<Cow<'r, str>>,
        #[serde(borrow)]
        issue: Option<Cow<'r, str>>,
        #[serde(borrow)]
        #[serde(rename = "journal-URL")]
        journal_url: Option<Cow<'r, str>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TugboatEntry<'r> {
        entry_key: &'r str,
        #[serde(borrow)]
        fields: Fields<'r>,
    }

    type OwnedBibliography = Vec<Entry>;
    type RawBibliography<'r> = Vec<BorrowEntry<'r>>;

    // Load tugboat.bib once
    let input_str = load_tugboat_bib();
    let input_bytes = input_str.len() as u64;

    let mut group = c.benchmark_group("tugboat_comparison");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));
    group.throughput(Throughput::Bytes(input_bytes));

    // Warmup all parsers
    warmup_all_parsers(&input_str);

    // Our parser - basic parsing
    group.bench_function("bibtex-parser", |b| {
        b.iter(|| {
            let db = Database::parser().parse(black_box(&input_str)).unwrap();
            black_box(db);
        });
    });

    // Our parser - with typical field access
    group.bench_function("bibtex-parser-with-access", |b| {
        b.iter(|| {
            let db = Database::parser().parse(black_box(&input_str)).unwrap();
            // Simulate typical usage - count entries by type and access some fields
            let mut article_count = 0;
            for entry in db.entries() {
                if matches!(entry.entry_type(), bibtex_parser::EntryType::Article) {
                    article_count += 1;
                    let _ = entry.get("author");
                    let _ = entry.get("title");
                    let _ = entry.get("year");
                }
            }
            black_box(article_count);
        });
    });

    // serde_bibtex variants
    group.bench_function("serde_bibtex-ignore", |b| {
        b.iter(|| {
            let result =
                IgnoredAny::deserialize(&mut Deserializer::from_str(black_box(&input_str)));
            let _ = black_box(result);
        })
    });

    group.bench_function("serde_bibtex-borrow", |b| {
        b.iter(|| {
            let result: SerdeBibResult<RawBibliography> =
                RawBibliography::deserialize(&mut Deserializer::from_str(black_box(&input_str)));
            let _ = black_box(result);
        })
    });

    group.bench_function("serde_bibtex-struct", |b| {
        b.iter(|| {
            let de_iter = Deserializer::from_str(black_box(&input_str)).into_iter_regular_entry();
            let result: Vec<SerdeBibResult<TugboatEntry>> = de_iter.collect();
            let _ = black_box(result);
        })
    });

    group.bench_function("serde_bibtex-copy", |b| {
        b.iter(|| {
            let mut macros = MacroDictionary::default();
            macros.set_month_macros();
            let result = OwnedBibliography::deserialize(&mut Deserializer::from_str_with_macros(
                black_box(&input_str),
                macros,
            ));
            let _ = black_box(result);
        })
    });

    // biblatex
    group.bench_function("biblatex", |b| {
        b.iter(|| {
            let result = BiblatexRawBib::parse(black_box(&input_str));
            let _ = black_box(result);
        })
    });

    // nom-bibtex
    group.bench_function("nom-bibtex", |b| {
        b.iter(|| {
            let result = Bibtex::parse(black_box(&input_str));
            let _ = black_box(result);
        })
    });

    group.finish();
}

/// Benchmark with field access patterns to simulate real usage
fn bench_tugboat_field_access(c: &mut Criterion) {
    use bibtex_parser::Database;

    let input_str = load_tugboat_bib();
    let input_bytes = input_str.len() as u64;

    let mut group = c.benchmark_group("tugboat_field_access");
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Bytes(input_bytes));

    // Warmup
    warmup_all_parsers(&input_str);

    // Pre-parse data for field access benchmarks
    let our_db = Database::parser().parse(&input_str).unwrap();

    // Field access patterns for our parser
    group.bench_function("bibtex-parser-field-access", |b| {
        b.iter(|| {
            let mut count = 0;
            for entry in our_db.entries() {
                if let Some(_author) = entry.get("author") {
                    count += 1;
                }
                if let Some(_title) = entry.get("title") {
                    count += 1;
                }
                if let Some(_year) = entry.get("year") {
                    count += 1;
                }
            }
            black_box(count);
        });
    });

    group.finish();
}

/// Multi-size comparison using different portions of tugboat.bib
fn bench_tugboat_sizes(c: &mut Criterion) {
    use bibtex_parser::Database;
    use nom_bibtex::Bibtex;

    let full_tugboat = load_tugboat_bib();

    // Create different sizes by taking different portions
    let small_portion = extract_entries(&full_tugboat, 100);
    let medium_portion = extract_entries(&full_tugboat, 1000);
    let large_portion = extract_entries(&full_tugboat, 5000);

    let mut group = c.benchmark_group("tugboat_sizes");
    group.measurement_time(Duration::from_secs(8));

    // Warmup
    warmup_all_parsers(&full_tugboat);

    for (name, input) in &[
        ("small-100", small_portion),
        ("medium-1000", medium_portion),
        ("large-5000", large_portion),
        ("full", full_tugboat),
    ] {
        let bytes = input.len() as u64;
        group.throughput(Throughput::Bytes(bytes));

        group.bench_with_input(
            BenchmarkId::new("bibtex-parser", name),
            input,
            |b, input| {
                b.iter(|| {
                    let db = Database::parser().parse(black_box(input)).unwrap();
                    black_box(db);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("nom-bibtex", name), input, |b, input| {
            b.iter(|| {
                let result = Bibtex::parse(black_box(input));
                let _ = black_box(result);
            });
        });
    }

    group.finish();
}

/// Extract first N entries from a BibTeX string (approximation)
fn extract_entries(input: &str, max_entries: usize) -> String {
    let mut result = String::new();
    let mut entry_count = 0;
    let mut in_entry = false;
    let mut brace_count = 0;

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('@')
            && !trimmed.starts_with("@comment")
            && !trimmed.starts_with("@string")
        {
            if entry_count >= max_entries {
                break;
            }
            in_entry = true;
            brace_count = 0;
        }

        if in_entry {
            result.push_str(line);
            result.push('\n');

            // Count braces to detect end of entry
            for ch in line.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            in_entry = false;
                            entry_count += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        } else if trimmed.starts_with("@comment") || trimmed.starts_with("@string") {
            // Include comments and string definitions
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Comprehensive warmup for all parsers
fn warmup_all_parsers(input: &str) {
    use biblatex::RawBibliography as BiblatexRawBib;
    use bibtex_parser::Database;
    use nom_bibtex::Bibtex;
    use serde::de::IgnoredAny;
    use serde_bibtex::de::Deserializer;

    // Use a smaller sample for warmup to avoid excessive warmup time
    let warmup_sample = extract_entries(input, 50);

    // Warm up our parser
    for _ in 0..10 {
        let _ = Database::parser().parse(&warmup_sample);
    }

    // Warm up serde_bibtex (using ignore mode to avoid API complexity)
    for _ in 0..10 {
        use serde::Deserialize;
        let _ = IgnoredAny::deserialize(&mut Deserializer::from_str(&warmup_sample));
    }

    // Warm up nom-bibtex
    for _ in 0..10 {
        let _ = Bibtex::parse(&warmup_sample);
    }

    // Warm up biblatex
    for _ in 0..10 {
        let _ = BiblatexRawBib::parse(&warmup_sample);
    }

    // Brief pause to let CPU stabilize
    std::thread::sleep(Duration::from_millis(100));
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);  // Fewer samples for large file benchmarks
    targets = bench_tugboat_comparison, bench_tugboat_field_access, bench_tugboat_sizes
}

criterion_main!(benches);
