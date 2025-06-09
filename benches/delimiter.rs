//! Delimiter finding optimization benchmarks
//!
//! Compares different approaches to finding BibTeX delimiters.
//! Run with: cargo bench --bench delimiter

use bibtex_parser::parser::lexer::scan_to_bibtex_delimiter;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// BibTeX delimiters we search for
const BIBTEX_DELIMITERS: &[u8; 5] = b"@{}=,";

/// Generate realistic BibTeX content with known delimiter density
fn generate_bibtex_content(entries: usize) -> Vec<u8> {
    let mut content = String::with_capacity(entries * 600);

    for i in 0..entries {
        content.push_str(&format!(
            r#"@article{{key{},
    author = {{Author, A. and Coauthor, B. and Third, C.}},
    title = {{A Comprehensive Study of Performance Optimization in Large-Scale Systems}},
    journal = {{Journal of Computer Science and Engineering}},
    year = {},
    volume = {},
    number = {},
    pages = {{100--200}},
    doi = {{10.1234/jcse.{}.{}}},
    abstract = {{This paper presents a comprehensive analysis of performance optimization
                 techniques applicable to large-scale distributed systems. We propose novel
                 algorithms that significantly improve throughput while maintaining consistency
                 guarantees. Experimental results demonstrate effectiveness.}}
}}

"#,
            i,
            2020 + (i % 5),
            40 + (i % 10),
            i % 12 + 1,
            2024,
            i
        ));
    }

    content.into_bytes()
}

/// Scalar implementation - byte-by-byte search
fn find_delimiter_scalar(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    haystack[start..]
        .iter()
        .position(|&b| BIBTEX_DELIMITERS.contains(&b))
        .map(|pos| (start + pos, haystack[start + pos]))
}

/// Naive memchr - search for each delimiter separately
fn find_delimiter_naive_memchr(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    let search_slice = &haystack[start..];
    let mut best: Option<(usize, u8)> = None;

    // Search for each delimiter individually
    for &delim in BIBTEX_DELIMITERS {
        if let Some(pos) = memchr::memchr(delim, search_slice) {
            match best {
                None => best = Some((pos, delim)),
                Some((best_pos, _)) if pos < best_pos => best = Some((pos, delim)),
                _ => {}
            }
        }
    }

    best.map(|(pos, delim)| (start + pos, delim))
}

/// Manual unrolled search - explicit checks
fn find_delimiter_unrolled(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    let bytes = &haystack[start..];

    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'@' | b'{' | b'}' | b'=' | b',' => return Some((start + i, byte)),
            _ => continue,
        }
    }

    None
}

/// Benchmark delimiter finding throughput
fn bench_delimiter_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("delimiter_throughput");

    // Configure for throughput measurements
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(200);

    for entries in [10, 100, 1000] {
        let input = generate_bibtex_content(entries);
        let input_size = input.len() as u64;

        // Count delimiters for verification
        let delimiter_count = count_delimiters(&input);
        eprintln!(
            "Input: {} entries, {} KB, {} delimiters ({:.1} delimiters/KB)",
            entries,
            input_size / 1024,
            delimiter_count,
            delimiter_count as f64 / (input_size as f64 / 1024.0)
        );

        // Set throughput in bytes for MB/s calculation
        group.throughput(Throughput::Bytes(input_size));

        // Benchmark optimized two-pass approach
        group.bench_function(BenchmarkId::new("two_pass_memchr", entries), |b| {
            b.iter(|| {
                let mut pos = 0;
                let mut count = 0;
                while let Some((next_pos, _)) = scan_to_bibtex_delimiter(black_box(&input), pos) {
                    count += 1;
                    pos = next_pos + 1;
                }
                black_box(count)
            });
        });

        // Benchmark scalar approach
        group.bench_function(BenchmarkId::new("scalar", entries), |b| {
            b.iter(|| {
                let mut pos = 0;
                let mut count = 0;
                while let Some((next_pos, _)) = find_delimiter_scalar(black_box(&input), pos) {
                    count += 1;
                    pos = next_pos + 1;
                }
                black_box(count)
            });
        });

        // Benchmark naive memchr
        group.bench_function(BenchmarkId::new("naive_memchr", entries), |b| {
            b.iter(|| {
                let mut pos = 0;
                let mut count = 0;
                while let Some((next_pos, _)) = find_delimiter_naive_memchr(black_box(&input), pos)
                {
                    count += 1;
                    pos = next_pos + 1;
                }
                black_box(count)
            });
        });

        // Benchmark unrolled approach
        group.bench_function(BenchmarkId::new("unrolled", entries), |b| {
            b.iter(|| {
                let mut pos = 0;
                let mut count = 0;
                while let Some((next_pos, _)) = find_delimiter_unrolled(black_box(&input), pos) {
                    count += 1;
                    pos = next_pos + 1;
                }
                black_box(count)
            });
        });
    }

    group.finish();
}

/// Benchmark different delimiter patterns
fn bench_delimiter_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("delimiter_patterns");

    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    // Create different delimiter distribution patterns
    let patterns = vec![
        ("balanced", create_balanced_pattern(10_000)),
        ("brace_heavy", create_brace_heavy_pattern(10_000)),
        ("comma_heavy", create_comma_heavy_pattern(10_000)),
        ("sparse", create_sparse_pattern(10_000)),
        ("at_heavy", create_at_heavy_pattern(10_000)),
    ];

    for (name, input) in patterns {
        let input_size = input.len() as u64;
        group.throughput(Throughput::Bytes(input_size));

        // Count delimiters in pattern
        let delimiter_count = count_delimiters(&input);
        eprintln!(
            "Pattern '{}': {} bytes, {} delimiters ({:.1}%)",
            name,
            input_size,
            delimiter_count,
            delimiter_count as f64 / input_size as f64 * 100.0
        );

        group.bench_function(BenchmarkId::new("two_pass", name), |b| {
            b.iter(|| count_all_delimiters(black_box(&input), scan_to_bibtex_delimiter));
        });

        group.bench_function(BenchmarkId::new("scalar", name), |b| {
            b.iter(|| count_all_delimiters(black_box(&input), find_delimiter_scalar));
        });
    }

    group.finish();
}

/// Benchmark worst-case scenarios
fn bench_pathological_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathological_cases");

    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    // All delimiters (worst case for naive approach)
    let all_delimiters = BIBTEX_DELIMITERS
        .iter()
        .cycle()
        .take(10_000)
        .copied()
        .collect::<Vec<_>>();

    group.bench_function("all_delimiters", |b| {
        b.iter(|| count_all_delimiters(black_box(&all_delimiters), scan_to_bibtex_delimiter));
    });

    // No delimiters (worst case for all approaches)
    let no_delimiters = vec![b'x'; 10_000];

    group.bench_function("no_delimiters", |b| {
        b.iter(|| count_all_delimiters(black_box(&no_delimiters), scan_to_bibtex_delimiter));
    });

    // Alternating delimiter and non-delimiter
    let alternating: Vec<u8> = (0..10_000)
        .map(|i| if i % 2 == 0 { b'@' } else { b'x' })
        .collect();

    group.bench_function("alternating", |b| {
        b.iter(|| count_all_delimiters(black_box(&alternating), scan_to_bibtex_delimiter));
    });

    group.finish();
}

/// Helper to count delimiters in input
fn count_delimiters(input: &[u8]) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while let Some((next_pos, _)) = scan_to_bibtex_delimiter(input, pos) {
        count += 1;
        pos = next_pos + 1;
    }
    count
}

/// Helper to count all delimiters using a given search function
fn count_all_delimiters<F>(input: &[u8], search_fn: F) -> usize
where
    F: Fn(&[u8], usize) -> Option<(usize, u8)>,
{
    let mut count = 0;
    let mut pos = 0;
    while let Some((next_pos, _)) = search_fn(input, pos) {
        count += 1;
        pos = next_pos + 1;
    }
    count
}

/// Create a pattern with balanced delimiter distribution
fn create_balanced_pattern(size: usize) -> Vec<u8> {
    let pattern = b"@article{key, author={value}, title={value}} ";
    pattern.iter().cycle().take(size).copied().collect()
}

/// Create a pattern heavy in braces
fn create_brace_heavy_pattern(size: usize) -> Vec<u8> {
    let pattern = b"{{{nested {braces} with {more {nesting}}}}} ";
    pattern.iter().cycle().take(size).copied().collect()
}

/// Create a pattern heavy in commas
fn create_comma_heavy_pattern(size: usize) -> Vec<u8> {
    let pattern = b"field1=a,field2=b,field3=c,field4=d,field5=e,";
    pattern.iter().cycle().take(size).copied().collect()
}

/// Create a sparse pattern with few delimiters
fn create_sparse_pattern(size: usize) -> Vec<u8> {
    let mut pattern = vec![b'x'; 100];
    pattern.push(b'@');
    pattern.into_iter().cycle().take(size).collect()
}

/// Create a pattern heavy in @ symbols
fn create_at_heavy_pattern(size: usize) -> Vec<u8> {
    let pattern = b"@article @book @misc @inproceedings @techreport ";
    pattern.iter().cycle().take(size).copied().collect()
}

// Configure criterion
criterion_group! {
    name = benches;
    config = Criterion::default()
        .noise_threshold(0.05)
        .significance_level(0.01)
        .warm_up_time(Duration::from_secs(2));
    targets = bench_delimiter_throughput,
              bench_delimiter_patterns,
              bench_pathological_cases
}

criterion_main!(benches);
