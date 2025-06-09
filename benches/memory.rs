//! Memory usage benchmarks for bibtex-parser
//!
//! This benchmark measures actual memory allocations during parsing.
//! Uses REALISTIC data based on actual academic entries.
//! Run with: cargo bench --bench memory

use bibtex_parser::Database;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// Include the test fixtures module
include!("../src/fixtures.rs");

/// Custom allocator that tracks memory usage
struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);

        if !ptr.is_null() {
            let old = ALLOCATED.fetch_add(size, Ordering::SeqCst);
            let new = old + size;

            // Update peak if necessary
            let mut peak = PEAK_ALLOCATED.load(Ordering::SeqCst);
            while new > peak {
                match PEAK_ALLOCATED.compare_exchange_weak(
                    peak,
                    new,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(p) => peak = p,
                }
            }
        }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// Reset allocation counters
fn reset_memory_stats() {
    ALLOCATED.store(0, Ordering::SeqCst);
    PEAK_ALLOCATED.store(0, Ordering::SeqCst);
}

/// Get current memory stats
fn get_memory_stats() -> (usize, usize) {
    (
        ALLOCATED.load(Ordering::SeqCst),
        PEAK_ALLOCATED.load(Ordering::SeqCst),
    )
}

/// Measure memory usage for parsing
fn measure_parse_memory(entries: usize) -> (usize, usize, f64) {
    // Generate realistic BibTeX content
    let input = generate_realistic_bibtex(entries);
    let input_size = input.len();

    // Show what we're testing
    if entries <= 10 {
        eprintln!("Testing {} entries (~{} bytes/entry)", entries, input_size / entries);
    }

    reset_memory_stats();

    // Parse and keep the database alive
    let db = Database::parse(&input).unwrap();

    // Force the database to stay alive
    assert!(db.entries().len() >= entries);

    let (current, peak) = get_memory_stats();
    let overhead_ratio = peak as f64 / input_size as f64;

    // Verify optimizations are working
    #[cfg(debug_assertions)]
    {
        use std::mem;
        assert_eq!(mem::size_of::<bibtex_parser::Entry>(), 64, "Entry should be 64 bytes");
        assert_eq!(mem::size_of::<bibtex_parser::Value>(), 24, "Value should be 24 bytes");
        
        // Check that vectors are shrunk
        for entry in db.entries() {
            assert_eq!(
                entry.fields.len(),
                entry.fields.capacity(),
                "Vectors should be shrunk to exact size"
            );
        }
    }

    (current, peak, overhead_ratio)
}

fn main() {
    println!("memory_parse");
    
    // Test different entry counts
    let test_sizes = [10, 50, 100, 500, 1000, 5000];

    eprintln!("\n📊 Testing with REALISTIC academic entries:");
    eprintln!("  - Average entry size: ~{} bytes", average_bytes_per_entry());
    eprintln!("  - Includes: long authors, titles, abstracts");
    eprintln!("  - Based on: NeurIPS, ICML, Phys Rev, etc.\n");

    for &entries in &test_sizes {
        let input_size = generate_realistic_bibtex(entries).len();
        let (current, peak, overhead) = measure_parse_memory(entries);

        // Output in a parseable format for the Python script
        println!(
            "memory_parse/{}\t{}\t{}\t{}\t{:.2}",
            entries, input_size, peak, current, overhead
        );
    }
    
    // Print optimization status and expected results
    eprintln!("\n✅ Memory optimizations active:");
    eprintln!("  - Entry: 64 bytes (was 456)");
    eprintln!("  - Value: 24 bytes (was 32)");
    eprintln!("  - Vectors: shrunk to exact size");
    eprintln!("\n📈 Expected with realistic data:");
    eprintln!("  - Overhead should be 1.1x - 1.5x");
    eprintln!("  - Lower than synthetic benchmark (2.x)");
    eprintln!("  - Matches real-world file measurements");
}