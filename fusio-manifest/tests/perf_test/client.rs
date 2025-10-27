use crate::perf_test::{metrics::MetricsCollector, utils::{KeyPool, KeyRegistry, WorkloadConfig}};
use fusio::executor::tokio::TokioExecutor;
use fusio_manifest::{s3::S3Manifest, types::Error};
use rand::{seq::SliceRandom, Rng};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub enum ClientType {
    MonotonicWriter { id: usize },
    Writer { id: usize },
    Reader { id: usize },
}

pub struct MockClient {
    id: usize,
    client_type: ClientType,
    manifest: Arc<S3Manifest<String, String, TokioExecutor>>,
    key_pool: Option<Arc<KeyPool>>,
    key_registry: Option<Arc<KeyRegistry>>,
    config: Arc<WorkloadConfig>,
    metrics: Arc<MetricsCollector>,
}

impl MockClient {
    pub fn new(
        id: usize,
        client_type: ClientType,
        manifest: Arc<S3Manifest<String, String, TokioExecutor>>,
        key_pool: Option<Arc<KeyPool>>,
        key_registry: Option<Arc<KeyRegistry>>,
        config: Arc<WorkloadConfig>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            id,
            client_type,
            manifest,
            key_pool,
            key_registry,
            config,
            metrics,
        }
    }

    #[tracing::instrument(skip(self), fields(writer_id = %self.id))]
    async fn run_write_transaction(&mut self) -> Result<(), Error> {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let key_pool = self.key_pool.as_ref().expect("KeyPool required for legacy Writer");
        let my_keys = key_pool.writer_keys(self.id);
        let mut rng = StdRng::from_entropy();
        let key = my_keys.choose(&mut rng).unwrap();

        let mut attempt = 0;
        loop {
            let start = Instant::now();

            tracing::debug!(writer_id = %self.id, attempt, key, "starting write session");

            let mut session = self.manifest.session_write().await?;

            let is_delete = rng.gen::<f64>() < self.config.write_delete_ratio;
            let value = if is_delete {
                session.delete(key.clone());
                String::new()
            } else {
                let val = generate_value(self.config.value_size);
                session.put(key.clone(), val.clone());
                val
            };

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

                    if !is_delete {
                        self.metrics.record_successful_write(self.id, key.clone(), value);
                    }

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
                        self.metrics.record_max_retries_exceeded();
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

    #[tracing::instrument(skip(self), fields(monotonic_writer_id = %self.id))]
    async fn run_monotonic_write_transaction(&mut self) -> Result<(), Error> {
        let key_registry = self.key_registry.as_ref()
            .expect("KeyRegistry required for MonotonicWriter");

        let key = key_registry.allocate_next_key();

        let mut attempt = 0;
        loop {
            let start = Instant::now();

            tracing::debug!(writer_id = %self.id, attempt, key, "starting monotonic write session");

            let mut session = self.manifest.session_write().await?;
            let value = generate_value(self.config.value_size);
            session.put(key.clone(), value.clone());

            let result = session.commit().await;
            let latency = start.elapsed();

            match result {
                Ok(_) => {
                    tracing::info!(
                        writer_id = %self.id,
                        attempt,
                        key,
                        latency_ms = latency.as_millis(),
                        "monotonic write committed successfully"
                    );
                    self.metrics.record_write_success(latency, attempt);
                    self.metrics.record_successful_write(self.id, key.clone(), value.clone());
                    key_registry.register_written_key(key);
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
                        self.metrics.record_max_retries_exceeded();
                        return Err(Error::PreconditionFailed);
                    }
                    attempt += 1;
                    continue;
                }
                Err(e) => {
                    tracing::error!(writer_id = %self.id, error = ?e, "monotonic write failed");
                    self.metrics.record_write_error(latency);
                    return Err(e);
                }
            }
        }
    }

    #[tracing::instrument(skip(self), fields(reader_id = %self.id))]
    async fn run_read_transaction(&mut self) -> Result<(), Error> {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::from_entropy();

        let all_keys = if let Some(ref key_pool) = self.key_pool {
            key_pool.reader_keys().to_vec()
        } else if let Some(ref key_registry) = self.key_registry {
            key_registry.all_keys()
        } else {
            tracing::error!("No key source available for reader");
            return Ok(());
        };

        if all_keys.is_empty() {
            tracing::debug!(reader_id = %self.id, "no keys available to read, skipping");
            return Ok(());
        }

        let key = all_keys.choose(&mut rng).unwrap();

        let start = Instant::now();
        tracing::debug!(reader_id = %self.id, key, "starting read session");

        let session = self.manifest.session_read().await?;
        let snapshot_txn_id = session.snapshot().txn_id.0;
        let value = session.get(key).await?;

        self.metrics.record_read_observation(
            self.id,
            snapshot_txn_id,
            key.clone(),
            value,
        );

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

    pub async fn run_loop(&mut self, duration: Duration) {
        let rate = match self.client_type {
            ClientType::MonotonicWriter { .. } => self.config.writer_rate,
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
                ClientType::MonotonicWriter { .. } => self.run_monotonic_write_transaction().await,
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
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    StdRng::from_entropy()
        .sample_iter(&Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}
