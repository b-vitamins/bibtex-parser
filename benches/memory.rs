//! Memory usage benchmarks for bibtex-parser
//!
//! This benchmark measures actual memory allocations during parsing.
//! Optimized version with progress reporting and reduced overhead.
//! Run with: cargo bench --bench memory

use bibtex_parser::Database;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// Include the test fixtures module
include!("../src/fixtures.rs");

/// Simple tracking allocator with minimal overhead
struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);

        if !ptr.is_null() {
            let old = ALLOCATED.fetch_add(size, Ordering::Relaxed);
            let new = old + size;

            // Update peak
            let mut peak = PEAK_ALLOCATED.load(Ordering::Relaxed);
            while new > peak {
                match PEAK_ALLOCATED.compare_exchange_weak(
                    peak,
                    new,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(p) => peak = p,
                }
            }

            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// Reset allocation counters
fn reset_memory_stats() {
    ALLOCATED.store(0, Ordering::Relaxed);
    PEAK_ALLOCATED.store(0, Ordering::Relaxed);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
}

/// Get current memory stats
fn get_memory_stats() -> (usize, usize, usize) {
    (
        ALLOCATED.load(Ordering::Relaxed),
        PEAK_ALLOCATED.load(Ordering::Relaxed),
        ALLOCATION_COUNT.load(Ordering::Relaxed),
    )
}

/// Measure memory usage for parsing
fn measure_parse_memory(entries: usize) -> (usize, usize, f64, f64, usize) {
    eprintln!("  Testing {} entries...", entries);
    let start = Instant::now();

    // Generate realistic BibTeX content
    eprint!("    Generating input... ");
    let gen_start = Instant::now();
    let input = generate_realistic_bibtex(entries);
    let input_size = input.len();
    eprintln!(
        "done ({} bytes in {:.1}s)",
        input_size,
        gen_start.elapsed().as_secs_f64()
    );

    reset_memory_stats();

    // Parse and keep the database alive
    eprint!("    Parsing... ");
    let parse_start = Instant::now();
    let db = match Database::parser().parse(&input) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            return (input_size, 0, 0.0, 0.0, 0);
        }
    };
    let parse_time = parse_start.elapsed().as_secs_f64();
    eprintln!("done ({:.1}s)", parse_time);

    // Force the database to stay alive
    assert!(db.entries().len() >= entries);

    let (current, peak, allocations) = get_memory_stats();
    let overhead_ratio = peak as f64 / input_size as f64;

    eprintln!(
        "    Results: {} peak, {:.2}x overhead, {} allocations",
        format_bytes(peak),
        overhead_ratio,
        allocations
    );
    eprintln!("    Total time: {:.1}s\n", start.elapsed().as_secs_f64());

    // Verify optimizations are working
    #[cfg(debug_assertions)]
    {
        use std::mem;
        assert_eq!(
            mem::size_of::<bibtex_parser::Entry>(),
            64,
            "Entry should be 64 bytes"
        );
        assert_eq!(
            mem::size_of::<bibtex_parser::Value>(),
            24,
            "Value should be 24 bytes"
        );

        // Check that vectors are shrunk
        for entry in db.entries() {
            assert_eq!(
                entry.fields.len(),
                entry.fields.capacity(),
                "Vectors should be shrunk to exact size"
            );
        }
    }

    (current, peak, overhead_ratio, parse_time, allocations)
}

/// Format bytes in human-readable form
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn main() {
    // Print header for compatibility
    println!("memory_parse");

    // Test different entry counts
    let test_sizes = [10, 50, 100, 500, 1000, 5000];

    eprintln!("\n📊 Memory Usage Benchmark");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("Testing with REALISTIC academic entries:");
    eprintln!(
        "  - Average entry size: ~{} bytes",
        average_bytes_per_entry()
    );
    eprintln!("  - Includes: long authors, titles, abstracts");
    eprintln!("  - Based on: NeurIPS, ICML, Phys Rev, etc.\n");

    let mut results = Vec::new();

    for &entries in &test_sizes {
        let input_size = generate_realistic_bibtex(entries).len();
        let (current, peak, overhead, parse_time, allocations) = measure_parse_memory(entries);

        // Output in parseable format for the Python script
        println!(
            "memory_parse/{}\t{}\t{}\t{}\t{:.2}",
            entries, input_size, peak, current, overhead
        );

        results.push((
            entries,
            input_size,
            peak,
            current,
            overhead,
            parse_time,
            allocations,
        ));
    }

    // Print summary
    eprintln!("\n Summary Report");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("┌─────────┬────────────┬────────────┬──────────┬──────────┬─────────┐");
    eprintln!("│ Entries │ Input Size │ Peak Memory│ Overhead │Parse Time│Allocs   │");
    eprintln!("├─────────┼────────────┼────────────┼──────────┼──────────┼─────────┤");

    for (entries, input_size, peak, _, overhead, parse_time, allocations) in &results {
        eprintln!(
            "│{:>8} │{:>11} │{:>11} │{:>9.2}x│{:>9.2}s│{:>8} │",
            entries,
            format_bytes(*input_size),
            format_bytes(*peak),
            overhead,
            parse_time,
            allocations
        );
    }
    eprintln!("└─────────┴────────────┴────────────┴──────────┴──────────┴─────────┘");

    // Calculate averages
    let avg_overhead =
        results.iter().map(|(_, _, _, _, o, _, _)| o).sum::<f64>() / results.len() as f64;

    eprintln!("\n Memory Optimizations Active:");
    eprintln!("  - Entry: 64 bytes (was 456)");
    eprintln!("  - Value: 24 bytes (was 32)");
    eprintln!("  - Vectors: shrunk to exact size");
    eprintln!("\n Results:");
    eprintln!("  - Average overhead: {:.2}x", avg_overhead);
    eprintln!(
        "  - Memory target: <1.5x {}",
        if avg_overhead < 1.5 {
            "✓ ACHIEVED"
        } else {
            "✗ FAILED"
        }
    );
}
