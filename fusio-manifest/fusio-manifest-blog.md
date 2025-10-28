# Building a Serializable Manifest on S3: What We Learned from WAL3

**TL;DR**: We built `fusio-manifest`, an S3-native metadata system with serializable isolation for LSM-tree databases. Inspired by Chroma's WAL3 but targeting a different problem: low-write, high-consistency manifest operations rather than high-throughput WAL. The last piece toward true serverless databases is falling into place.

---

## Summary

S3 changed the game twice. First in 2020 with strong consistency—no more eventual consistency headaches. Then in 2024 with conditional writes—atomic compare-and-swap on object storage. That second change unlocked something fundamental: **optimistic concurrency control in the cloud**.

Chroma's [WAL3](https://www.trychroma.com/engineering/wal3) proved you could build write-ahead logs directly on S3. That was revelatory. But we're chasing a different dragon: not general-purpose WAL (that's still latency-sensitive and throughput-hungry), but the **LSM-tree manifest**—the metadata system that tracks which files exist, at what levels, with what key ranges. This thing may has low writes throughput, but it ***cannot*** get consistency wrong. **EVER**.

We built `fusio-manifest` for [Tonbo](https://github.com/tonbo-io/tonbo), our realtime serverless analytics engine. It provides serializable isolation on S3 while readers and writers never block each other through snapshot. Through simulation/chaos testing, we validated the design and found the sweet spots: <2 writers, hundreds of readers, ~10% condition failure with effective retries.

This post covers what we learned from WAL3, why manifest is the *right* use case for S3-based logs, the architecture, and the performance data that proves it works.

---

## Why S3? Why Now?

### The Traditional Story

Traditionally, write-ahead logs live on **local disk**. Fast, high throughput, low latency at sub-millisecond latency. You flush to disk, fsync, and return to client "your write is safe". That's the gold standard.

But local disk means:
- Managing persistent volumes (Kubernetes StatefulSets, anyone?)
- Replication complexity (Raft, chain replication, quorum writes)
- Operational overhead (disk failures, capacity planning, backup/restore)
- Cloud vendor lock-in (EBS is not the same as Azure Disk)

Object storage promises the opposite:
- **Durability**: 11 nines by default, multi-AZ replication built-in
- **Scalability**: Effectively infinite, pay-per-use
- **Simplicity**: HTTP API, no persistent state to manage
- **Cost**: ~$0.023/GB/month vs ~$0.10+/GB/month for block storage
- **True serverless**: No infrastructure, just API calls

The problem? Until 2024, you couldn't build *correct* transactional systems on S3 without external coordination (DynamoDB, etcd, etc.). Conditional writes changed that.

### What WAL3 Taught Us

Chroma's WAL3 demonstrated four critical insights:

**1. Append-only logs on S3 actually work**

The benefits are real:
- No persistent volumes to manage
- Multi-AZ durability by default
- Pay only for what you store
- Trivial disaster recovery (it's just objects)
- Backend portability (AWS, MinIO, LocalStack—same code)

**2. But latency is still a question mark**

Latency numbers are honest: p99 > 200ms. For a real-time database serving user queries, that's... not great. You can't return a user "your write succeeded" after 200ms+ every time. That kills interactivity.

Latency-sensitive WAL remains disk's domain. For now.

**3. Throughput can scale to acceptable levels**

Through batching and clever optimizations like batch prefixes, throughput can be improved. WAL3 achieves respectable throughput. Not local-disk levels, but enough for many workloads. The architecture scales horizontally—each collection gets its own log.

**4. MVCC is essential: reads and writes must not block each other**

Snapshot isolation lets readers work off consistent snapshots while writers commit new transactions. This is fundamental for databases where queries can't stall because someone's writing.

### The Manifest Opportunity

Here's where we diverge from WAL3. We're not trying to build a general-purpose WAL. We're targeting something specific: **LSM-tree manifests**.

If you're familiar with RocksDB, you know the [MANIFEST file](https://github.com/facebook/rocksdb/wiki/MANIFEST)—it tracks which SSTable files exist, at what levels, with what key ranges and sequence numbers. It's the metadata layer. Changes happen when:
- Writers flush memtables → new L0 files
- Compactors merge files → new files at L1+, old files deleted
- GC cleans up → files removed

This is naturally **low write throughput** (0.1-10 TPS) and **not latency-sensitive** (compaction is background work). But it requires **serializable isolation**—you cannot have two compactors simultaneously deleting the same file, or readers observing inconsistent file sets.

And it needs **high read throughput**—query engines constantly check which files to scan.

That's the sweet spot for S3-based logs: low write rate, high consistency requirements, high read concurrency, operational simplicity.

**We built `fusio-manifest for` this.** (Don't confuse it with WAL3's manifest! different problem, different layer.)

---

## Architecture: Five Components, One CAS Barrier

Everything coordinates through a single **HEAD** object. CAS that, you win. Fail, you retry. Simple.

```
        Writer                    Reader
          │                         │
          │ 1. snapshot HEAD        │ 1. snapshot HEAD
          │ 2. stage ops            │ 2. acquire lease
          │ 3. write segment        │ 3. get/scan (segments + checkpoint)
          │ 4. CAS HEAD             │ 4. release lease
          │    ├─ success ✓         │
          │    └─ conflict ✗ → retry
          │
          ▼
    ┌──────────────┐
    │     HEAD     │ ← Single source of truth
    │ {txn, seq}   │    {last_txn_id, last_segment_seq, checkpoint_id}
    └──────┬───────┘
           │
           ├─────────────┬─────────────┬──────────────┐
           ▼             ▼             ▼              ▼
      segments/      checkpoints/   leases/        gc/
      seg-1.json     ckpt-42.json   lease-*.json   GARBAGE
      seg-2.json     ckpt-42.meta
      ...
```

**HEAD**: 200-byte JSON. Contains the current `txn_id`, `segment_seq`, and optional `checkpoint_id`. Every write CAS-updates this. That's the serialization point.

**Segments**: Append-only, immutable. Each segment = batch of `{key, op, value}` records tagged with a monotonic `txn_id`. Writers write segment, then CAS HEAD. If CAS fails, segment becomes "orphan" (recoverable later).

**Checkpoints**: Periodic compaction snapshots. Fold all segments up to `txn_id` into a single key-value dump. Readers scan segments *newer* than the checkpoint, then consult checkpoint on misses. Bounds recovery time.

**Leases**: Short-lived JSON objects tracking active readers/writers. Each session pins a `snapshot_txn_id`. GC uses minimum lease watermark to know what's safe to delete.

**GC Plan**: CAS-protected coordination for multi-process GC. Three phases: compute (figure out what to delete), apply (ensure HEAD references sufficient checkpoint), delete+reset (actually remove objects).

### How Serializable Isolation Works

1. **Writer flow**: Snapshot HEAD → stage puts/deletes → write segment to S3 → CAS HEAD with new `{txn_id, seq}`. If CAS fails (someone else committed first), retry from snapshot.

2. **Reader flow**: Snapshot HEAD → acquire lease → read segments + checkpoint within snapshot bounds → release lease. Never see writes after your snapshot.

3. **No lost updates**: CAS ensures exactly one writer wins per `txn_id`. Monotonic `txn_id` prevents write skew.

4. **No phantom reads**: Snapshot isolation means your view is frozen. New keys written after your snapshot? You don't see them.

Same MVCC guarantees as PostgreSQL. On S3. With zero distributed consensus.

### Portability: Test Local, Deploy Cloud

The trait-based design (`HeadStore`, `SegmentIo`, `CheckpointStore`, `LeaseStore`) means the same code runs on:
- **In-memory** (tests): Synchronous, deterministic, fast
- **S3** (production): AWS, MinIO, LocalStack
- **Any async runtime**: Tokio, async-std, Monoio, blocking executors

Write unit tests with in-memory stores. Deploy to S3. No code changes.

---

## Proving Correctness: Simulation & Chaos Testing

We ran extensive simulation tests against real S3 (LocalStack) to prove serializable isolation holds under various scenarios:

**Client scenarios tested**:
- Concurrent writers (1-4) competing for HEAD CAS
- Concurrent readers (1-4) with overlapping snapshots
- Varying write rates (0.1-1.0 TPS) and read rates (1-100 ops/sec)
- Key overlap patterns (5-50% shared keys between writers)
- Retry policies (0-2 max retries)

**Chaos scenarios simulated**:
- Network delays and timeouts
- CPU throttling
- Combined failure modes (network + CPU)
- CAS conflicts under contention
- Orphan segment recovery

![Comprehensive Performance Sweep](https://liguoso-fusio-test.s3.amazonaws.com/comprehensive_sweep_v2.png)

### Core Findings

**Baseline performance** (2 writers @ 0.1 TPS): ~10% precondition failure rate with effective retries recovering 90%+ of conflicts. This establishes the expected behavior for typical LSM manifest workloads.

**Multi-writer contention**: As writer count increases, CAS conflicts scale linearly. With overlapping key spaces (15-25% shared keys), conflict rates rise but remain manageable with simple retry logic. Natural partitioning in LSM manifests (different compaction levels, non-overlapping key ranges) keeps this low in practice.

**Chaos resilience**: Under simulated network delays, CPU throttling, and combined failures, the system maintained serializable isolation. No consistency violations observed—no lost updates, no phantom reads, no write skew anomalies.

**Correctness validation**: Beyond performance metrics, we validated fundamental properties through:
- Deterministic tests (in-memory backends with controlled execution)
- Property-based testing (thousands of random concurrent workloads)
- Explicit checks: monotonic transaction IDs, snapshot isolation, atomic visibility

**Result**: Zero consistency violations across all test runs. Serializable isolation provably works on S3.

### Practical Guidance

Although we tested multi-writer scenarios to validate correctness under contention, **the best use case remains single-writer, multiple-reader** for your database backend. LSM-tree manifests naturally have one writer (the compaction coordinator or flush manager) with many readers (query engines checking which files to scan). This pattern keeps conflicts minimal and should not be a bottleneck for your merge-tree metadata management.

---

## The Mission: True Serverless == Diskless

WAL3 cleared a critical path. They proved you can put append-only logs on object storage with acceptable performance and strong consistency. That's huge.

But the journey to **true serverless databases** requires more than just WAL. You need:
1. **WAL** (latency-sensitive, high throughput) ← WAL3 addressed this
2. **Manifest** (consistency-critical, low throughput) ← fusio-manifest addresses this
3. **Storage layer** (data files on S3)

We're building piece by piece. fusio-manifest targets the metadata layer—arguably the *most critical* piece for correctness. One consistency bug in manifest = entire database corrupted. We can't afford eventual consistency or race conditions here.

We're bullish on serverless. We're bullish on object storage. The agility and simplicity gains are too large to ignore. Managed services are great, but **true serverless**—where you write code, deploy it, and S3 handles the rest—that's the endgame.

fusio-manifest is one piece. We're building the rest.

## What's Next

**Production-level testing**: While our simulation tests prove correctness, we need long-running tests in real production environments to gather metrics on:
- Multi-day workloads with realistic traffic patterns
- Real AWS S3 under various failure modes
- Production-level observability: latency distributions, retry storms, GC efficiency
- Cost analysis: S3 request costs, data transfer costs at scale

**Missing features**:
- **Binary segment format with CRC**: Current JSON segments need framing v2 with magic bytes, version, length, and checksums for faster validation and torn-tail handling
- **Setsum verification**: Following WAL3's lead, implement associative checksums to continuously prove `digest(live) + digest(GC'd) = digest(all writes)`
- **Lazy checkpoint loading**: Current implementation downloads entire checkpoint payloads; need streaming/incremental loading for manifests with millions of keys
- **Observability gaps**: Comprehensive tracing spans, metrics exports, and dashboard templates for production monitoring
- **Rate limiting**: Per-process semaphores to prevent S3 request storms during CAS-heavy operations (lease renewals, GC coordination)

These aren't blockers—fusio-manifest works today for LSM manifests. But production hardening requires the above.

## References

1. [WAL3: A Write-Ahead Log for Chroma](https://www.trychroma.com/engineering/wal3) - Chroma Engineering Blog
2. [RocksDB MANIFEST](https://github.com/facebook/rocksdb/wiki/MANIFEST) - Metadata layer design patterns
3. [Amazon S3 Conditional Writes](https://aws.amazon.com/about-aws/whats-new/2024/08/amazon-s3-conditional-writes/) - AWS announcement (August 2024)
4. [fusio-manifest RFD](../docs/fusio-manifest-rfd.md) - Full design document
5. [Tonbo](https://github.com/tonbo-io/tonbo) - Rust LSM database using fusio-manifest
6. [Fusio](https://github.com/tonbo-io/fusio) - Unified I/O abstraction layer

---

*fusio-manifest is open source (Apache 2.0) as part of the Fusio project. Thanks to the Chroma team for pioneering S3-native logs and sharing their insights.*
