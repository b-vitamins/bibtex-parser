// High-quality performance benchmark suite for bibtex-parser
// Follows best practices for reliable, reproducible benchmarking

#![allow(clippy::too_many_lines)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
#[cfg(target_os = "linux")]
use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::sync::OnceLock;
use std::time::{Duration, Instant};

// Cache input files to avoid I/O variance
static TUGBOAT_BIB: &str = include_str!("../tests/fixtures/tugboat.bib");

/// Keep the benchmark thread on a single core when running on Linux.
#[cfg(target_os = "linux")]
fn pin_benchmark_thread() {
    set_thread_affinity(benchmark_cpu());
}

#[cfg(not(target_os = "linux"))]
fn pin_benchmark_thread() {}

#[cfg(target_os = "linux")]
fn benchmark_cpu() -> usize {
    static BENCHMARK_CPU: OnceLock<usize> = OnceLock::new();
    *BENCHMARK_CPU.get_or_init(detect_benchmark_cpu)
}

#[cfg(target_os = "linux")]
fn detect_benchmark_cpu() -> usize {
    use bibtex_parser::Database;

    if let Ok(cpu) = std::env::var("BIBTEX_BENCH_CPU") {
        if let Ok(cpu) = cpu.parse::<usize>() {
            return cpu;
        }
    }

    let candidate_cpus = collect_candidate_cpus();
    let Some(&first_cpu) = candidate_cpus.first() else {
        return 0;
    };

    let mut best_cpu = first_cpu;
    let mut best_elapsed = Duration::MAX;

    for cpu in candidate_cpus {
        set_thread_affinity(cpu);

        // A short empirical probe is more reliable on DVFS-heavy systems than
        // a single frequency snapshot from sysfs.
        let mut fastest_probe = Duration::MAX;
        for _ in 0..3 {
            let start = Instant::now();
            let db = Database::parser().parse(black_box(TUGBOAT_BIB)).unwrap();
            black_box(&db);
            fastest_probe = fastest_probe.min(start.elapsed());
        }

        if fastest_probe < best_elapsed {
            best_elapsed = fastest_probe;
            best_cpu = cpu;
        }
    }

    best_cpu
}

#[cfg(target_os = "linux")]
fn collect_candidate_cpus() -> Vec<usize> {
    let mut candidates = Vec::new();
    let mut seen_siblings = BTreeSet::new();

    if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            let Some(cpu_id) = name
                .strip_prefix("cpu")
                .and_then(|id| id.parse::<usize>().ok())
            else {
                continue;
            };

            let siblings_path = entry.path().join("topology/thread_siblings_list");
            let Ok(siblings) = std::fs::read_to_string(siblings_path) else {
                continue;
            };
            let siblings = siblings.trim().to_string();

            // Keep one representative per physical core and let the empirical
            // probe decide which core is actually fastest.
            if !seen_siblings.insert(siblings) {
                continue;
            }

            candidates.push(cpu_id);
        }
    }

    candidates.sort_unstable();
    candidates
}

#[cfg(target_os = "linux")]
fn set_thread_affinity(cpu: usize) {
    unsafe {
        let mut cpu_set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut cpu_set);
        libc::CPU_SET(cpu, &mut cpu_set);
        let _ = libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &cpu_set);
    }
}

/// Actively warm the parser so the benchmark starts at steady-state frequency.
fn stabilize_system() {
    use bibtex_parser::Database;

    pin_benchmark_thread();

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let db = Database::parser().parse(black_box(TUGBOAT_BIB)).unwrap();
        black_box(&db);
    }
}

/// Main parser comparison benchmark
fn bench_parser_comparison(c: &mut Criterion) {
    use bibtex_parser::Database;

    let mut group = c.benchmark_group("throughput");

    // Configure for high-quality measurements
    group.measurement_time(Duration::from_secs(20)); // Longer measurement time
    group.warm_up_time(Duration::from_secs(12)); // Longer warmup for DVFS-heavy systems
    group.sample_size(200); // More samples for statistics
    group.significance_level(0.01); // Stricter significance testing
    group.confidence_level(0.99); // Higher confidence requirement
    group.noise_threshold(0.02); // 2% noise threshold

    let input_bytes = TUGBOAT_BIB.len() as u64;
    group.throughput(Throughput::Bytes(input_bytes));

    // Extensive warmup phase
    stabilize_system();
    for _ in 0..50 {
        let _ = Database::parser().parse(TUGBOAT_BIB);
        std::hint::black_box(());
    }

    // Our parser - core performance
    group.bench_function("bibtex-parser", |b| {
        b.iter(|| {
            let db = Database::parser().parse(black_box(TUGBOAT_BIB)).unwrap();
            // Ensure result is not optimized away
            black_box(&db);
            assert!(!db.entries().is_empty());
        });
    });

    // serde_bibtex comparison - all modes
    bench_serde_bibtex_ignore(&mut group);
    bench_serde_bibtex_borrow(&mut group);
    bench_serde_bibtex_struct(&mut group);
    bench_serde_bibtex_copy(&mut group);

    // nom-bibtex comparison
    bench_nom_bibtex(&mut group);

    // biblatex comparison
    bench_biblatex(&mut group);

    group.finish();
}

/// Benchmark serde_bibtex parser - ignore mode (fastest, discards all data)
fn bench_serde_bibtex_ignore(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
) {
    use serde::de::IgnoredAny;
    use serde::Deserialize;
    use serde_bibtex::de::Deserializer;

    // Warmup
    for _ in 0..10 {
        let _ = IgnoredAny::deserialize(&mut Deserializer::from_str(TUGBOAT_BIB));
    }

    group.bench_function("serde_bibtex-ignore", |b| {
        b.iter(|| {
            let result =
                IgnoredAny::deserialize(&mut Deserializer::from_str(black_box(TUGBOAT_BIB)));
            black_box(&result);
        });
    });
}

/// Benchmark serde_bibtex parser - borrow mode (zero-copy, borrowed data)
fn bench_serde_bibtex_borrow(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
) {
    use serde::Deserialize;
    use serde_bibtex::de::Deserializer;
    use serde_bibtex::entry::BorrowEntry;

    type RawBibliography<'r> = Vec<BorrowEntry<'r>>;

    // Warmup
    for _ in 0..10 {
        let _ = RawBibliography::deserialize(&mut Deserializer::from_str(TUGBOAT_BIB));
    }

    group.bench_function("serde_bibtex-borrow", |b| {
        b.iter(|| {
            let result: Result<RawBibliography, _> =
                RawBibliography::deserialize(&mut Deserializer::from_str(black_box(TUGBOAT_BIB)));
            match result {
                Ok(entries) => {
                    black_box(&entries);
                    assert!(!entries.is_empty());
                }
                Err(e) => panic!("serde_bibtex-borrow parsing failed: {}", e),
            }
        });
    });
}

/// Benchmark serde_bibtex parser - struct mode (deserialize into specific struct)
fn bench_serde_bibtex_struct(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
) {
    use serde::Deserialize;
    use serde_bibtex::de::Deserializer;
    use std::borrow::Cow;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Fields<'r> {
        #[serde(borrow)]
        author: Option<Cow<'r, str>>,
        #[serde(borrow)]
        title: Option<Cow<'r, str>>,
        #[serde(borrow)]
        year: Option<Cow<'r, str>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TugboatEntry<'r> {
        entry_key: &'r str,
        #[serde(borrow)]
        fields: Fields<'r>,
    }

    // Warmup
    for _ in 0..10 {
        let de_iter = Deserializer::from_str(TUGBOAT_BIB).into_iter_regular_entry();
        let _: Vec<Result<TugboatEntry, _>> = de_iter.collect();
    }

    group.bench_function("serde_bibtex-struct", |b| {
        b.iter(|| {
            let de_iter = Deserializer::from_str(black_box(TUGBOAT_BIB)).into_iter_regular_entry();
            let result: Vec<Result<TugboatEntry, _>> = de_iter.collect();
            black_box(&result);
        });
    });
}

/// Benchmark serde_bibtex parser - copy mode (owned data with macro expansion)
fn bench_serde_bibtex_copy(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
) {
    use serde::Deserialize;
    use serde_bibtex::de::Deserializer;
    use serde_bibtex::entry::Entry;
    use serde_bibtex::MacroDictionary;

    type OwnedBibliography = Vec<Entry>;

    // Warmup
    for _ in 0..10 {
        let mut macros = MacroDictionary::default();
        macros.set_month_macros();
        let _ = OwnedBibliography::deserialize(&mut Deserializer::from_str_with_macros(
            TUGBOAT_BIB,
            macros,
        ));
    }

    group.bench_function("serde_bibtex-copy", |b| {
        b.iter(|| {
            let mut macros = MacroDictionary::default();
            macros.set_month_macros();
            let result = OwnedBibliography::deserialize(&mut Deserializer::from_str_with_macros(
                black_box(TUGBOAT_BIB),
                macros,
            ));
            match result {
                Ok(entries) => {
                    black_box(&entries);
                    assert!(!entries.is_empty());
                }
                Err(e) => panic!("serde_bibtex-copy parsing failed: {}", e),
            }
        });
    });
}

/// Benchmark nom-bibtex parser
fn bench_nom_bibtex(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>) {
    use nom_bibtex::Bibtex;

    // Warmup nom-bibtex
    for _ in 0..10 {
        let _ = Bibtex::parse(TUGBOAT_BIB);
    }

    group.bench_function("nom-bibtex", |b| {
        b.iter(|| {
            let result = Bibtex::parse(black_box(TUGBOAT_BIB));
            match result {
                Ok(bib) => {
                    black_box(&bib);
                    assert!(!bib.bibliographies().is_empty());
                }
                Err(e) => panic!("nom-bibtex parsing failed: {:?}", e),
            }
        });
    });
}

/// Benchmark biblatex parser
fn bench_biblatex(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>) {
    use biblatex::RawBibliography;

    // Warmup biblatex
    for _ in 0..10 {
        let _ = RawBibliography::parse(TUGBOAT_BIB);
    }

    group.bench_function("biblatex", |b| {
        b.iter(|| {
            let result = RawBibliography::parse(black_box(TUGBOAT_BIB));
            match result {
                Ok(bib) => {
                    black_box(&bib);
                    assert!(!bib.entries.is_empty());
                }
                Err(e) => panic!("biblatex parsing failed: {:?}", e),
            }
        });
    });
}

/// Focused benchmark on specific performance-critical operations
fn bench_critical_operations(c: &mut Criterion) {
    use bibtex_parser::Database;

    let mut group = c.benchmark_group("operations");
    group.measurement_time(Duration::from_secs(15));
    group.warm_up_time(Duration::from_secs(6));
    group.sample_size(150);

    // Pre-parse database for operation benchmarks
    let db = Database::parser().parse(TUGBOAT_BIB).unwrap();

    stabilize_system();

    // Benchmark: Sequential entry iteration (baseline)
    group.bench_function("entry_iteration", |b| {
        b.iter(|| {
            let mut count = 0;
            for entry in db.entries() {
                if !entry.key().is_empty() {
                    count += 1;
                }
            }
            black_box(count);
        });
    });

    // Benchmark: Field access pattern (typical usage)
    group.bench_function("field_access", |b| {
        b.iter(|| {
            let mut total_len = 0;
            for entry in db.entries().iter().take(1000) {
                if let Some(author) = entry.get("author") {
                    total_len += author.len();
                }
                if let Some(title) = entry.get("title") {
                    total_len += title.len();
                }
            }
            black_box(total_len);
        });
    });

    // Benchmark: Type filtering with pre-collected entries
    group.bench_function("type_filtering", |b| {
        use bibtex_parser::EntryType;

        // Pre-collect to avoid iterator overhead in measurement
        let entries: Vec<_> = db.entries().iter().collect();

        b.iter(|| {
            let mut articles = 0;
            for entry in &entries {
                if matches!(entry.entry_type(), EntryType::Article) {
                    articles += 1;
                }
            }
            black_box(articles);
        });
    });

    group.finish();
}

/// Memory efficiency benchmark
fn bench_memory_efficiency(c: &mut Criterion) {
    use bibtex_parser::Database;

    let mut group = c.benchmark_group("memory");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(4));

    stabilize_system();

    // Test with different input sizes to verify linear scaling
    let sizes = [
        ("100_entries", extract_entries(TUGBOAT_BIB, 100)),
        ("500_entries", extract_entries(TUGBOAT_BIB, 500)),
        ("1000_entries", extract_entries(TUGBOAT_BIB, 1000)),
    ];

    for (name, input) in &sizes {
        let input_bytes = input.len() as u64;
        group.throughput(Throughput::Bytes(input_bytes));

        group.bench_with_input(
            BenchmarkId::new("parse", name),
            input.as_str(),
            |b, input| {
                b.iter(|| {
                    let db = Database::parser().parse(black_box(input)).unwrap();
                    // Verify parsing succeeded
                    assert!(!db.entries().is_empty());
                    black_box(&db);
                });
            },
        );
    }

    group.finish();
}

/// Extract first N entries from BibTeX string
fn extract_entries(input: &str, max_entries: usize) -> String {
    let mut result = String::with_capacity(input.len() / 10);
    let mut entry_count = 0;
    let mut depth = 0;
    let mut in_entry = false;

    for line in input.lines() {
        let trimmed = line.trim_start();

        // Check for entry start
        if !in_entry && trimmed.starts_with('@') {
            let entry_type = trimmed
                .split_once(|c: char| c == '{' || c.is_whitespace())
                .map(|(t, _)| t.to_lowercase())
                .unwrap_or_default();

            // Skip non-entry items
            if entry_type == "@comment" || entry_type == "@preamble" || entry_type == "@string" {
                result.push_str(line);
                result.push('\n');
                continue;
            }

            if entry_count >= max_entries {
                break;
            }

            in_entry = true;
        }

        if in_entry {
            result.push_str(line);
            result.push('\n');

            // Track brace depth
            for ch in line.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            in_entry = false;
                            entry_count += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        } else if !in_entry {
            // Include preambles and string definitions
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(10))
        .measurement_time(Duration::from_secs(20))
        .sample_size(100)
        .significance_level(0.02)
        .confidence_level(0.98)
        .noise_threshold(0.03);
    targets = bench_parser_comparison, bench_critical_operations, bench_memory_efficiency
}

criterion_main!(benches);
