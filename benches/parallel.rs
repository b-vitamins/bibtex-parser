use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

// Include test fixtures
include!("../src/fixtures.rs");

fn bench_parallel_scaling(c: &mut Criterion) {
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

criterion_group!(benches, bench_parallel_scaling);
criterion_main!(benches);
