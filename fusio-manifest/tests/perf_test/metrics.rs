use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use hdrhistogram::Histogram;

#[derive(Debug, Clone)]
pub struct WriteRecord {
    pub writer_id: usize,
    pub key: String,
    pub value: String,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct ReadRecord {
    pub reader_id: usize,
    pub snapshot_txn_id: u64,
    pub key: String,
    pub value: Option<String>,
    pub timestamp: Instant,
}

pub struct MetricsCollector {
    write_success_latency: Arc<Mutex<Histogram<u64>>>,
    precondition_failure_latency: Arc<Mutex<Histogram<u64>>>,
    read_latency: Arc<Mutex<Histogram<u64>>>,

    total_writes_attempted: AtomicU64,
    total_writes_succeeded: AtomicU64,
    total_precondition_failures: AtomicU64,
    total_write_errors: AtomicU64,
    total_reads: AtomicU64,

    retry_counts: Arc<Mutex<Vec<usize>>>,
    start_time: Instant,

    successful_writes: Arc<Mutex<Vec<WriteRecord>>>,
    reader_observations: Arc<Mutex<Vec<ReadRecord>>>,

    total_retry_failures: AtomicU64,
    total_max_retries_exceeded: AtomicU64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            write_success_latency: Arc::new(Mutex::new(Histogram::<u64>::new(3).unwrap())),
            precondition_failure_latency: Arc::new(Mutex::new(Histogram::<u64>::new(3).unwrap())),
            read_latency: Arc::new(Mutex::new(Histogram::<u64>::new(3).unwrap())),
            total_writes_attempted: AtomicU64::new(0),
            total_writes_succeeded: AtomicU64::new(0),
            total_precondition_failures: AtomicU64::new(0),
            total_write_errors: AtomicU64::new(0),
            total_reads: AtomicU64::new(0),
            retry_counts: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
            successful_writes: Arc::new(Mutex::new(Vec::new())),
            reader_observations: Arc::new(Mutex::new(Vec::new())),
            total_retry_failures: AtomicU64::new(0),
            total_max_retries_exceeded: AtomicU64::new(0),
        }
    }

    pub fn record_successful_write(&self, writer_id: usize, key: String, value: String) {
        self.successful_writes.lock().unwrap().push(WriteRecord {
            writer_id,
            key,
            value,
            timestamp: Instant::now(),
        });
    }

    pub fn record_read_observation(
        &self,
        reader_id: usize,
        snapshot_txn_id: u64,
        key: String,
        value: Option<String>,
    ) {
        self.reader_observations.lock().unwrap().push(ReadRecord {
            reader_id,
            snapshot_txn_id,
            key,
            value,
            timestamp: Instant::now(),
        });
    }

    pub fn get_write_records(&self) -> Vec<WriteRecord> {
        self.successful_writes.lock().unwrap().clone()
    }

    pub fn get_read_records(&self) -> Vec<ReadRecord> {
        self.reader_observations.lock().unwrap().clone()
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
        self.total_precondition_failures
            .fetch_add(1, Ordering::Relaxed);
        self.precondition_failure_latency
            .lock()
            .unwrap()
            .record(latency.as_micros() as u64)
            .ok();

        if retry_count > 0 {
            self.total_retry_failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_max_retries_exceeded(&self) {
        self.total_max_retries_exceeded
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_write_error(&self, _latency: Duration) {
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
        let total_retry_failures = self.total_retry_failures.load(Ordering::Relaxed);
        let total_max_retries_exceeded = self.total_max_retries_exceeded.load(Ordering::Relaxed);

        let write_hist = self.write_success_latency.lock().unwrap();
        let precond_hist = self.precondition_failure_latency.lock().unwrap();
        let read_hist = self.read_latency.lock().unwrap();

        let retry_counts = self.retry_counts.lock().unwrap();
        let avg_retries = if !retry_counts.is_empty() {
            retry_counts.iter().sum::<usize>() as f64 / retry_counts.len() as f64
        } else {
            0.0
        };

        let initial_failures = total_precond - total_retry_failures;
        let retry_successes = if initial_failures > total_max_retries_exceeded {
            initial_failures - total_max_retries_exceeded
        } else {
            0
        };

        let retry_success_rate = if initial_failures > 0 {
            retry_successes as f64 / initial_failures as f64
        } else {
            0.0
        };

        let retry_failure_rate = if total_precond > 0 {
            total_retry_failures as f64 / total_precond as f64
        } else {
            0.0
        };

        MetricsSummary {
            duration: elapsed,
            total_write_attempts: total_attempts,
            total_write_success: total_success,
            total_precondition_failures: total_precond,
            total_write_errors: total_errors,
            precondition_failure_rate: if total_attempts > 0 {
                total_precond as f64 / total_attempts as f64
            } else {
                0.0
            },
            write_tps: total_success as f64 / elapsed.as_secs_f64(),
            read_tps: total_reads as f64 / elapsed.as_secs_f64(),
            write_p50_ms: write_hist.value_at_quantile(0.5) as f64 / 1000.0,
            write_p95_ms: write_hist.value_at_quantile(0.95) as f64 / 1000.0,
            write_p99_ms: write_hist.value_at_quantile(0.99) as f64 / 1000.0,
            precond_failure_latency_p50_ms: precond_hist.value_at_quantile(0.5) as f64 / 1000.0,
            precond_failure_latency_p99_ms: precond_hist.value_at_quantile(0.99) as f64 / 1000.0,
            read_p50_ms: read_hist.value_at_quantile(0.5) as f64 / 1000.0,
            read_p95_ms: read_hist.value_at_quantile(0.95) as f64 / 1000.0,
            read_p99_ms: read_hist.value_at_quantile(0.99) as f64 / 1000.0,
            avg_retry_count: avg_retries,
            total_reads,
            total_retry_failures,
            total_max_retries_exceeded,
            retry_failure_rate,
            retry_success_rate,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSummary {
    #[serde(serialize_with = "serialize_duration")]
    pub duration: Duration,
    pub total_write_attempts: u64,
    pub total_write_success: u64,
    pub total_precondition_failures: u64,
    pub total_write_errors: u64,
    pub precondition_failure_rate: f64,
    pub write_tps: f64,
    pub read_tps: f64,
    pub write_p50_ms: f64,
    pub write_p95_ms: f64,
    pub write_p99_ms: f64,
    pub precond_failure_latency_p50_ms: f64,
    pub precond_failure_latency_p99_ms: f64,
    pub read_p50_ms: f64,
    pub read_p95_ms: f64,
    pub read_p99_ms: f64,
    pub avg_retry_count: f64,
    pub total_reads: u64,
    pub total_retry_failures: u64,
    pub total_max_retries_exceeded: u64,
    pub retry_failure_rate: f64,
    pub retry_success_rate: f64,
}

fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_f64(duration.as_secs_f64())
}

impl MetricsSummary {
    pub fn print_report(&self) {
        println!("\n========== Workload Results ==========");
        println!("Duration: {:.2}s", self.duration.as_secs_f64());
        println!("\n--- Write Metrics ---");
        println!("Total attempts:        {}", self.total_write_attempts);
        println!("Successful commits:    {}", self.total_write_success);
        println!(
            "Precondition failures: {}",
            self.total_precondition_failures
        );
        println!("Other errors:          {}", self.total_write_errors);
        println!(
            "\n*** Precondition Failure Rate: {:.2}% ***",
            self.precondition_failure_rate * 100.0
        );
        println!("\nWrite TPS:             {:.2}", self.write_tps);
        println!("Avg retries per op:    {:.2}", self.avg_retry_count);
        println!("\n--- Retry Effectiveness ---");
        println!("Retry failures:        {}", self.total_retry_failures);
        println!("Max retries exceeded:  {}", self.total_max_retries_exceeded);
        println!(
            "Retry success rate:    {:.2}%",
            self.retry_success_rate * 100.0
        );
        println!(
            "Retry failure rate:    {:.2}%",
            self.retry_failure_rate * 100.0
        );
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
