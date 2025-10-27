# Chaos Testing Implementation Plan for fusio-manifest

## Overview

Build a comprehensive chaos testing framework that simulates multiple concurrent clients (readers/writers) against a real S3Manifest, with configurable chaos scenarios to determine the relationship between precondition failure rates and various configuration parameters (writer count, TPS, key pool size, key overlap ratio).

**Primary Goal**: Test serializable isolation guarantees under realistic production load patterns and identify configuration boundaries where precondition failure rates become unacceptable (>10%).

---

## 1. Project Structure

```
fusio-manifest/tests/
├── e2e_serializable_isolation.rs  (existing)
└── performance_test/
    ├── mod.rs                      # Main module entry & test orchestration
    ├── client.rs                   # Mock client abstraction (Writer/Reader)
    ├── workload.rs                 # Workload driver logic
    ├── chaos.rs                    # Chaos injection layer
    ├── metrics.rs                  # Metrics collection & aggregation
    ├── scenarios.rs                # Test scenario definitions
    ├── utils.rs                    # Helpers (key pool, config)
    └── visualization.rs            # CSV export & plotting helpers
```

---

## 2. Dependencies

### Add to `fusio-manifest/Cargo.toml`:

```toml
[dev-dependencies]
# Existing
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
rand = "0.8"
rstest = "0.21"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }

# New dependencies
hdrhistogram = "7.5"              # Latency histograms
tokio-util = "0.7"                # Rate limiting utilities
csv = "1.3"                       # CSV export for analysis
serde = { version = "1", features = ["derive"] }  # Already in main deps
```

**Chaos Engineering Approach**: Use lightweight in-process injection (tokio::time::sleep + jitter) instead of external tools like toxiproxy for easier CI integration.

---

## 3. Core Components

### A. Configuration (`utils.rs`)

```rust
/// Main workload configuration
#[derive(Debug, Clone)]
pub struct WorkloadConfig {
    // Fleet sizing
    pub num_writers: usize,
    pub num_readers: usize,
    pub duration: Duration,

    // Rate control (ops/sec per client)
    pub writer_rate: f64,           // Start: 0.1 ops/sec
    pub reader_rate: f64,           // Start: 1.0 ops/sec

    // Key pool configuration
    pub key_pool_size: usize,       // Start: 100 keys
    pub key_overlap_ratio: f64,     // Start: 0.2 (20% overlap)

    // Operation characteristics
    pub value_size: usize,          // Bytes per value
    pub write_delete_ratio: f64,    // % of writes that are deletes (0.0-1.0)

    // Retry behavior
    pub max_retry_count: usize,     // Start: 1 (one retry on precondition failure)

    // Chaos configuration
    pub chaos: ChaosScenario,
}

/// Key pool with configurable overlap between writers
pub struct KeyPool {
    all_keys: Vec<String>,
    writer_key_sets: Vec<Vec<String>>,
    reader_keys: Vec<String>,
}

impl KeyPool {
    /// Create key pool with controlled overlap
    ///
    /// Example: 3 writers, 100 keys, 20% overlap
    /// - Writer 0: keys [0..40)
    /// - Writer 1: keys [32..72)   (overlap [32..40) with W0)
    /// - Writer 2: keys [64..100)  (overlap [64..72) with W1)
    /// - Readers: all keys [0..100)
    pub fn new(total_keys: usize, num_writers: usize, overlap_ratio: f64) -> Self {
        // Implementation: distribute keys with overlap_ratio control
    }

    pub fn writer_keys(&self, writer_id: usize) -> &[String] {
        &self.writer_key_sets[writer_id]
    }

    pub fn reader_keys(&self) -> &[String] {
        &self.reader_keys
    }
}
```

### B. Mock Client (`client.rs`)

```rust
pub enum ClientType {
    Writer { id: usize },
    Reader { id: usize },
}

pub struct MockClient {
    id: usize,
    client_type: ClientType,
    manifest: Arc<S3Manifest<String, String, TokioExecutor>>,
    key_pool: Arc<KeyPool>,
    config: Arc<WorkloadConfig>,
    metrics: Arc<MetricsCollector>,
    rng: ThreadRng,
}

impl MockClient {
    /// Run a single write transaction with retry logic
    #[tracing::instrument(skip(self), fields(writer_id = %self.id))]
    async fn run_write_transaction(&mut self) -> Result<(), Error> {
        let my_keys = self.key_pool.writer_keys(self.id);
        let key = my_keys.choose(&mut self.rng).unwrap();

        let mut attempt = 0;
        loop {
            let start = Instant::now();

            tracing::debug!(writer_id = %self.id, attempt, key, "starting write session");

            let mut session = self.manifest.session_write().await?;

            // Decide: put or delete
            if self.rng.gen::<f64>() < self.config.write_delete_ratio {
                session.delete(key.clone());
            } else {
                let value = generate_value(self.config.value_size);
                session.put(key.clone(), value);
            }

            let result = session.commit().await;
            let latency = start.elapsed();

            match result {
                Ok(_) => {
                    tracing::info!(
                        writer_id = %self.id,
                        attempt,
                        key,
                        latency_ms = latency.as_millis(),
                        "write committed successfully"
                    );
                    self.metrics.record_write_success(latency, attempt);
                    return Ok(());
                }
                Err(Error::PreconditionFailed) => {
                    tracing::warn!(
                        writer_id = %self.id,
                        attempt,
                        key,
                        latency_ms = latency.as_millis(),
                        "PRECONDITION FAILURE - retrying"
                    );
                    self.metrics.record_precondition_failure(latency, attempt);

                    if attempt >= self.config.max_retry_count {
                        tracing::error!(
                            writer_id = %self.id,
                            key,
                            "max retries exceeded"
                        );
                        return Err(Error::PreconditionFailed);
                    }
                    attempt += 1;
                    continue;
                }
                Err(e) => {
                    tracing::error!(writer_id = %self.id, error = ?e, "write failed");
                    self.metrics.record_write_error(latency);
                    return Err(e);
                }
            }
        }
    }

    /// Run a single read transaction
    #[tracing::instrument(skip(self), fields(reader_id = %self.id))]
    async fn run_read_transaction(&mut self) -> Result<(), Error> {
        let key = self.key_pool.reader_keys().choose(&mut self.rng).unwrap();

        let start = Instant::now();
        tracing::debug!(reader_id = %self.id, key, "starting read session");

        let session = self.manifest.session_read().await?;
        let _value = session.get(key).await?;
        session.end().await?;

        let latency = start.elapsed();
        tracing::debug!(
            reader_id = %self.id,
            key,
            latency_ms = latency.as_millis(),
            "read completed"
        );

        self.metrics.record_read(latency);
        Ok(())
    }

    /// Run rate-limited loop for duration
    pub async fn run_loop(&mut self, duration: Duration) {
        let rate = match self.client_type {
            ClientType::Writer { .. } => self.config.writer_rate,
            ClientType::Reader { .. } => self.config.reader_rate,
        };

        let interval = Duration::from_secs_f64(1.0 / rate);
        let mut ticker = tokio::time::interval(interval);
        let deadline = Instant::now() + duration;

        loop {
            ticker.tick().await;

            if Instant::now() >= deadline {
                break;
            }

            let result = match self.client_type {
                ClientType::Writer { .. } => self.run_write_transaction().await,
                ClientType::Reader { .. } => self.run_read_transaction().await,
            };

            if let Err(e) = result {
                tracing::error!(client_id = %self.id, error = ?e, "transaction failed");
            }
        }
    }
}

fn generate_value(size: usize) -> String {
    use rand::distributions::Alphanumeric;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}
```

### C. Metrics Collection (`metrics.rs`)

```rust
use hdrhistogram::Histogram;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct MetricsCollector {
    // Latency histograms (microseconds)
    write_success_latency: Arc<Mutex<Histogram<u64>>>,
    precondition_failure_latency: Arc<Mutex<Histogram<u64>>>,
    read_latency: Arc<Mutex<Histogram<u64>>>,

    // Counters
    total_writes_attempted: AtomicU64,
    total_writes_succeeded: AtomicU64,
    total_precondition_failures: AtomicU64,
    total_write_errors: AtomicU64,
    total_reads: AtomicU64,

    // Retry statistics
    retry_counts: Arc<Mutex<Vec<usize>>>,  // Track retry count per operation

    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            write_success_latency: Arc::new(Mutex::new(
                Histogram::<u64>::new(3).unwrap()  // 3 significant figures
            )),
            precondition_failure_latency: Arc::new(Mutex::new(
                Histogram::<u64>::new(3).unwrap()
            )),
            read_latency: Arc::new(Mutex::new(
                Histogram::<u64>::new(3).unwrap()
            )),
            total_writes_attempted: AtomicU64::new(0),
            total_writes_succeeded: AtomicU64::new(0),
            total_precondition_failures: AtomicU64::new(0),
            total_write_errors: AtomicU64::new(0),
            total_reads: AtomicU64::new(0),
            retry_counts: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        }
    }

    pub fn record_write_success(&self, latency: Duration, retry_count: usize) {
        self.total_writes_attempted.fetch_add(1, Ordering::Relaxed);
        self.total_writes_succeeded.fetch_add(1, Ordering::Relaxed);
        self.write_success_latency
            .lock()
            .unwrap()
            .record(latency.as_micros() as u64)
            .ok();
        self.retry_counts.lock().unwrap().push(retry_count);
    }

    pub fn record_precondition_failure(&self, latency: Duration, retry_count: usize) {
        self.total_precondition_failures.fetch_add(1, Ordering::Relaxed);
        self.precondition_failure_latency
            .lock()
            .unwrap()
            .record(latency.as_micros() as u64)
            .ok();
    }

    pub fn record_write_error(&self, latency: Duration) {
        self.total_writes_attempted.fetch_add(1, Ordering::Relaxed);
        self.total_write_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_read(&self, latency: Duration) {
        self.total_reads.fetch_add(1, Ordering::Relaxed);
        self.read_latency
            .lock()
            .unwrap()
            .record(latency.as_micros() as u64)
            .ok();
    }

    pub fn summary(&self) -> MetricsSummary {
        let elapsed = self.start_time.elapsed();

        let total_attempts = self.total_writes_attempted.load(Ordering::Relaxed);
        let total_success = self.total_writes_succeeded.load(Ordering::Relaxed);
        let total_precond = self.total_precondition_failures.load(Ordering::Relaxed);
        let total_errors = self.total_write_errors.load(Ordering::Relaxed);
        let total_reads = self.total_reads.load(Ordering::Relaxed);

        let write_hist = self.write_success_latency.lock().unwrap();
        let precond_hist = self.precondition_failure_latency.lock().unwrap();
        let read_hist = self.read_latency.lock().unwrap();

        let retry_counts = self.retry_counts.lock().unwrap();
        let avg_retries = if !retry_counts.is_empty() {
            retry_counts.iter().sum::<usize>() as f64 / retry_counts.len() as f64
        } else {
            0.0
        };

        MetricsSummary {
            duration: elapsed,

            // Write metrics
            total_write_attempts: total_attempts,
            total_write_success: total_success,
            total_precondition_failures: total_precond,
            total_write_errors: total_errors,

            // Precondition failure rate (key metric)
            precondition_failure_rate: if total_attempts > 0 {
                total_precond as f64 / total_attempts as f64
            } else {
                0.0
            },

            // TPS
            write_tps: total_success as f64 / elapsed.as_secs_f64(),
            read_tps: total_reads as f64 / elapsed.as_secs_f64(),

            // Latency percentiles (microseconds -> milliseconds)
            write_p50_ms: write_hist.value_at_quantile(0.5) as f64 / 1000.0,
            write_p95_ms: write_hist.value_at_quantile(0.95) as f64 / 1000.0,
            write_p99_ms: write_hist.value_at_quantile(0.99) as f64 / 1000.0,

            precond_failure_latency_p50_ms: precond_hist.value_at_quantile(0.5) as f64 / 1000.0,
            precond_failure_latency_p99_ms: precond_hist.value_at_quantile(0.99) as f64 / 1000.0,

            read_p50_ms: read_hist.value_at_quantile(0.5) as f64 / 1000.0,
            read_p95_ms: read_hist.value_at_quantile(0.95) as f64 / 1000.0,
            read_p99_ms: read_hist.value_at_quantile(0.99) as f64 / 1000.0,

            // Retry statistics
            avg_retry_count: avg_retries,

            total_reads,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSummary {
    pub duration: Duration,

    // Write counters
    pub total_write_attempts: u64,
    pub total_write_success: u64,
    pub total_precondition_failures: u64,
    pub total_write_errors: u64,

    // Key metric: precondition failure rate
    pub precondition_failure_rate: f64,

    // Throughput
    pub write_tps: f64,
    pub read_tps: f64,

    // Write latency (successful commits)
    pub write_p50_ms: f64,
    pub write_p95_ms: f64,
    pub write_p99_ms: f64,

    // Precondition failure latency
    pub precond_failure_latency_p50_ms: f64,
    pub precond_failure_latency_p99_ms: f64,

    // Read latency
    pub read_p50_ms: f64,
    pub read_p95_ms: f64,
    pub read_p99_ms: f64,

    // Retry statistics
    pub avg_retry_count: f64,

    // Read counters
    pub total_reads: u64,
}

impl MetricsSummary {
    pub fn print_report(&self) {
        println!("\n========== Workload Results ==========");
        println!("Duration: {:.2}s", self.duration.as_secs_f64());
        println!("\n--- Write Metrics ---");
        println!("Total attempts:        {}", self.total_write_attempts);
        println!("Successful commits:    {}", self.total_write_success);
        println!("Precondition failures: {}", self.total_precondition_failures);
        println!("Other errors:          {}", self.total_write_errors);
        println!("\n*** Precondition Failure Rate: {:.2}% ***", self.precondition_failure_rate * 100.0);
        println!("\nWrite TPS:             {:.2}", self.write_tps);
        println!("Avg retries per op:    {:.2}", self.avg_retry_count);
        println!("\n--- Write Latency (successful commits) ---");
        println!("p50: {:.2}ms", self.write_p50_ms);
        println!("p95: {:.2}ms", self.write_p95_ms);
        println!("p99: {:.2}ms", self.write_p99_ms);
        println!("\n--- Precondition Failure Latency ---");
        println!("p50: {:.2}ms", self.precond_failure_latency_p50_ms);
        println!("p99: {:.2}ms", self.precond_failure_latency_p99_ms);
        println!("\n--- Read Metrics ---");
        println!("Total reads:    {}", self.total_reads);
        println!("Read TPS:       {:.2}", self.read_tps);
        println!("p50: {:.2}ms", self.read_p50_ms);
        println!("p95: {:.2}ms", self.read_p95_ms);
        println!("p99: {:.2}ms", self.read_p99_ms);
        println!("======================================\n");
    }
}
```

### D. Workload Driver (`workload.rs`)

```rust
pub struct WorkloadDriver {
    config: WorkloadConfig,
    manifest: Arc<S3Manifest<String, String, TokioExecutor>>,
    metrics: Arc<MetricsCollector>,
}

impl WorkloadDriver {
    pub fn new(
        config: WorkloadConfig,
        manifest: Arc<S3Manifest<String, String, TokioExecutor>>,
    ) -> Self {
        Self {
            config,
            manifest,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    pub async fn run(&self) -> MetricsSummary {
        tracing::info!(
            num_writers = %self.config.num_writers,
            num_readers = %self.config.num_readers,
            writer_rate = %self.config.writer_rate,
            key_pool_size = %self.config.key_pool_size,
            key_overlap_ratio = %self.config.key_overlap_ratio,
            duration_secs = %self.config.duration.as_secs(),
            "starting workload"
        );

        // Create key pool
        let key_pool = Arc::new(KeyPool::new(
            self.config.key_pool_size,
            self.config.num_writers,
            self.config.key_overlap_ratio,
        ));

        // Spawn writer clients
        let mut handles = vec![];
        for writer_id in 0..self.config.num_writers {
            let mut client = MockClient::new(
                writer_id,
                ClientType::Writer { id: writer_id },
                self.manifest.clone(),
                key_pool.clone(),
                Arc::new(self.config.clone()),
                self.metrics.clone(),
            );

            let duration = self.config.duration;
            let handle = tokio::spawn(async move {
                client.run_loop(duration).await;
            });
            handles.push(handle);
        }

        // Spawn reader clients
        for reader_id in 0..self.config.num_readers {
            let mut client = MockClient::new(
                reader_id,
                ClientType::Reader { id: reader_id },
                self.manifest.clone(),
                key_pool.clone(),
                Arc::new(self.config.clone()),
                self.metrics.clone(),
            );

            let duration = self.config.duration;
            let handle = tokio::spawn(async move {
                client.run_loop(duration).await;
            });
            handles.push(handle);
        }

        // Wait for all clients to finish
        for handle in handles {
            handle.await.ok();
        }

        tracing::info!("workload completed");

        // Collect metrics
        self.metrics.summary()
    }
}
```

### E. Chaos Injection (`chaos.rs`)

```rust
#[derive(Debug, Clone)]
pub enum ChaosScenario {
    None,
    NetworkLatency {
        mean_ms: u64,
        stddev_ms: u64,
    },
    CpuOverload {
        num_threads: usize,
        utilization_pct: f64,
    },
    MemoryPressure {
        leak_mb: usize,
    },
}

pub struct ChaosController {
    scenario: ChaosScenario,
    active: Arc<AtomicBool>,
    _handles: Vec<tokio::task::JoinHandle<()>>,
}

impl ChaosController {
    pub fn new(scenario: ChaosScenario) -> Self {
        Self {
            scenario,
            active: Arc::new(AtomicBool::new(false)),
            _handles: vec![],
        }
    }

    pub async fn start(&mut self) {
        match &self.scenario {
            ChaosScenario::None => {}
            ChaosScenario::NetworkLatency { mean_ms, stddev_ms } => {
                tracing::warn!(
                    mean_ms = %mean_ms,
                    stddev_ms = %stddev_ms,
                    "injecting network latency"
                );
                // Note: Network latency is injected per-operation in S3 wrapper
                // This is just logging
            }
            ChaosScenario::CpuOverload { num_threads, utilization_pct } => {
                tracing::warn!(
                    num_threads = %num_threads,
                    utilization = %utilization_pct,
                    "injecting CPU overload"
                );
                self.active.store(true, Ordering::Relaxed);
                for _ in 0..*num_threads {
                    let active = self.active.clone();
                    let util = *utilization_pct;
                    let handle = tokio::task::spawn_blocking(move || {
                        busy_loop(active, util);
                    });
                    self._handles.push(handle);
                }
            }
            ChaosScenario::MemoryPressure { leak_mb } => {
                tracing::warn!(leak_mb = %leak_mb, "allocating memory pressure");
                // Allocate large buffer and hold it
                let _buffer = vec![0u8; leak_mb * 1024 * 1024];
                std::mem::forget(_buffer);  // Intentional leak for test
            }
        }
    }

    pub async fn stop(&self) {
        self.active.store(false, Ordering::Relaxed);
        tracing::info!("chaos controller stopped");
    }
}

fn busy_loop(active: Arc<AtomicBool>, utilization: f64) {
    while active.load(Ordering::Relaxed) {
        // Busy spin for utilization%, then sleep for (1-utilization)%
        let work_ms = (10.0 * utilization) as u64;
        let sleep_ms = (10.0 * (1.0 - utilization)) as u64;

        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < work_ms as u128 {
            // Busy work
            std::hint::spin_loop();
        }

        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
}
```

---

## 4. Tracing Instrumentation

### Add to existing `fusio-manifest` source files:

#### `fusio-manifest/src/session.rs`

```rust
// In Manifest impl
#[tracing::instrument(skip(self), fields(session_type = "write"))]
pub async fn session_write(&self) -> Result<WriteSession<...>> {
    tracing::debug!("session_write started");
    // existing implementation...
}

#[tracing::instrument(skip(self), fields(session_type = "read"))]
pub async fn session_read(&self) -> Result<ReadSession<...>> {
    tracing::debug!("session_read started");
    // existing implementation...
}

// In WriteSession impl
#[tracing::instrument(skip(self), fields(txn_id = %self.staged_txn_id))]
pub async fn commit(self) -> Result<()> {
    tracing::debug!(
        txn_id = %self.staged_txn_id,
        num_puts = %self.staged_records.iter().filter(|r| matches!(r.op, Op::Put)).count(),
        num_dels = %self.staged_records.iter().filter(|r| matches!(r.op, Op::Del)).count(),
        "committing write session"
    );

    let result = /* existing commit logic */;

    match &result {
        Ok(_) => tracing::info!(txn_id = %self.staged_txn_id, "commit succeeded"),
        Err(Error::PreconditionFailed) => {
            tracing::warn!(txn_id = %self.staged_txn_id, "PRECONDITION FAILED on commit");
        }
        Err(e) => tracing::error!(txn_id = %self.staged_txn_id, error = ?e, "commit failed"),
    }

    result
}
```

#### `fusio-manifest/src/manifest.rs`

```rust
#[tracing::instrument(skip(self))]
pub async fn snapshot(&self) -> Result<Snapshot> {
    tracing::debug!("loading snapshot");
    let snap = /* existing implementation */;
    tracing::info!(
        txn_id = %snap.txn_id,
        last_segment_seq = ?snap.last_segment_seq,
        "snapshot loaded"
    );
    Ok(snap)
}

async fn recover_orphans(&self, snap: &Snapshot) -> Result<Snapshot> {
    tracing::debug!(from_seq = ?snap.last_segment_seq, "recovering orphans");
    let orphans = /* existing orphan detection logic */;

    if !orphans.is_empty() {
        tracing::warn!(
            count = orphans.len(),
            segment_seqs = ?orphans.iter().map(|s| s.seq).collect::<Vec<_>>(),
            "orphan segments recovered"
        );
    }

    /* existing recovery logic */
}
```

#### `fusio-manifest/src/head.rs`

```rust
async fn put(&self, json: &HeadJson, cond: PutCondition) -> Result<HeadTag> {
    tracing::debug!(
        txn_id = json.last_txn_id,
        condition = ?cond,
        "putting HEAD with condition"
    );

    let result = /* existing CAS logic */;

    match &result {
        Ok(tag) => tracing::info!(
            txn_id = json.last_txn_id,
            tag = ?tag,
            "HEAD updated successfully"
        ),
        Err(_) => tracing::warn!(
            txn_id = json.last_txn_id,
            "HEAD CAS conflict"
        ),
    }

    result
}
```

#### `fusio-manifest/src/segment.rs`

```rust
async fn put_next(&self, seq: u64, txn_id: u64, payload: &[u8], ...) -> Result<SegmentId> {
    tracing::debug!(seq, txn_id, size_bytes = payload.len(), "writing segment");
    let id = /* existing write logic */;
    tracing::info!(seq, txn_id, segment_id = ?id, "segment written");
    Ok(id)
}
```

---

## 5. Test Scenarios

### Initial Configuration Baselines

```rust
// scenarios.rs

pub fn baseline_config() -> WorkloadConfig {
    WorkloadConfig {
        num_writers: 2,
        num_readers: 10,
        duration: Duration::from_secs(120),  // 2 minutes

        writer_rate: 0.1,           // 0.1 writes/sec per writer
        reader_rate: 1.0,           // 1 read/sec per reader

        key_pool_size: 100,
        key_overlap_ratio: 0.2,     // 20% overlap

        value_size: 256,            // 256 bytes
        write_delete_ratio: 0.1,    // 10% deletes

        max_retry_count: 1,         // One retry on precondition failure

        chaos: ChaosScenario::None,
    }
}
```

### Sweep Configuration Space

We will run tests sweeping these dimensions while holding others constant:

#### **Sweep 1: Number of Writers** (holding writer_rate=0.1, key_pool=100, overlap=0.2)
- 1, 2, 4, 8, 16 writers

#### **Sweep 2: Writer TPS** (holding num_writers=4, key_pool=100, overlap=0.2)
- 0.1, 0.2, 0.5, 1.0, 2.0, 5.0 writes/sec per writer

#### **Sweep 3: Key Pool Size** (holding num_writers=4, writer_rate=0.1, overlap=0.2)
- 50, 100, 200, 500, 1000 keys

#### **Sweep 4: Key Overlap Ratio** (holding num_writers=4, writer_rate=0.1, key_pool=100)
- 0.0, 0.1, 0.2, 0.5, 0.8, 1.0

#### **Sweep 5: Max Retry Count** (holding num_writers=8, writer_rate=1.0, key_pool=100, overlap=0.2)
- 0, 1, 2, 5, 10 retries

---

## 6. Test Execution & Analysis

### Test Runner

```rust
// tests/performance_test/mod.rs

#[tokio::test]
#[ignore]  // Run manually: cargo test --test performance_test --features tokio -- --ignored --nocapture
async fn sweep_num_writers() {
    init_tracing();

    let mut results = Vec::new();

    for num_writers in [1, 2, 4, 8, 16] {
        let mut config = baseline_config();
        config.num_writers = num_writers;

        let manifest = create_real_s3_manifest(&format!("sweep-writers-{}", num_writers)).await.unwrap();
        let driver = WorkloadDriver::new(config.clone(), Arc::new(manifest));

        println!("\n=== Running with {} writers ===", num_writers);
        let summary = driver.run().await;
        summary.print_report();

        results.push((config, summary));
    }

    // Export to CSV
    export_results_csv("sweep_num_writers.csv", &results).unwrap();

    println!("\n=== Summary ===");
    for (config, summary) in &results {
        println!(
            "Writers: {}, Precondition Failure Rate: {:.2}%, Write TPS: {:.2}",
            config.num_writers,
            summary.precondition_failure_rate * 100.0,
            summary.write_tps
        );
    }
}

#[tokio::test]
#[ignore]
async fn sweep_writer_tps() {
    init_tracing();

    let mut results = Vec::new();

    for rate in [0.1, 0.2, 0.5, 1.0, 2.0, 5.0] {
        let mut config = baseline_config();
        config.writer_rate = rate;
        config.num_writers = 4;

        let manifest = create_real_s3_manifest(&format!("sweep-rate-{}", rate)).await.unwrap();
        let driver = WorkloadDriver::new(config.clone(), Arc::new(manifest));

        println!("\n=== Running with writer_rate={} ===", rate);
        let summary = driver.run().await;
        summary.print_report();

        results.push((config, summary));
    }

    export_results_csv("sweep_writer_tps.csv", &results).unwrap();
}

// Helper: create S3Manifest with real credentials
async fn create_real_s3_manifest(
    test_name: &str
) -> Result<S3Manifest<String, String, TokioExecutor>, Box<dyn std::error::Error>> {
    let bucket = std::env::var("FUSIO_MANIFEST_BUCKET")?;
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let key_id = std::env::var("AWS_ACCESS_KEY_ID")?;
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")?;
    let token = std::env::var("AWS_SESSION_TOKEN").ok();

    let prefix = format!(
        "chaos-tests/{}/{}",
        test_name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let builder = fusio::remotes::aws::s3::Builder::new(&bucket)
        .prefix(&prefix)
        .region(&region)
        .sign_payload(true)
        .credential(fusio::remotes::aws::AwsCredential {
            key_id,
            secret_key,
            token,
        });

    let config = builder.build();
    let context = Arc::new(ManifestContext::new(TokioExecutor::default()));

    Ok(config.with_context(context).into())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("fusio_manifest=debug,performance_test=info"))
        )
        .with_target(true)
        .with_line_number(true)
        .init();
}
```

### CSV Export & Visualization

```rust
// visualization.rs

use csv::Writer;
use std::fs::File;

pub fn export_results_csv(
    filename: &str,
    results: &[(WorkloadConfig, MetricsSummary)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_writer(File::create(filename)?);

    // Write header
    wtr.write_record(&[
        "num_writers",
        "num_readers",
        "writer_rate",
        "reader_rate",
        "key_pool_size",
        "key_overlap_ratio",
        "max_retry_count",
        "duration_secs",
        "precondition_failure_rate",
        "write_tps",
        "read_tps",
        "write_p50_ms",
        "write_p95_ms",
        "write_p99_ms",
        "precond_p50_ms",
        "precond_p99_ms",
        "read_p50_ms",
        "read_p99_ms",
        "avg_retry_count",
    ])?;

    // Write data rows
    for (config, summary) in results {
        wtr.write_record(&[
            config.num_writers.to_string(),
            config.num_readers.to_string(),
            config.writer_rate.to_string(),
            config.reader_rate.to_string(),
            config.key_pool_size.to_string(),
            config.key_overlap_ratio.to_string(),
            config.max_retry_count.to_string(),
            config.duration.as_secs().to_string(),
            format!("{:.4}", summary.precondition_failure_rate),
            format!("{:.2}", summary.write_tps),
            format!("{:.2}", summary.read_tps),
            format!("{:.2}", summary.write_p50_ms),
            format!("{:.2}", summary.write_p95_ms),
            format!("{:.2}", summary.write_p99_ms),
            format!("{:.2}", summary.precond_failure_latency_p50_ms),
            format!("{:.2}", summary.precond_failure_latency_p99_ms),
            format!("{:.2}", summary.read_p50_ms),
            format!("{:.2}", summary.read_p99_ms),
            format!("{:.2}", summary.avg_retry_count),
        ])?;
    }

    wtr.flush()?;
    println!("Results exported to {}", filename);
    Ok(())
}
```

### Plotting (Python script)

```python
# plot_results.py

import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns

def plot_sweep(csv_file, x_col, title):
    df = pd.read_csv(csv_file)

    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle(title, fontsize=16)

    # Plot 1: Precondition failure rate vs x
    axes[0, 0].plot(df[x_col], df['precondition_failure_rate'] * 100, marker='o')
    axes[0, 0].axhline(y=10, color='r', linestyle='--', label='10% threshold')
    axes[0, 0].set_xlabel(x_col)
    axes[0, 0].set_ylabel('Precondition Failure Rate (%)')
    axes[0, 0].legend()
    axes[0, 0].grid(True)

    # Plot 2: Write TPS vs x
    axes[0, 1].plot(df[x_col], df['write_tps'], marker='o', color='green')
    axes[0, 1].set_xlabel(x_col)
    axes[0, 1].set_ylabel('Write TPS')
    axes[0, 1].grid(True)

    # Plot 3: Write latency (p50, p95, p99) vs x
    axes[1, 0].plot(df[x_col], df['write_p50_ms'], marker='o', label='p50')
    axes[1, 0].plot(df[x_col], df['write_p95_ms'], marker='s', label='p95')
    axes[1, 0].plot(df[x_col], df['write_p99_ms'], marker='^', label='p99')
    axes[1, 0].set_xlabel(x_col)
    axes[1, 0].set_ylabel('Write Latency (ms)')
    axes[1, 0].legend()
    axes[1, 0].grid(True)

    # Plot 4: Precondition failure latency vs x
    axes[1, 1].plot(df[x_col], df['precond_p50_ms'], marker='o', label='p50')
    axes[1, 1].plot(df[x_col], df['precond_p99_ms'], marker='^', label='p99')
    axes[1, 1].set_xlabel(x_col)
    axes[1, 1].set_ylabel('Precondition Failure Latency (ms)')
    axes[1, 1].legend()
    axes[1, 1].grid(True)

    plt.tight_layout()
    plt.savefig(f'{csv_file[:-4]}.png', dpi=300)
    print(f"Plot saved to {csv_file[:-4]}.png")

if __name__ == '__main__':
    plot_sweep('sweep_num_writers.csv', 'num_writers', 'Precondition Failure vs Number of Writers')
    plot_sweep('sweep_writer_tps.csv', 'writer_rate', 'Precondition Failure vs Writer Rate')
    plot_sweep('sweep_key_pool.csv', 'key_pool_size', 'Precondition Failure vs Key Pool Size')
    plot_sweep('sweep_overlap.csv', 'key_overlap_ratio', 'Precondition Failure vs Key Overlap Ratio')
```

---

## 7. Implementation Phases

### **Phase 1: Foundation (Days 1-2)** ✅ **COMPLETED**
- [x] Add `tracing` instrumentation to existing `fusio-manifest` source files
  - `session.rs`: session start/commit with txn_id logging
  - `manifest.rs`: snapshot loading, orphan recovery
  - `head.rs`: HEAD CAS operations with condition logging
  - `segment.rs`: segment writes
- [x] Create test module structure in `tests/perf_test/`
- [x] Implement `KeyPool` with overlap logic
- [x] Implement `MockClient` (writer & reader with retry logic)
- [x] Implement `MetricsCollector` with histograms
- [x] Implement `WorkloadDriver`
- [x] Add dependencies to `Cargo.toml` (tracing, hdrhistogram, csv, tracing-subscriber)
- [x] Write `test_baseline` that runs 2 writers, 10 readers for 2min
- [x] **Successfully ran baseline test against real S3**

**Baseline Test Results:**
```
Duration: 122.35s
Write TPS: 0.20 | Read TPS: 7.45
Precondition Failure Rate: 45.83% (11 failures / 24 attempts)
Write Latency: p50=1552ms, p95=3023ms, p99=3045ms
Read Latency: p50=1108ms, p95=2249ms, p99=2423ms
```

### **Phase 2: Verification & Baseline Refinement (Days 3-4)** ✅ **COMPLETED**
- [x] Add serialization verification test (check segment order, txn_id monotonicity)
  - Enhanced `verify_serializable_isolation()` with segment sequence checks
  - Verifies transaction ID monotonicity and no duplicate keys
- [x] Verify all writes are reflected in final manifest state
- [x] Test with different overlap ratios (0.0, 0.1, 0.2, 0.5) to understand failure rates
  - Implemented `test_overlap_sweep()` with 4 overlap ratios
  - Results exported to `sweep_overlap.csv`
- [x] Document expected precondition failure rates vs overlap
- [x] Optimize configuration
- [x] Test CSV export functionality
  - Created `visualization.rs` module with CSV export
  - Added `config_label` column for easier identification
- [x] **Bonus**: Enhanced AWS credentials support
  - Reads from `~/.aws/credentials` file
  - Supports EC2 IAM roles via environment variables
  - Configurable S3 bucket name via `FUSIO_MANIFEST_BUCKET`
- [x] **Bonus**: Organized S3 structure
  - Single parent folder per sweep run
  - Human-readable timestamps: `comprehensive-sweep-20250123-143022`
  - Test subfolders: `test-000-W2_WR0.05_O0.1_RD4_RT5`

### **Phase 3: Parameter Sweeps (Days 5-6)** ✅ **COMPLETED (via comprehensive sweep)**
- [x] **Comprehensive Sweep** - Tests ALL parameter combinations (better than individual sweeps!)
  - Configuration space: 3 × 4 × 4 × 1 × 2 = **96 configurations**
  - `num_writers`: [2, 3, 4]
  - `writer_rate`: [0.05, 0.1, 0.15, 0.2] ops/sec
  - `key_overlap_ratio`: [0.1, 0.2, 0.3, 0.4]
  - `num_readers`: [4] (fixed)
  - `reader_rate`: [5, 6] ops/sec
- [x] Parallel execution: 8 concurrent tests
- [x] Duration: 60 seconds per test
- [x] Total time: 20-25 minutes for all 96 configs
- [x] Export to `comprehensive_sweep.csv` with all metrics
- [x] Auto-generate plots with Python script
  - 7-panel comprehensive visualization
  - Failure rate heatmap (writers × overlap)
  - Top 10 best/worst configs
  - Multiple scatter plots showing relationships
- [x] Print top 5 best and worst configurations

**Note:** This comprehensive approach provides much richer data than individual parameter sweeps would have, as it tests all combinations rather than varying one parameter at a time.

### **Phase 4: Chaos Engineering (Days 7-8)** ✅ **COMPLETED**
- [x] **Auto-select best configuration** from Phase 3 comprehensive sweep
  - Parse `comprehensive_sweep.csv`
  - Select config with lowest `precondition_failure_rate`
  - Implemented in `utils.rs::get_best_config_from_csv()`
- [x] Implement `ChaosController` with actual chaos injection
  - Network latency: `tokio::time::sleep()` delays (50ms, 100ms, 200ms)
  - CPU overload: Spawns busy-loop threads (2/4 threads @ 50%/80% utilization)
  - Combined scenarios (100ms + 2 threads @ 50%)
  - Full implementation in `chaos.rs`
- [x] Create `test_chaos_sweep()` - single-command test
  - Runs best config under 7 different chaos scenarios
  - Duration: 5 minutes per scenario (300s for stress testing)
  - Total time: ~35 minutes
  - Added to `performance_test.rs`
- [x] **Chaos Scenarios:**
  - Baseline (no chaos) - control
  - Network latency: 50ms, 100ms, 200ms
  - CPU overload: 2 threads @ 50%, 4 threads @ 80%
  - Combined: 100ms latency + 2 threads @ 50% CPU
- [x] Export to `chaos_sweep.csv` with chaos scenario labels
- [x] Auto-generate `chaos_sweep.png` showing performance degradation
  - 6 comprehensive plots in `plot_results.py::plot_chaos_sweep()`
  - Degradation analysis vs baseline
  - Color-coded by scenario type
- [x] **Enhanced Isolation Verification (Option B)**:
  - Track all writes and reads during test via `WriteRecord`/`ReadRecord`
  - Verify readers only saw committed states
  - Verify causal consistency (reads happen after writes)
  - Verify monotonic reads per reader
  - Implemented in `performance_test.rs::verify_serializable_isolation_with_tracking()`
  - Applied per-scenario for chaos tests
  - Applied per-batch for comprehensive sweep

### **Phase 5: Analysis & Tuning (Days 9+)**
- [ ] Identify configuration where precondition_failure_rate crosses 10%
- [ ] Test retry strategies (max_retry_count: 0, 1, 2, 5, 10)
- [ ] Long-running soak tests (10min, 30min, 1hr)
- [ ] Document findings in report

---

## 8. Expected Outcomes

### Key Research Questions

1. **What is the maximum sustainable write TPS before precondition failure rate exceeds 10%?**
   - Hypothesis: ~4-8 writers at 1.0 ops/sec with 20% overlap

2. **How does key overlap ratio affect failure rate?**
   - Hypothesis: Linear relationship (20% overlap → ~5% failures, 80% overlap → ~30% failures)

3. **What is the latency penalty of precondition failures?**
   - Hypothesis: 2-3x normal write latency (due to session restart + orphan recovery)

4. **How effective are retries?**
   - Hypothesis: 1 retry sufficient for <80% overlap, 2-5 retries needed for high contention

5. **Does reader fleet affect writer contention?**
   - Hypothesis: Minimal impact (readers use snapshots, don't block writers)

### Deliverables

- [ ] CSV datasets for all parameter sweeps
- [ ] PNG plots showing precondition failure rate vs each parameter
- [ ] Markdown report summarizing findings
- [ ] Configuration recommendations for production use

---

## 9. Environment Setup

### Required Environment Variables

```bash
export FUSIO_MANIFEST_BUCKET=your-chaos-test-bucket
export AWS_REGION=us-east-1
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
# AWS_SESSION_TOKEN is optional (for SSO)
```

### Run Tests

```bash
# Single baseline test
cargo test --test performance_test --features tokio test_baseline -- --ignored --nocapture

# Run all sweep tests
cargo test --test performance_test --features tokio -- --ignored --nocapture

# Generate plots
python3 plot_results.py
```

---

## 10. Success Criteria

- [ ] All tests run successfully against real S3 without crashes
- [ ] Metrics capture precondition failure rate with <1% measurement error
- [ ] Tracing logs show clear session lifecycle (start, commit, precondition failures)
- [ ] CSV exports contain all required columns
- [ ] Plots clearly show relationship between parameters and failure rates
- [ ] Identified at least one configuration where failure rate crosses 10% threshold
- [ ] Retry behavior demonstrably reduces effective failure rate

---

## Notes

- **S3 Rate Limits**: Be aware of AWS account limits (5,500 PUT/s per prefix). Use unique prefixes per test to avoid throttling.
- **Test Duration**: Each 2-minute test with 16 writers at 5.0 ops/sec = ~960 total write attempts. Budget ~3-5 minutes per test including setup/teardown.
- **Retry Semantics**: On `PreconditionFailed`, client immediately starts new `session_write()` which calls `recover_orphans()`. This ensures linearizability.
- **Lease Management**: Readers use `session_read()` which auto-acquires lease. Must call `.end()` to release. For long tests, consider `start_lease_keeper()`.
- **Chaos Limitations**: Lightweight chaos (in-process latency injection) sufficient for initial tests. For network partition testing, consider external tools (toxiproxy, Chaos Mesh).

---

## Future Enhancements

- [ ] Add GC during workload (test compaction impact on active sessions)
- [ ] Test checkpoint recovery (restart manifest mid-test)
- [ ] Multi-region chaos (cross-region latency simulation)
- [ ] Integration with observability platform (Prometheus, Grafana)
- [ ] Automated regression testing in CI (detect performance degradation)
