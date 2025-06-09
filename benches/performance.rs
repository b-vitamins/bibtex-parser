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
                let _ = Database::parse(black_box(input));
            }
            
            // Now measure
            b.iter(|| {
                let db = Database::parse(black_box(input)).unwrap();
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
        let _ = Database::parse(&warmup_input);
    }
    
    // Also warm up with different sizes to hit various code paths
    for size in &[10, 50, 500] {
        let input = generate_realistic_bibtex(*size);
        for _ in 0..50 {
            let _ = Database::parse(&input);
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
            let db = Database::parse(&input).unwrap();
            let _ = db.find_by_type("article");
        }
        
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
    
    // Warm up
    warmup_process();

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

/// Warm up nom-bibtex parser too
fn warmup_nom_bibtex() {
    use nom_bibtex::Bibtex;
    
    let input = generate_realistic_bibtex(100);
    for _ in 0..100 {
        let _ = Bibtex::parse(&input);
    }
}

// Configure Criterion with more aggressive settings
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))  // Longer warmup
        .measurement_time(Duration::from_secs(10))  // Longer measurement
        .sample_size(200);  // More samples
    targets = bench_bibtex_parser, bench_memory_patterns, bench_operations, bench_comparison
}

criterion_main!(benches);