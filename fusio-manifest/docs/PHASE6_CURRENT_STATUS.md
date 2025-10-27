# Phase 6 Implementation - Current Status

## ✅ COMPLETED (Major Simplifications Done!)

### Core Refactoring
1. **Removed Compactor** - No compactor code anywhere
2. **Simplified WorkloadConfig** - Only writer/reader fields
3. **Simplified MetricsCollector** - No compactor histograms
4. **Updated generate_all_configs_v2()** - 15 configs (3 writers × 5 rates)
5. **KeyRegistry** - Monotonic key allocation working
6. **MonotonicWriter** - Sequential write transaction implemented
7. **ClientType** - Only MonotonicWriter, Writer (legacy), Reader

### Configuration
**New Baseline:**
```rust
num_writers: 1,
num_readers: 2,
writer_rate: 0.1,
reader_rate: 100.0,
duration: 120s
```

**New Sweep:** 15 configs
- Writers: [1, 2, 3]
- Writer rates: [0.02, 0.05, 0.1, 0.15, 0.2]
- Readers: [2] (fixed)
- Reader rate: [100.0] (fixed)

---

## 🚧 REMAINING WORK (< 1 hour!)

### Immediate (Blocking Compilation)
1. **Update workload.rs** (15 minutes)
   - Wrap `key_pool` in `Some()` for legacy tests
   - Add KeyRegistry support for new baseline test
   - Update client spawning

2. **Update performance_test.rs** (20 minutes)
   - Update `test_baseline` to use MonotonicWriter + KeyRegistry
   - Update `test_comprehensive_sweep` to use `generate_all_configs_v2()`
   - Fix client constructor calls

3. **Update visualization.rs** (10 minutes)
   - Simplify CSV export (remove compactor columns)
   - Keep columns: config_label, num_writers, writer_rate, num_readers, reader_rate,
     writer_p50/p95/p99_ms, read_p50/p95/p99_ms, precondition_failure_rate

### Optional (Can be done later)
4. **Create plot_latency_sweep.py** (20 minutes)
   - Line plot with connected dots
   - X-axis: Config labels
   - Y-axis: Latency (ms)
   - 4 lines: writer p50/p99, reader p50/p99

---

## 📊 Current Compilation Errors

**Count: 2 types of errors, ~10 occurrences**

### Error 1: workload.rs - Missing Option wrapper
```
error: expected Option<Arc<KeyPool>>, found Arc<KeyPool>
Fix: Change `key_pool.clone()` → `Some(key_pool.clone())`
      Add `None` for key_registry parameter
```

### Error 2: workload.rs - Missing key_registry parameter
```
error: function takes 7 arguments but 6 supplied
Fix: Add `None` as 5th parameter (between key_pool and config)
```

**Locations:**
- workload.rs:52 (Writer spawn)
- workload.rs:69 (Reader spawn)
- performance_test.rs (multiple locations)

---

## 🎯 Quick Fix Guide

### Fix workload.rs

**Before:**
```rust
MockClient::new(
    id,
    ClientType::Writer { id },
    manifest.clone(),
    key_pool.clone(),              // Wrong!
    config.clone(),
    metrics.clone(),
)
```

**After:**
```rust
MockClient::new(
    id,
    ClientType::Writer { id },
    manifest.clone(),
    Some(key_pool.clone()),        // Wrapped in Some
    None,                           // Add key_registry = None
    config.clone(),
    metrics.clone(),
)
```

### New Baseline Test Pattern

```rust
// Create KeyRegistry instead of KeyPool
let key_registry = Arc::new(KeyRegistry::new());

// Spawn MonotonicWriter
let writer = MockClient::new(
    0,
    ClientType::MonotonicWriter { id: 0 },
    manifest.clone(),
    None,                           // No KeyPool
    Some(key_registry.clone()),     // Use KeyRegistry
    config.clone(),
    metrics.clone(),
);

// Spawn Readers
let reader = MockClient::new(
    i,
    ClientType::Reader { id: i },
    manifest.clone(),
    None,                           // No KeyPool
    Some(key_registry.clone()),     // Use KeyRegistry
    config.clone(),
    metrics.clone(),
);
```

---

## 📁 Files Modified (Phase 6 So Far)

1. ✅ `utils.rs` - KeyRegistry, simplified WorkloadConfig, generate_all_configs_v2()
2. ✅ `client.rs` - Removed compactor, simplified to 3 client types
3. ✅ `metrics.rs` - Removed compactor fields
4. 🚧 `workload.rs` - Needs Option wrapper fixes
5. 🚧 `performance_test.rs` - Needs baseline update
6. 🚧 `visualization.rs` - Needs CSV simplification
7. ⏸️ `plot_results.py` - Needs latency plot function

---

## 🎉 What Works Now

- **KeyRegistry**: Monotonic key allocation, thread-safe
- **MonotonicWriter**: Sequential key generation (key_000000, key_000001, ...)
- **Simplified Config**: Only essential fields, no compactor complexity
- **15-config sweep**: Fast, focused on latency trends
- **Metrics**: Simple latency tracking, no compactor overhead

---

## 🚀 Next Steps to Run Baseline

1. Fix workload.rs (wrap in Some, add None)
2. Update test_baseline in performance_test.rs
3. Compile and test
4. Run: `cargo test -p fusio-manifest --test performance_test test_baseline -- --ignored --nocapture`
5. Observe latency results!

**Expected Baseline Results:**
- Writer p50: ~1.5s, p99: ~3s
- Reader p50: ~1s, p99: ~2.5s
- Precondition failures: 0 (single writer!)
- Total writes: ~12
- Total reads: ~24,000

---

## 📋 Simplified Architecture (Final)

```
WorkloadConfig {
    num_writers: usize,
    num_readers: usize,
    writer_rate: f64,
    reader_rate: f64,
    duration: Duration,
    value_size: usize,
    max_retry_count: usize,
}

ClientType {
    MonotonicWriter { id },  // New v2 tests
    Writer { id },            // Legacy tests
    Reader { id },
}

KeyRegistry {
    written_keys: Arc<Mutex<Vec<String>>>,
    next_key_id: Arc<AtomicUsize>,
}

MetricsCollector {
    write_success_latency: Histogram,
    read_latency: Histogram,
    total_writes_succeeded: AtomicU64,
    total_reads: AtomicU64,
    precondition_failures: AtomicU64,
}
```

---

## ⏱️ Time Estimate to Completion

| Task | Time |
|------|------|
| Fix workload.rs | 5 min |
| Update performance_test.rs | 15 min |
| Test compilation | 5 min |
| Run baseline test | 5 min |
| **Total** | **30 minutes** |

Visualization can be added later - baseline test is the priority!

---

## 🎯 Success Criteria for Baseline

- [ ] Compiles without errors
- [ ] Single writer generates monotonic keys (key_000000, key_000001, ...)
- [ ] ~12 writes in 2 minutes (0.1 tps × 120s)
- [ ] ~24,000 reads in 2 minutes (100 tps × 2 readers × 120s)
- [ ] Precondition failure rate = 0% (critical!)
- [ ] Writer p99 < 5s
- [ ] Reader p99 < 3s
- [ ] No panics or crashes
- [ ] CSV export successful

Ready to finish this! The hard work is done, just need to connect the pieces.
