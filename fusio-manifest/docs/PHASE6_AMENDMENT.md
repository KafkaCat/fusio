# Phase 6: Realistic Production Workload Modeling

## Status: IN PROGRESS

## Motivation

Previous phases (1-5) tested arbitrary writer configurations with random key selection and configurable overlap ratios. While valuable for stress-testing, these don't match the real production use case identified by the team:

**Real Production Pattern:**
- **1 monotonic writer**: Appends new keys sequentially (simulates WAL append operations)
- **1 compactor**: Reads existing data, compacts, writes merged result (simulates background compaction)
- **Multiple readers**: High throughput read operations (simulates query workload)

**Key Insight:** The team identified that precondition failures between the writer and compactor are the primary concern, not arbitrary writer-writer conflicts.

---

## Changes Summary

### 1. Writer Behavior Model Overhaul

#### MonotonicWriter
- **Pattern**: Sequential key generation (`key_000000`, `key_000001`, `key_000002`, ...)
- **Rate**: 0.1 ops/sec (baseline), variable in sweep (0.02-0.08)
- **No random selection**: Always appends to the end
- **Tracks**: Last written key ID for verification

#### Compactor
- **Pattern**: Read-merge-write with sleep
  1. Read 2 random existing keys
  2. Sleep for 5 seconds (simulates compaction work)
  3. Write 1 new compacted key
- **Rate**: 0.05 ops/sec (measured excluding sleep time)
- **Conflict potential**: May conflict with writer on HEAD updates

#### Readers
- **Pattern**: Random reads from all existing keys
- **Rate**: 100 ops/sec baseline, sweep up to 300 ops/sec
- **Count**: 2 baseline, sweep 1-5 readers

### 2. Configuration Changes

#### New WorkloadConfig Fields
```rust
pub num_compactors: usize,        // Typically 1
pub compactor_rate: f64,          // 0.05 ops/sec
pub compactor_sleep_secs: u64,    // 5 seconds
pub compactor_read_count: usize,  // 2 keys per compaction
```

#### Removed Fields
- `key_overlap_ratio`: Not applicable with monotonic writes
- `key_pool_size`: Now dynamic, grows as writer writes

#### New Baseline
```yaml
num_writers: 1              # Monotonic writer
num_compactors: 1           # Compactor
num_readers: 2              # Start small
writer_rate: 0.1           # ops/sec
compactor_rate: 0.05       # ops/sec (excluding sleep)
reader_rate: 100.0         # ops/sec per reader
compactor_sleep_secs: 5
duration: 120              # 2 minutes
```

#### New Comprehensive Sweep Space
```yaml
num_writers: [1]           # Fixed
num_compactors: [1]        # Fixed
num_readers: [1, 2, 3, 4, 5]
reader_rate: [100, 200, 300]
writer_rate: [0.02, 0.04, 0.06, 0.08]
compactor_rate: [0.05]     # Fixed for now

Total: 1 × 1 × 5 × 3 × 4 × 1 = 60 configurations
Duration: ~8-10 minutes (60 configs ÷ 8 parallel)
```

### 3. Key Management: KeyPool → KeyRegistry

#### Old: KeyPool
- Pre-generated fixed pool of keys
- Distributed with overlap ratios
- Static allocation

#### New: KeyRegistry
- Dynamic key allocation (monotonic counter)
- Tracks all written keys in shared Vec
- Thread-safe with Arc<Mutex<>>

```rust
pub struct KeyRegistry {
    written_keys: Arc<Mutex<Vec<String>>>,
    next_key_id: Arc<AtomicUsize>,
}

// MonotonicWriter: allocate_next_key()
// Compactor: get_random_keys(2)
// Readers: all_keys() → sample randomly
```

### 4. Metrics Focus Shift

#### Primary Metric: LATENCY
- **Writer latency**: p50, p95, p99 (sequential write performance)
- **Compactor latency**: p50, p95, p99 (includes 5s sleep + commit time)
- **Read latency**: p50, p95, p99 (read scalability)

#### Secondary Metric: FAILURES
- **Precondition failures**: Writer vs Compactor (who conflicts more?)
- **Retry failures**: Failures even after retries (critical errors)
- **Total failure rate**: Overall system health

#### New Metrics Breakdown
```rust
// Separate histograms
writer_latency: Histogram<u64>
compactor_latency: Histogram<u64>
read_latency: Histogram<u64>

// Separate failure counters
writer_precond_failures: AtomicU64
compactor_precond_failures: AtomicU64
writer_retry_failures: AtomicU64
compactor_retry_failures: AtomicU64
```

### 5. Visualization Changes

#### New Baseline Plot (2×2 Grid)
1. **Write Latency**: Writer vs Compactor (p50, p99) grouped bars
2. **Read Latency**: p50, p95, p99 line plot
3. **Failure Rates**: Stacked bar [writer_precond, compactor_precond, retry_failures]
4. **TPS Overview**: Bar chart [writer_tps, compactor_tps, read_tps]

#### New Sweep Plot (3×3 Grid)
1. **Reader Count vs Read p99**: Line plot grouped by reader_rate
2. **Reader Rate vs Read p99**: Line plot grouped by num_readers
3. **Writer Rate vs Writer p99**: Line plot
4. **Compactor p99 Stability**: Box plot across all configs
5. **Writer Failure Rate Heatmap**: writer_rate × reader_count
6. **Compactor Failure Rate Heatmap**: writer_rate × reader_count
7. **Combined Failure Rate**: Bar chart per config
8. **Retry Failure Rate**: Stacked bars (writer vs compactor)
9. **Throughput Summary**: Grouped bars per config

#### Updated Chaos Plot
- Same 7 scenarios
- Updated to show writer vs compactor metrics
- Focus on latency degradation

---

## Implementation Plan

### Phase 6.1: Core Refactoring ✅ IN PROGRESS
- [x] Document Phase 6 plan (this file)
- [ ] Implement KeyRegistry (replace KeyPool)
- [ ] Add MonotonicWriter, Compactor to ClientType
- [ ] Update WorkloadConfig structure
- [ ] Implement new transaction methods in client.rs

### Phase 6.2: Metrics Enhancement
- [ ] Add separate writer/compactor histograms
- [ ] Add retry failure tracking
- [ ] Update CSV export with 30+ columns
- [ ] Update MetricsSummary struct
- [ ] Update metrics printing

### Phase 6.3: Test Updates
- [ ] Update test_baseline configuration
- [ ] Create generate_all_configs_v2()
- [ ] Update test_comprehensive_sweep
- [ ] Update test_chaos_sweep to load from v2 CSV
- [ ] Verify all tests compile

### Phase 6.4: Visualization
- [ ] Implement plot_baseline_focus()
- [ ] Implement plot_comprehensive_sweep_v2()
- [ ] Update chaos plotting functions
- [ ] Add CLI argument handling (--v2 flag)
- [ ] Test plot generation

### Phase 6.5: Verification Enhancement
- [ ] Add monotonic write verification
- [ ] Add compactor behavior verification
- [ ] Verify read-before-write ordering
- [ ] Update isolation checks

### Phase 6.6: Documentation
- [ ] Update IMPLEMENTATION_GUIDE.md
- [ ] Add Phase 6 to quick reference
- [ ] Document new baseline config
- [ ] Document new sweep parameters
- [ ] Update example commands

---

## Files to Modify

### Core Implementation (7 files)
1. ✅ `docs/PHASE6_AMENDMENT.md` - This file
2. `tests/perf_test/utils.rs` - KeyRegistry, WorkloadConfig updates
3. `tests/perf_test/client.rs` - New ClientType variants, new methods
4. `tests/perf_test/metrics.rs` - Separate histograms, new counters
5. `tests/perf_test/workload.rs` - Spawn compactor tasks
6. `tests/perf_test/visualization.rs` - New CSV columns (30+)
7. `tests/performance_test.rs` - Update all 3 test functions

### Visualization (1 file)
8. `plot_results.py` - New plotting functions

### Documentation (2 files)
9. `IMPLEMENTATION_GUIDE.md` - Add Phase 6 section
10. `tests/perf_test/IMPLEMENTATION_PLAN.md` - Update with Phase 6

---

## Expected Results

### Baseline Test (2 minutes)
```
Writer (monotonic):
  - Operations: ~12 writes (0.1 ops/sec × 120s)
  - Keys written: key_000000 to key_000011
  - Latency: p50 ~1.5s, p99 ~3s
  - Precond failures: 0-1 (rare, only if compactor conflicts)

Compactor:
  - Operations: ~2-3 compactions (0.05 ops/sec × 120s)
  - Each: Read 2 keys → sleep 5s → write 1 key
  - Latency: p50 ~7s, p99 ~9s (includes 5s sleep)
  - Precond failures: 0-2 (may conflict with writer on HEAD)

Readers (2 × 100 ops/sec):
  - Operations: ~24,000 reads
  - Latency: p50 ~1s, p99 ~2.5s
  - No failures (readers don't conflict)

Final state:
  - ~14-15 keys total (12 from writer + 2-3 from compactor)
  - Sequential writer keys verified
  - All reads observed committed state
```

### Comprehensive Sweep (60 configs, ~10 minutes)
```
Key Findings Expected:
- Read scalability limit: Identify at what reader_count/rate read p99 exceeds 3s
- Writer rate threshold: When does writer p99 exceed 5s?
- Compactor stability: Does compactor p99 remain stable (~7-9s)?
- Failure patterns: Writer-compactor conflicts increase with writer_rate
- Optimal config: Balance throughput vs latency

Best config prediction:
- 1 writer @ 0.04 ops/sec
- 1 compactor @ 0.05 ops/sec
- 3 readers @ 200 ops/sec
- Total TPS: ~600 reads/sec + 0.09 writes/sec
- Writer p99: <3s, Read p99: <2s, Failures: <5%
```

### Chaos Test (7 scenarios, ~5 minutes parallel)
```
Network Latency 100ms:
- Writer p99: +100ms → ~3.1s
- Compactor p99: +100ms → ~7.1s (sleep dominates)
- Read p99: +100ms → ~2.6s

Network Latency 500ms:
- Writer p99: +500ms → ~3.5s
- Compactor p99: +500ms → ~7.5s
- Read p99: +500ms → ~3.0s

CPU Overload 4@80%:
- Writer p99: +20% → ~3.6s (slower processing)
- Compactor p99: +20% → ~8.5s
- Read p99: +20% → ~3.0s
- TPS reduction: -30%

Combined (200ms + CPU):
- Writer p99: ~4s
- Compactor p99: ~8.5s
- Read p99: ~3.5s
- Failure rate: +50% (more contention)
```

---

## Verification Strategy

### 1. Monotonic Write Verification
```rust
fn verify_monotonic_writes(write_records: &[WriteRecord]) {
    let writer_writes: Vec<_> = write_records
        .iter()
        .filter(|r| r.writer_id == 0)  // Assume writer_id=0 is monotonic writer
        .collect();

    for (i, record) in writer_writes.iter().enumerate() {
        let expected_key = format!("key_{:06}", i);
        assert_eq!(record.key, expected_key, "Writer key not monotonic!");
    }
}
```

### 2. Compactor Behavior Verification
```rust
fn verify_compactor_behavior(
    read_records: &[ReadRecord],
    write_records: &[WriteRecord],
) {
    // For each compactor write, verify:
    // 1. Corresponding reads happened before the write
    // 2. Exactly 2 reads preceded the write
    // 3. Sleep delay was ~5 seconds between read and write
}
```

### 3. Isolation Guarantees (Keep Existing)
- ✅ No duplicate keys in final state
- ✅ Transaction ID monotonicity
- ✅ All successful writes reflected
- ✅ Readers only observe committed data
- ✅ Monotonic reads within reader sessions

---

## Success Criteria

### Code Quality
- [ ] All tests compile without warnings
- [ ] All tests run to completion
- [ ] No panics or crashes
- [ ] Clean clippy output

### Functional Requirements
- [ ] Baseline test shows expected behavior
- [ ] Writer generates sequential keys
- [ ] Compactor sleeps 5 seconds per operation
- [ ] Readers see all written keys
- [ ] Metrics correctly separate writer vs compactor

### Performance Requirements
- [ ] Comprehensive sweep completes in <15 minutes
- [ ] Baseline test completes in ~2 minutes
- [ ] Chaos test completes in ~5 minutes (parallel)
- [ ] CSV export works for all tests
- [ ] Plots generate without errors

### Verification Requirements
- [ ] Isolation verification passes all checks
- [ ] Monotonic write verification passes
- [ ] Compactor behavior verification passes
- [ ] No data corruption detected
- [ ] Retry failures tracked accurately

---

## Migration Path

### Backward Compatibility
- Keep old test functions alongside new ones
- Old CSVs: `comprehensive_sweep.csv`, `chaos_sweep.csv`
- New CSVs: `comprehensive_sweep_v2.csv`, `chaos_sweep_v2.csv`
- Both visualization modes available via CLI flags

### Git Strategy
- Branch: `feat/phase6-realistic-workload`
- Tag before changes: `v1.0-phase5-complete`
- Can revert if issues arise

### Validation Plan
1. Run Phase 6 baseline test
2. Compare results to expected output
3. Run small sweep (5 configs) to validate
4. Run full sweep (60 configs)
5. Compare chaos results to Phase 4
6. Review visualizations for correctness
7. Merge to main after team review

---

## Timeline Estimate

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| 6.1 | Core refactoring | 4-6 hours |
| 6.2 | Metrics enhancement | 2-3 hours |
| 6.3 | Test updates | 2-3 hours |
| 6.4 | Visualization | 2-3 hours |
| 6.5 | Verification | 1-2 hours |
| 6.6 | Documentation | 1 hour |
| **Total** | | **12-18 hours** |

---

## Open Questions

1. **Compactor key selection**: Should compactor always read the 2 most recent keys, or 2 random keys?
   - **Decision**: 2 random keys (more realistic, tests broader key range)

2. **Compactor sleep**: Should sleep happen before or after read?
   - **Decision**: After read, before write (simulates "compaction work")

3. **Delete operations**: Should writer or compactor perform deletes?
   - **Decision**: Remove deletes for now (simplify, add later if needed)

4. **Multiple compactors**: Should we test 0, 1, 2 compactors?
   - **Decision**: Fixed at 1 for Phase 6, variable in future phases

5. **Reader key selection**: Random from all keys or recent keys?
   - **Decision**: Random from all keys (tests broader range)

---

## Future Work (Phase 7+)

### Potential Extensions
- [ ] Variable compactor count (0, 1, 2)
- [ ] Compactor rate sweep
- [ ] Range compaction (read keys X-Y, write merged)
- [ ] Delete operations modeling
- [ ] GC during workload
- [ ] Multi-writer scenarios (partitioned key spaces)
- [ ] Lease expiration handling
- [ ] Long-running soak tests (1hr+)

### Advanced Scenarios
- [ ] Read-heavy workload (1000+ readers)
- [ ] Write-heavy workload (10+ writers)
- [ ] Bursty traffic patterns
- [ ] Gradual ramp-up tests
- [ ] Sustained high load tests

---

## References

- Phase 1-5 documentation: `docs/archive/`
- Technical details: `tests/perf_test/IMPLEMENTATION_PLAN.md`
- User guide: `IMPLEMENTATION_GUIDE.md`
- Current branch: `feat/simulation-tests`
- Base commit: `2cbcf76` (Add simulation test scaffolding)
