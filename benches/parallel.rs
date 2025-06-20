// benches/parallel.rs
// Benchmark single-file parsing throughput scaling with threads (Phase 1.5)

use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// Include test fixtures and helpers
include!("../src/fixtures.rs");

/// Benchmark parsing throughput on a single large input with varying threads
fn bench_parallel_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));

    // Test different total data sizes (MB)
    for &mb_size in &[10usize, 50, 100] {
        let bytes_per_entry = average_bytes_per_entry();
        let total_entries = (mb_size * 1024 * 1024) / bytes_per_entry;
        let input = generate_realistic_bibtex(total_entries);
        let input_bytes = input.len() as u64;

        println!(
            "\nTesting {} MB ({} entries, {} bytes)",
            mb_size, total_entries, input_bytes
        );
        group.throughput(Throughput::Bytes(input_bytes));

        for &threads in &[1usize, 2, 4, 8, 12] {
            group.bench_with_input(
                BenchmarkId::new("single_file", format!("{}MB_{}t", mb_size, threads)),
                &input,
                |b, input| {
                    b.iter(|| {
                        let db = Database::parser().threads(threads).parse(input).unwrap();
                        black_box(db);
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_parallel_throughput);
criterion_main!(benches);
