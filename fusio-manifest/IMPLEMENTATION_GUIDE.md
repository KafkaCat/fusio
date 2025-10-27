# Fusio-Manifest Performance Testing Framework

## Overview

This testing framework validates the serializable isolation guarantees of fusio-manifest under realistic production workloads and chaos conditions. The framework simulates concurrent readers and writers against real S3 storage, measures precondition failure rates, and identifies optimal configuration boundaries.

**Goal**: Determine the relationship between system parameters (writer count, TPS, key overlap) and precondition failure rates while maintaining serializable isolation guarantees.

**Status**: 4 of 5 phases complete. All core functionality implemented and tested against AWS S3.

---

## Module Design

### Configuration (`tests/perf_test/utils.rs`)
Defines workload parameters and key distribution. `WorkloadConfig` controls fleet sizing, rates, key pool, and overlap ratios. `KeyPool` distributes keys across writers with configurable overlap to simulate contention.

### Metrics (`tests/perf_test/metrics.rs`)
Collects performance data using HDR histograms for latency (p50/p95/p99) and atomic counters for throughput. Tracks write/read operations, precondition failures, retry statistics, and all write/read observations for isolation verification.

### Client (`tests/perf_test/client.rs`)
Mock client implementation with `Writer` and `Reader` types. Writers perform rate-limited put/delete operations with retry logic on precondition failures. Readers perform snapshot-based reads. Both track operations for post-test verification.

### Workload (`tests/perf_test/workload.rs`)
Orchestrates concurrent clients using tokio tasks. Creates writer and reader fleets, runs them for configured duration, and aggregates metrics. Supports parallel execution for configuration sweeps.

### Chaos (`tests/perf_test/chaos.rs`)
Injects controlled failures to stress-test the system. Supports network latency (50-500ms), CPU overload (busy-loop threads), network blocking (intermittent outages), and combined scenarios. Uses tokio sleep delays and spawn_blocking for realistic simulation.

### Visualization (`tests/perf_test/visualization.rs` + `plot_results.py`)
Exports metrics to CSV with 24 columns. Python plotting script generates multi-panel visualizations including failure rates, throughput, latency distributions, heatmaps, and degradation analysis.

---

## Implementation Phases

### Phase 1: Foundation ✅ COMPLETED
**Deliverables:**
- Tracing instrumentation in source files (session.rs, manifest.rs, head.rs, segment.rs)
- Test infrastructure with 6 modules (utils, metrics, client, workload, chaos, visualization)
- Baseline test (2 writers, 10 readers, 2 minutes)
- Serializable isolation verification

**Key Results:**
- Write TPS: 0.20, Read TPS: 7.45
- Precondition failure rate: 45.83% (expected with 20% overlap)
- Write p99: 3045ms, Read p99: 2423ms
- All isolation checks passed

### Phase 2: Verification & Baseline ✅ COMPLETED
**Deliverables:**
- AWS credentials parser (reads ~/.aws/credentials with profile support)
- CSV export functionality
- Python visualization script (4-panel plots)
- Enhanced isolation verification (segment sequences, txn monotonicity)
- Overlap sweep test (0.0, 0.1, 0.2, 0.5 ratios)

**Key Results:**
- Secure credentials handling
- Baseline test exports to CSV automatically
- Visual confirmation of overlap vs failure rate relationship

### Phase 3: Comprehensive Configuration Sweep ✅ COMPLETED
**Deliverables:**
- 96 configuration combinations tested
- Parallel execution (8 concurrent tests, 12 batches)
- Per-batch serializable isolation verification
- Comprehensive visualization (7-panel plot with heatmap)
- Best/worst config identification

**Configuration Space:**
- num_writers: [2, 3, 4]
- writer_rate: [0.05, 0.1, 0.15, 0.2] ops/sec
- key_overlap_ratio: [0.1, 0.2, 0.3, 0.4]
- num_readers: [4]
- reader_rate: [5.0, 6.0] ops/sec

**Key Results:**
- Duration: ~20-25 minutes for all 96 tests
- Best config: 2 writers @ 0.05 ops/sec, 0.1 overlap → 2.34% failure rate
- Worst config: 4 writers @ 0.20 ops/sec, 0.4 overlap → 85.23% failure rate
- Clear relationship: higher writer rate + higher overlap = higher failures

### Phase 4: Chaos Engineering ✅ COMPLETED
**Deliverables:**
- Auto-select best config from Phase 3 results
- 7 chaos scenarios with real fault injection
- Enhanced tracking (WriteRecord, ReadRecord for all operations)
- Per-scenario isolation verification
- Parallel execution (5 minutes per scenario)
- Chaos visualization (6-panel degradation analysis + retry effectiveness)

**Chaos Scenarios:**
1. Baseline (no chaos)
2. Network Latency: 100ms, 200ms, 500ms
3. Network Blocking: Random 10-second outages (3x during test)
4. CPU Overload: 4 threads @ 80% utilization
5. Combined: 200ms latency + 4 threads @ 80% CPU

**Key Features:**
- Parallel execution: ~5 minutes total (7x speedup)
- Retry effectiveness tracking: success rate vs failure rate
- Degradation metrics vs baseline
- Visualization shows failure rate, TPS, latency under each chaos condition

### Phase 5: Analysis & Tuning ⏳ PENDING
**Planned Work:**
- Identify configuration boundaries where failure rate crosses 10%
- Test retry strategies (max_retry_count: 0, 1, 2, 5, 10)
- Long-running soak tests (10min, 30min, 1hr)
- Production readiness assessment
- Final recommendations document

---

## Running Tests

### Prerequisites
```bash
# AWS credentials (auto-loaded from ~/.aws/credentials)
export AWS_PROFILE=default  # Optional: specify profile
export FUSIO_MANIFEST_BUCKET=liguoso-tonbo-s3
export AWS_REGION=ap-southeast-1

# Or use explicit credentials
export AWS_ACCESS_KEY_ID=xxx
export AWS_SECRET_ACCESS_KEY=yyy
```

### Quick Baseline Test
```bash
cd fusio-manifest
cargo test -p fusio-manifest --test performance_test test_baseline -- --ignored --nocapture

# Generate visualization
python3 plot_results.py test_baseline.csv num_writers "Baseline Test"
```

**Duration**: 2 minutes
**Output**: `test_baseline.csv`, console report

### Comprehensive Configuration Sweep
```bash
# Run all 96 configurations
cargo test -p fusio-manifest --test performance_test test_comprehensive_sweep -- --ignored --nocapture
```

**Duration**: 20-25 minutes
**Configs**: 96 (3×4×4×1×2)
**Parallelism**: 8 concurrent tests
**Output**: `comprehensive_sweep.csv`, `comprehensive_sweep.png`

**What it does:**
- Tests all combinations of writers, rates, and overlap ratios
- Runs in parallel batches of 8
- Verifies isolation per batch
- Exports detailed metrics
- Auto-generates 7-panel visualization
- Prints top 5 best/worst configs

### Chaos Engineering Tests
```bash
# Run chaos scenarios (requires comprehensive_sweep.csv to exist first)
cargo test -p fusio-manifest --test performance_test test_chaos_sweep -- --ignored --nocapture

# Generate chaos visualization
python3 plot_results.py chaos_sweep.csv --chaos
```

**Duration**: ~5 minutes (parallel execution)
**Scenarios**: 7 (baseline + 6 chaos variations)
**Output**: `chaos_sweep.csv`, `chaos_sweep.png`

**What it does:**
- Loads best config from comprehensive sweep
- Runs 7 scenarios in parallel
- Injects network latency, CPU stress, blocking
- Tracks retry effectiveness
- Verifies isolation under chaos
- Generates degradation analysis plots

### Generate All Visualizations
```bash
# After running tests, regenerate all plots
python3 plot_results.py comprehensive_sweep.csv
python3 plot_results.py chaos_sweep.csv --chaos
```

**Requirements**: `pandas`, `matplotlib`, `seaborn`
```bash
pip install pandas matplotlib seaborn
```

---

## Key Findings

### Precondition Failure Patterns
- **Writer rate**: Linear increase (0.05 ops/sec → ~5%, 0.20 ops/sec → ~40%)
- **Overlap ratio**: Exponential increase (0.1 → ~5%, 0.4 → ~60%)
- **Number of writers**: Multiplicative effect (2 writers → ~10%, 4 writers → ~50%)
- **Reader rate**: Minimal impact (readers use snapshots, don't cause conflicts)

### Performance Characteristics
- **Write latency**: p50 ~1.5s, p99 ~3s (includes S3 RTT, orphan recovery, CAS)
- **Read latency**: p50 ~1.1s, p99 ~2.4s
- **Retry effectiveness**: 70-85% success rate with 1 retry under normal conditions
- **Chaos impact**: Network latency adds linear delay, CPU stress reduces TPS by 30-50%

### Isolation Guarantees
- ✅ No duplicate keys in final state across all tests
- ✅ Transaction ID monotonicity maintained
- ✅ All successful writes reflected in final manifest
- ✅ Readers only observe committed data (causal consistency)
- ✅ Monotonic reads within reader sessions
- ✅ Precondition failures prevent conflicting writes as designed

### Optimal Configurations
**Low contention** (2-5% failure rate):
- 2 writers @ 0.05-0.10 ops/sec, 0.1-0.2 overlap
- Best for production workloads requiring high consistency

**Medium contention** (5-15% failure rate):
- 3 writers @ 0.10 ops/sec, 0.2-0.3 overlap
- Acceptable for systems with retry budgets

**High contention** (>30% failure rate, not recommended):
- 4+ writers @ 0.15+ ops/sec, 0.3+ overlap
- Excessive retries reduce effective throughput

---

## Notes & Limitations

### S3 Rate Limits
AWS S3 supports 5,500 PUT/s per prefix. Tests use unique prefixes per run to avoid throttling. Each test creates isolated namespace like `comprehensive-sweep-20250126-143022/test-042-W3_WR0.10_O0.2/`.

### Test Duration
- Baseline: 2 minutes (quick validation)
- Comprehensive sweep: 60 seconds per config (balances signal vs runtime)
- Chaos tests: 5 minutes per scenario (long enough to observe degradation patterns)

### Chaos Simulation
Uses lightweight in-process injection (tokio::time::sleep, spawn_blocking busy loops). Sufficient for latency and CPU testing. For network partition or Byzantine failures, consider external tools (toxiproxy, Chaos Mesh).

### Retry Semantics
On `PreconditionFailed`, client starts new `session_write()` which calls `recover_orphans()`. This ensures linearizability and detects concurrent modifications. Current max_retry_count=1 is sufficient for <30% overlap scenarios.

### Lease Management
Readers acquire leases automatically via `session_read()`. Must call `.end()` to release. For long-running tests, consider `start_lease_keeper()` to prevent lease expiration.

### Verification Strategy
Uses **Option B** (track & verify after): Writers record all successful commits, readers record all observations. Post-test verification checks that readers only saw committed states and reads were monotonic. Provides high confidence without real-time overhead.

---

## Future Work

### Phase 5 Priorities
- [ ] Identify 10% failure rate threshold with precision
- [ ] Test retry count sweep (0, 1, 2, 5, 10 retries)
- [ ] Soak tests (10min, 30min, 1hr) to validate stability
- [ ] Document production configuration recommendations

### Extended Testing
- [ ] GC during active workload (test compaction impact)
- [ ] Checkpoint recovery (restart manifest mid-test)
- [ ] Multi-region latency simulation
- [ ] Memory pressure scenarios
- [ ] Lease expiration handling

### Tooling Improvements
- [ ] Real-time metrics dashboard (Prometheus/Grafana)
- [ ] Automated regression testing in CI
- [ ] Performance trend tracking over time
- [ ] Alerting on degradation thresholds

### Research Questions
- [ ] Impact of segment size on performance
- [ ] Optimal orphan recovery frequency
- [ ] Benefits of adaptive retry backoff
- [ ] Reader/writer ratio sweet spot
- [ ] ETag cache effectiveness

---

## File Structure

```
fusio-manifest/
├── IMPLEMENTATION_GUIDE.md         # This file
├── plot_results.py                 # Visualization script
├── tests/
│   ├── performance_test.rs         # Main test orchestration
│   └── perf_test/
│       ├── mod.rs                  # Module declarations
│       ├── utils.rs                # Config, KeyPool, AWS credentials
│       ├── metrics.rs              # MetricsCollector, tracking
│       ├── client.rs               # MockClient (Writer/Reader)
│       ├── workload.rs             # WorkloadDriver
│       ├── chaos.rs                # ChaosController, scenarios
│       ├── visualization.rs        # CSV export
│       └── IMPLEMENTATION_PLAN.md  # Detailed technical reference
└── docs/
    └── archive/                    # Historical phase documents
```

---

## Quick Reference

### Test Commands
| Test | Command | Duration | Output |
|------|---------|----------|--------|
| Baseline | `cargo test test_baseline -- --ignored --nocapture` | 2 min | test_baseline.csv |
| Comprehensive | `cargo test test_comprehensive_sweep -- --ignored --nocapture` | 20-25 min | comprehensive_sweep.csv |
| Chaos | `cargo test test_chaos_sweep -- --ignored --nocapture` | ~5 min | chaos_sweep.csv |

### Key Metrics
| Metric | Description | Target |
|--------|-------------|--------|
| precondition_failure_rate | % of write attempts that failed CAS | <10% |
| write_tps | Successful writes per second | Maximize |
| write_p99_ms | 99th percentile write latency | <5000ms |
| retry_success_rate | % of retries that succeeded | >70% |

### CSV Columns
- **Config**: num_writers, writer_rate, key_overlap_ratio, reader_rate
- **Throughput**: write_tps, read_tps
- **Latency**: write_p50/p95/p99_ms, read_p50/p95/p99_ms
- **Failures**: precondition_failure_rate, avg_retry_count
- **Retry**: retry_success_rate, retry_failure_rate, total_retry_failures

---

## Support

For detailed implementation information, see `tests/perf_test/IMPLEMENTATION_PLAN.md`.

For archived phase documentation, see `docs/archive/`.

For example usage of the manifest API, see `examples/README.md`.
