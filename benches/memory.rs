//! Memory usage benchmarks for bibtex-parser
//!
//! This benchmark measures actual memory allocations during parsing and operations.
//! Run with: cargo bench --bench memory

use bibtex_parser::Database;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Generate a BibTeX string with n entries
fn generate_bibtex(n_entries: usize) -> String {
    let mut bib = String::with_capacity(n_entries * 350);
    
    // Add some string definitions
    bib.push_str(
        r#"@string{ieee = "IEEE Transactions"}
@string{acm = "ACM Computing Surveys"}

"#,
    );
    
    // Generate entries
    for i in 0..n_entries {
        let entry = format!(
            r#"@article{{entry{},
    author = "Author {} and Coauthor {}",
    title = "Title of Paper Number {}",
    journal = ieee,
    year = {},
    volume = {},
    pages = "{}-{}"
}}

"#,
            i,
            i,
            i,
            i,
            2000 + (i % 25),
            i % 50,
            i * 10,
            i * 10 + 9
        );
        bib.push_str(&entry);
    }
    
    bib
}

/// Measure memory usage for parsing
fn measure_parse_memory(entries: usize) -> (usize, usize, f64) {
    let input = generate_bibtex(entries);
    let input_size = input.len();
    
    reset_memory_stats();
    
    // Parse and keep the database alive
    let db = Database::parse(&input).unwrap();
    
    // Force the database to stay alive
    assert!(db.entries().len() >= entries);
    
    let (current, peak) = get_memory_stats();
    let overhead_ratio = peak as f64 / input_size as f64;
    
    (current, peak, overhead_ratio)
}

fn main() {
    println!("memory_parse");
    
    // Test different entry counts
    let test_sizes = [10, 50, 100, 500, 1000, 5000];
    
    for &entries in &test_sizes {
        let input_size = generate_bibtex(entries).len();
        let (current, peak, overhead) = measure_parse_memory(entries);
        
        // Output in a parseable format for the Python script
        println!("memory_parse/{}\t{}\t{}\t{}\t{:.2}", 
                 entries, input_size, peak, current, overhead);
    }
}