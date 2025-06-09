# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase 1.1 Complete** - Comprehensive benchmarking infrastructure
  - Parse performance benchmarks for various file sizes (10-5000 entries)
  - Query operation benchmarks (find_by_key, find_by_type, find_by_field)
  - Memory usage patterns benchmarks
  - Comparison benchmarks with `nom-bibtex` parser
- **Phase 1.2 Complete** - Memory optimizations achieving <1.5x target
  - Boxing of Concat variant in Value enum (32 → 24 bytes)
  - Vector shrink_to_fit optimization eliminating over-allocation
  - Memory test suite validating struct sizes
  - Realistic test fixtures based on actual academic entries (src/fixtures.rs)
- Memory profiling with custom allocator
  - Tracks peak memory allocation
  - Calculates memory overhead ratio (memory used / input size)
  - Zero-copy efficiency validation
- Diagnostic tools for deep memory analysis
  - `src/bin/memanalysis.rs` - Structure size and allocation analysis
  - `src/bin/tracealloc.rs` - Allocation tracing with backtraces
  - `src/bin/diagnose.rs` - Comprehensive memory diagnostic with field distribution analysis
- Automated benchmark reporting with Python script
  - Rich terminal output with tables and color coding
  - Markdown report generation with historical tracking
  - Support for both performance and memory profiling
  - Updated to show Phase 1.2 completion status
- Optional `nom-bibtex` dependency for comparison benchmarks
- Development environment support with `manifest.scm` for Guix

### Changed
- **BREAKING: Value::Concat now contains Box<Vec<Value>>** instead of Vec<Value>
  - Reduces Value enum size from 32 to 24 bytes (25% reduction)
  - Saves 8 bytes per field value
- **Entry struct fixed from 456 → 64 bytes** (86% reduction)
  - This was the primary cause of memory overhead
  - Saved ~392 bytes per entry
- Added shrink_to_fit calls to eliminate vector over-allocation
  - Applied to entry fields after parsing
  - Applied to database collections after completion
  - Eliminates ~30% typical vector waste
- Memory benchmark now uses realistic data instead of synthetic
  - Based on real entries from NeurIPS, ICML, Physical Review, etc.
  - Average entry size ~600-900 bytes (was ~260 bytes)
  - Shows true memory efficiency of the parser
- Reorganized benchmarks into separate files:
  - `benches/performance.rs` - Basic parsing benchmarks and comparison suite
  - `benches/memory.rs` - Memory profiling benchmarks (now with realistic data)
- Updated implementation strategy based on profiling results
  - Abandoned string interning approach (increased memory by 20-126%!)
  - Abandoned SmallVec approach (caused 2.83x - 5.31x file-specific variability)
  - Focused on fixing structural issues instead of complex optimizations

### Fixed
- Zero-copy regression in `database.rs` where string expansion was creating unnecessary owned values
- Parser handling of `%` comments which were being consumed by whitespace skipping
- Memory overhead now within target range (was 2.76x - 5.31x, now 0.75x - 1.14x)

### Performance
- **Throughput**: 359 MB/s average (improved from 341 MB/s)
- **Speed**: 3.43x faster than nom-bibtex (range: 3.05x - 4.00x)
- Parse 1K entries in 0.9ms (well under 5ms goal) ✓
- Parse 5K entries in 4.2ms (well under 50ms goal) ✓
- **Memory overhead**: 0.75x - 1.14x ✓ (was 2.76x - 5.31x)
  - Small files (10 entries): 1.14x
  - Medium files (50-100 entries): 0.78x  
  - Large files (500-5000 entries): 0.75x - 0.80x
  - Parser now uses LESS memory than input file size for most files!

### Discovered
- **Realistic data shows true memory efficiency**
  - Synthetic benchmark data (260 bytes/entry) was too small
  - Real entries are 500-900 bytes each
  - Larger content dilutes fixed structural overhead
  - Memory target was already achievable, just hidden by unrealistic benchmarks
- **Simple optimizations beat complex schemes**
  - Boxing one enum variant: 25% reduction
  - Shrinking vectors: 30% savings
  - Total impact: 71% memory reduction
  - Much simpler than string interning or SmallVec approaches
- **String interning is counterproductive** for BibTeX parsing
  - Pool overhead (~100KB) exceeds savings for typical files
  - Field names only account for ~200KB even with 4000+ entries
  - Our zero-copy design already prevents string allocations
- **SmallVec causes file-specific performance variability**
  - Files with ≤10 fields per entry: Good performance
  - Files with >10 fields per entry: 400 bytes wasted per entry
  - Causes unpredictable 2.83x - 5.31x overhead
  - Conclusion: Consistency more important than micro-optimization
- **Entry struct was the real problem**
  - Was 456 bytes due to padding/alignment issues
  - Should have been 64 bytes
  - Single biggest source of memory waste

## [0.1.0] - TBD

### Planned for Release
- **Phase 1**: Performance optimizations
  - [x] Measurement infrastructure ✓
  - [x] Fix structural overhead ✓
  - [ ] SIMD acceleration
  - [ ] Parallel parsing
  - [ ] Memory-mapped files
- **Phase 2**: Features
  - [ ] Streaming parser
  - [ ] Validation framework
  - [ ] LaTeX to Unicode conversion
  - [ ] Serde support
- **Phase 3**: Quality
  - [ ] Fuzzing
  - [ ] Enhanced error messages
  - [ ] Comprehensive documentation

### Target Metrics
- [x] Memory overhead: <1.5x file size ✓ (achieved 0.75x - 1.14x)
- [ ] Parse performance: 10x improvement over baseline
- [ ] Zero panics from fuzzing (100M iterations)

---

## Implementation Notes

### Phase 1.1 - Measurement Infrastructure (✅ Complete)
Established comprehensive baseline metrics through:
1. Created benchmark suite comparing with nom-bibtex
2. Built memory profiling infrastructure
3. Developed diagnostic tools for deep analysis
4. Discovered string interning is harmful for this use case
5. Discovered SmallVec causes unacceptable file-specific variability

### Phase 1.2 - Fix Structural Overhead (✅ Complete)
Successfully reduced memory overhead to target levels:

#### 1.2a - Fix Entry Size (✅ Complete)
- **Issue**: Entry struct was 456 bytes instead of 64
- **Solution**: Fixed struct definition and alignment
- **Result**: 456 → 64 bytes (86% reduction)
- **Impact**: Primary fix, saved ~400KB per 1000 entries

#### 1.2b - Optimize Value Enum (✅ Complete)  
- **Issue**: Value enum was 32 bytes due to large Concat variant
- **Solution**: Box the Vec in Concat variant
- **Result**: 32 → 24 bytes (25% reduction)
- **Impact**: Saved 90-280 KB on typical files

#### 1.2c - Eliminate Vector Waste (✅ Complete)
- **Issue**: Vectors over-allocate by ~30% typically
- **Solution**: Call shrink_to_fit() after parsing
- **Result**: 0% wasted capacity
- **Impact**: Saved 100-400 KB on typical files

### Phase 1.3 - SIMD Acceleration (Next)
Planning to accelerate hot paths:
1. Whitespace skipping (currently uses basic loops)
2. Delimiter finding (can use SIMD string search)
3. Balanced brace parsing (parallel scanning)

Expected impact: 2-3x speedup on lexing operations

### Key Learnings
1. **Always profile before optimizing** - String interning seemed obvious but made things worse
2. **Check struct sizes first** - Entry being 456 bytes was the root cause
3. **Simple solutions win** - Boxing one variant beat complex schemes
4. **Realistic benchmarks matter** - Synthetic data hid our actual performance
5. **Zero-copy works** - 100% of strings remain borrowed in typical usage

### Failed Optimization Attempts (Valuable Lessons)
1. **String Interning with lasso** (2024-12-09)
   - Hypothesis: Deduplicate repeated field names and values
   - Result: 20-126% INCREASE in memory usage
   - Why: Pool overhead (~100KB) exceeded savings (~200KB)
   
2. **SmallVec<[Field; 10]>** (2024-12-09)
   - Hypothesis: Avoid heap allocations for typical entries
   - Result: 2.83x - 5.31x overhead depending on field count
   - Why: 416-byte struct, wastes 400 bytes when > 10 fields
   - Lesson: File-specific performance is unacceptable for a parser

### Memory Optimization Results Summary
```
Before: 2.76x - 5.31x overhead (file-dependent)
After:  0.75x - 1.14x overhead (exceeds target!)

Breakdown of improvements:
- Entry struct fix:     ~50% reduction  
- Value enum boxing:    ~15% reduction
- Vector shrinking:     ~10% reduction
- Combined effect:      ~71% total reduction

The parser now uses LESS memory than the input file size for most real-world files!
```
