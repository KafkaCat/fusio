# Phase 6 Implementation Plan - REVISED

## Status: IN PROGRESS

## Key Changes from Original Plan

### What Changed
1. **NO COMPACTOR** - Removed compactor entirely, simplifies to 1 writer + readers
2. **Focus: LATENCY** - Primary metric is latency (p50, p99), not failure rates
3. **Baseline: NO precondition failures** - 1 writer = no conflicts = pure latency measurement
4. **Simpler sweep** - Writer TPS (5 values) × Num writers (3 values) = 15 configs
5. **Visualization: Line plots** - Connected dots showing latency trends across configs

### What Stayed the Same
- KeyRegistry for monotonic key allocation
- MonotonicWriter for sequential writes
- Reader behavior unchanged
- Serializable isolation verification

---

## Revised Requirements

### Baseline Test Configuration
```yaml
num_writers: 1              # Single writer (no conflicts!)
num_readers: 2              # Two readers
writer_rate: 0.1           # 0.1 ops/sec
reader_rate: 100.0         # 100 ops/sec per reader
duration: 120              # 2 minutes

Expected Results:
- ~12 writes (0.1 × 120s)
- ~24,000 reads (100 × 2 × 120s)
- Precondition failures: 0 (only 1 writer!)
- Focus: Writer p50/p99 latency, Reader p50/p99 latency
```

### Comprehensive Sweep Configuration
```yaml
num_writers: [1, 2, 3]
writer_rate: [0.02, 0.05, 0.1, 0.15, 0.2]
num_readers: [2]            # Fixed
reader_rate: [100.0]        # Fixed
duration: 60               # 1 minute per test

Total: 3 × 5 = 15 configurations
Duration: ~2 minutes (15 configs ÷ 8 parallel)
```

### Visualization Requirements
**Line Plot with Connected Dots:**
- X-axis: Config label (e.g., "W1_WR0.02", "W2_WR0.10")
- Y-axis: Latency (ms)
- Lines:
  - Writer p50 (solid blue)
  - Writer p99 (dashed blue)
  - Reader p50 (solid green)
  - Reader p99 (dashed green)
- Each config is a dot on the line
- Shows latency trends as TPS and writer count increase

---

## Simplified Architecture

### WorkloadConfig (Revised)
```rust
pub struct WorkloadConfig {
    pub num_writers: usize,     // 1 for baseline, 1-3 for sweep
    pub num_readers: usize,     // Fixed at 2
    pub writer_rate: f64,       // 0.02-0.2 sweep
    pub reader_rate: f64,       // Fixed at 100.0
    pub duration: Duration,

    // Removed: num_compactors, compactor_rate, compactor_sleep_secs, compactor_read_count
    // Removed: key_overlap_ratio (no overlap with monotonic writes)
    // Removed: write_delete_ratio (no deletes)

    pub value_size: usize,
    pub max_retry_count: usize,
}
```

### ClientType (Revised)
```rust
pub enum ClientType {
    MonotonicWriter { id: usize },  // Sequential key writes
    Reader { id: usize },            // Random key reads

    // Removed: Compactor
    // Keep legacy: Writer (for old tests)
}
```

### MetricsCollector (Revised)
```rust
pub struct MetricsCollector {
    // Simple latency tracking
    writer_latency: Arc<Mutex<Histogram<u64>>>,
    read_latency: Arc<Mutex<Histogram<u64>>>,

    // Simple counters
    total_writes_succeeded: AtomicU64,
    total_precondition_failures: AtomicU64,  // Should be 0 for baseline!
    total_reads: AtomicU64,

    // Tracking for verification
    successful_writes: Arc<Mutex<Vec<WriteRecord>>>,
    reader_observations: Arc<Mutex<Vec<ReadRecord>>>,

    start_time: Instant,
}
```

### MetricsSummary (Revised)
```rust
pub struct MetricsSummary {
    pub duration: Duration,

    // Write metrics
    pub total_writes: u64,
    pub write_tps: f64,
    pub writer_p50_ms: f64,     // PRIMARY METRIC
    pub writer_p95_ms: f64,
    pub writer_p99_ms: f64,     // PRIMARY METRIC

    // Read metrics
    pub total_reads: u64,
    pub read_tps: f64,
    pub read_p50_ms: f64,       // PRIMARY METRIC
    pub read_p95_ms: f64,
    pub read_p99_ms: f64,       // PRIMARY METRIC

    // Verification (should be 0 for baseline)
    pub total_precondition_failures: u64,
    pub precondition_failure_rate: f64,
}
```

---

## Implementation Plan (Revised)

### Phase 6.1: Core Refactoring ✅ DONE (with modifications)
- [x] KeyRegistry implemented
- [x] MonotonicWriter ClientType added
- [x] run_monotonic_write_transaction() implemented
- [ ] **REMOVE** Compactor code
- [ ] **SIMPLIFY** WorkloadConfig (remove compactor fields)
- [ ] **SIMPLIFY** metrics (remove compactor histograms)

### Phase 6.2: Simplified Metrics ⏳ IN PROGRESS
- [ ] Remove compactor-related fields from MetricsCollector
- [ ] Add simple methods:
  - `record_writer_success(latency)` - no retry tracking needed for baseline
  - `record_writer_precond_failure(latency)` - track but should be 0
  - `record_read(latency)` - already exists
- [ ] Update MetricsSummary with simplified fields
- [ ] Update summary() to calculate latency percentiles
- [ ] Update print_report() to emphasize latency

### Phase 6.3: Test Updates ⏳ NEXT
- [ ] Update `utils.rs::generate_all_configs_v2()` - 15 configs instead of 60
- [ ] Update `workload.rs` - spawn monotonic writers (no compactor)
- [ ] Update `performance_test.rs::test_baseline`:
  - 1 writer @ 0.1 tps, 2 readers @ 100 tps
  - 2 minutes duration
  - Export to `test_baseline_v2.csv`
  - Print latency report
- [ ] Update `performance_test.rs::test_comprehensive_sweep`:
  - Use `generate_all_configs_v2()`
  - 15 configs, parallel execution
  - Export to `comprehensive_sweep_v2.csv`

### Phase 6.4: Simplified Visualization
- [ ] Update `visualization.rs` CSV columns:
  - config_label, num_writers, num_readers, writer_rate, reader_rate
  - duration_secs, total_writes, total_reads
  - writer_p50_ms, writer_p95_ms, writer_p99_ms
  - read_p50_ms, read_p95_ms, read_p99_ms
  - precondition_failure_rate (should be ~0%)
- [ ] Create `plot_latency_sweep()` in plot_results.py:
  - Line plot with connected dots
  - X-axis: Config labels
  - Y-axis: Latency (ms)
  - 4 lines: writer p50/p99, reader p50/p99

### Phase 6.5: Run Baseline Test
- [ ] Compile and run baseline
- [ ] Verify latency measurements
- [ ] Check precondition failure rate (expect 0%)
- [ ] Validate monotonic key writes

---

## Expected Baseline Results

### Single Writer (No Conflicts!)
```
Writer Latency:
  p50: ~1.5s  (includes S3 RTT + orphan recovery + commit)
  p95: ~2.5s
  p99: ~3.0s

Reader Latency:
  p50: ~1.0s  (snapshot + S3 get)
  p95: ~2.0s
  p99: ~2.5s

Precondition Failures: 0  (only 1 writer, no conflicts!)
Total Writes: ~12
Total Reads: ~24,000
```

### Multi-Writer Sweep (Conflicts Expected)
With 2-3 writers, we'll see:
- **Latency increase** due to retry overhead
- **Precondition failures** when writers conflict on HEAD
- **Trend**: Higher writer_rate → more conflicts → higher p99 latency

---

## Simplified File Changes

### Files to Modify
1. ✅ `utils.rs` - Simplify WorkloadConfig, update generate_all_configs_v2()
2. ✅ `client.rs` - Remove compactor code
3. 🚧 `metrics.rs` - Simplify, remove compactor fields
4. ⏸️ `workload.rs` - Spawn monotonic writers only
5. ⏸️ `visualization.rs` - Simplified CSV export
6. ⏸️ `performance_test.rs` - Update baseline and sweep tests
7. ⏸️ `plot_results.py` - Create latency line plot

### Removed Complexity
- ❌ Compactor ClientType
- ❌ run_compactor_transaction()
- ❌ compactor_latency histogram
- ❌ compactor_precond_failures counters
- ❌ compactor_rate, compactor_sleep_secs configs
- ❌ Retry failure tracking (not needed for baseline)
- ❌ 60-config sweep (reduced to 15)

---

## CSV Format (Simplified)

```csv
config_label,num_writers,num_readers,writer_rate,reader_rate,duration_secs,
total_writes,total_reads,write_tps,read_tps,
writer_p50_ms,writer_p95_ms,writer_p99_ms,
read_p50_ms,read_p95_ms,read_p99_ms,
precondition_failure_rate
```

**Example Row:**
```
W1_WR0.10_RD2_RT100,1,2,0.1,100.0,120,
12,24000,0.10,200.0,
1520.5,2450.3,3012.8,
1050.2,1980.5,2450.0,
0.0
```

---

## Visualization Design

### plot_latency_sweep(csv_file)
```python
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv(csv_file)

fig, ax = plt.subplots(figsize=(12, 6))

# X-axis: Config labels (sorted by writer_rate, then num_writers)
df = df.sort_values(['num_writers', 'writer_rate'])
x = range(len(df))
labels = df['config_label'].tolist()

# Plot writer latency
ax.plot(x, df['writer_p50_ms'], 'o-', color='blue', label='Writer p50', linewidth=2)
ax.plot(x, df['writer_p99_ms'], 'o--', color='blue', label='Writer p99', linewidth=2)

# Plot reader latency
ax.plot(x, df['read_p50_ms'], 's-', color='green', label='Reader p50', linewidth=2)
ax.plot(x, df['read_p99_ms'], 's--', color='green', label='Reader p99', linewidth=2)

ax.set_xticks(x)
ax.set_xticklabels(labels, rotation=45, ha='right')
ax.set_xlabel('Configuration')
ax.set_ylabel('Latency (ms)')
ax.set_title('Latency vs Configuration')
ax.legend()
ax.grid(True, alpha=0.3)

plt.tight_layout()
plt.savefig('latency_sweep.png', dpi=300)
```

---

## Success Criteria (Revised)

### Baseline Test
- [ ] Compiles without errors
- [ ] Runs to completion (2 minutes)
- [ ] Generates ~12 monotonic writes (key_000000 to key_000011)
- [ ] Precondition failures = 0 (critical!)
- [ ] Writer p99 < 5s (reasonable latency)
- [ ] Reader p99 < 3s (reasonable latency)
- [ ] CSV export successful
- [ ] Monotonic write verification passes

### Comprehensive Sweep
- [ ] 15 configs complete in ~2 minutes
- [ ] Latency trends visible (higher rate/writers → higher latency)
- [ ] Precondition failures increase with more writers
- [ ] Plot shows clear latency progression
- [ ] No crashes or panics

---

## Timeline (Revised)

| Phase | Task | Time |
|-------|------|------|
| 6.1 | Simplify existing code (remove compactor) | 30 min |
| 6.2 | Simplify metrics | 30 min |
| 6.3 | Update tests | 45 min |
| 6.4 | Update visualization | 30 min |
| 6.5 | Run baseline test | 10 min |
| **Total** | | **2.5 hours** |

Much faster than original 6-hour estimate!

---

## Next Actions

1. **Remove compactor code** from client.rs
2. **Simplify WorkloadConfig** - remove all compactor fields
3. **Simplify MetricsCollector** - remove compactor histograms
4. **Update generate_all_configs_v2()** - 15 configs instead of 60
5. **Update workload.rs** - spawn only writers and readers
6. **Update baseline test** - 1 writer + 2 readers
7. **Test compilation**
8. **Run baseline test**
9. **Verify latency results**

Let's focus on getting baseline working first, then sweep can follow!
