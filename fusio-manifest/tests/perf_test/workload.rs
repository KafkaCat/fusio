use std::sync::Arc;

use fusio::executor::tokio::TokioExecutor;
use fusio_manifest::s3::S3Manifest;

use crate::perf_test::{
    client::{ClientType, MockClient},
    metrics::{MetricsCollector, MetricsSummary},
    utils::{KeyPool, WorkloadConfig},
};

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

    pub fn metrics(&self) -> &Arc<MetricsCollector> {
        &self.metrics
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

        let key_pool = Arc::new(KeyPool::new(
            self.config.key_pool_size,
            self.config.num_writers,
            self.config.key_overlap_ratio,
        ));

        let mut handles = vec![];

        for writer_id in 0..self.config.num_writers {
            let mut client = MockClient::new(
                writer_id,
                ClientType::Writer { id: writer_id },
                self.manifest.clone(),
                Some(key_pool.clone()),
                None,
                Arc::new(self.config.clone()),
                self.metrics.clone(),
            );

            let duration = self.config.duration;
            let handle = tokio::spawn(async move {
                client.run_loop(duration).await;
            });
            handles.push(handle);
        }

        for reader_id in 0..self.config.num_readers {
            let mut client = MockClient::new(
                reader_id,
                ClientType::Reader { id: reader_id },
                self.manifest.clone(),
                Some(key_pool.clone()),
                None,
                Arc::new(self.config.clone()),
                self.metrics.clone(),
            );

            let duration = self.config.duration;
            let handle = tokio::spawn(async move {
                client.run_loop(duration).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.ok();
        }

        tracing::info!("workload completed");

        self.metrics.summary()
    }
}
