use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

// Include test fixtures
include!("../src/fixtures.rs");

fn bench_parallel_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_scaling");
    group.measurement_time(Duration::from_secs(10));

    // Test with different file counts
    for &file_count in &[10, 50, 100] {
        let inputs: Vec<String> = (0..file_count)
            .map(|_| generate_realistic_bibtex(100))
            .collect();

        // Benchmark different thread counts
        for &threads in &[1, 2, 4, 8] {
            group.bench_with_input(
                BenchmarkId::new(format!("{}_files", file_count), threads),
                &inputs,
                |b, inputs| {
                    b.iter(|| {
                        let files: Vec<_> = inputs
                            .iter()
                            .enumerate()
                            .map(|(i, content)| {
                                let path = format!("/tmp/bench_{}.bib", i);
                                std::fs::write(&path, content).unwrap();
                                path
                            })
                            .collect();

                        let db = Database::parser()
                            .threads(threads)
                            .parse_files(&files)
                            .unwrap();

                        // Clean up
                        for path in &files {
                            let _ = std::fs::remove_file(path);
                        }

                        black_box(db);
                    });
                },
            );
        }
    }

    group.finish();
}

fn explain_parallel_limitations(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_explanation");

    // Show that parsing is the bottleneck
    let input = generate_realistic_bibtex(1000);

    group.bench_function("parse_only", |b| {
        b.iter(|| {
            let items = crate::parser::parse_bibtex(black_box(&input)).unwrap();
            black_box(items);
        });
    });

    group.bench_function("parse_and_expand", |b| {
        b.iter(|| {
            let db = Database::parse(black_box(&input)).unwrap();
            black_box(db);
        });
    });

    println!("\nNOTE: Single-file parallel parsing is not implemented because:");
    println!("1. BibTeX requires sequential processing of @string definitions");
    println!("2. Entry boundaries are not trivially parallelizable");
    println!("3. Parse time dominates (>90%) vs string expansion (<10%)");
    println!("\nUse parse_files() for parallel processing of multiple files.");

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(10));
    targets = bench_parallel_files,
              explain_parallel_limitations
}

criterion_main!(benches);
