---
title: 11-ErrorHandling
type: reference
summary: Обработка ошибок и восстановление — категории сбоев, retry, circuit breaker, по доменам
tags: [ipfrs, errors, resilience, recovery]
related: ["[[04-StorageDomain]]", "[[05-NetworkDomain]]", "[[10-Performance]]"]
read_time: 35 мин
updated: 2026-06-18
---

# Error Handling & Recovery: Надёжность в действии

**Краткое резюме**: Распределённая система сложнее обычного приложения. Здесь описаны типичные сбои и как IPFRS их обрабатывает.

---

## Категории ошибок

```
┌─────────────────────────────────────┐
│ External (не наша ошибка)           │
├─────────────────────────────────────┤
│ Network partition (peer offline)    │
│ Storage full (SSD no space)         │
│ Peer malicious (returns bad block)  │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Internal (наша ошибка)              │
├─────────────────────────────────────┤
│ Corruption (hash mismatch)          │
│ Deadlock (circular wait)            │
│ Buffer overflow (too much data)     │
│ Logic error (infinite loop)         │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Transient (временная)               │
├─────────────────────────────────────┤
│ Timeout (network slow)              │
│ Cache miss (retry)                  │
│ Peer busy (try another)             │
└─────────────────────────────────────┘
```

---

## Per-Domain Error Handling

### 1. Storage Domain Errors

**Error**: Block corruption detected

```rust
pub enum StorageError {
    CorruptionDetected {
        cid: Cid,
        expected_hash: Hash,
        actual_hash: Hash,
    },
    OutOfSpace,
    DatabaseLocked,
    SerializationError,
}

// Handler
match storage.get(&cid).await {
    Ok(block) => {
        // Verify invariant
        if hash(&block.data) != cid {
            // ALERT: Corruption!
            emit_alert(Alert::CorruptionDetected { cid });
            
            // Try recovery
            if let Ok(true) = self.repair_from_replicas(&cid).await {
                metrics.corruption_repaired += 1;
            } else {
                metrics.corruption_unrecoverable += 1;
                // Remove from index, mark as lost
                self.mark_unavailable(&cid)?;
            }
        }
    }
    Err(StorageError::OutOfSpace) => {
        // Trigger garbage collection
        self.trigger_gc()?;
        // Retry after GC
        storage.get(&cid).await?
    }
    Err(e) => {
        // Log and propagate
        error!("Storage error: {:?}", e);
        return Err(e.into());
    }
}
```

**Prevention**:
- Always verify: `hash(retrieved) == cid`
- Redundancy: Store important blocks on multiple peers
- Checksums: Double-check before sending to user

---

### 2. Network Domain Errors

**Error**: Peer offline, DHT lookup fails

```rust
pub enum NetworkError {
    PeerOffline(PeerId),
    DHTPropagationDelay,
    InvalidPeerId,
    DialFailure { peer_id: PeerId, reason: String },
}

// Handler
match network.find_providers(&cid).await {
    Ok(peers) if !peers.is_empty() => {
        // Success: found providers
        return Ok(peers);
    }
    Ok(peers) if peers.is_empty() => {
        // Transient: DHT hasn't propagated yet
        // Retry with backoff
        sleep(exponential_backoff(attempt)).await;
        return network.find_providers(&cid).await;
    }
    Err(NetworkError::PeerOffline(peer)) => {
        // Update reputation
        network.mark_offline(&peer)?;
        
        // Try bootstrap nodes
        let bootstrap = network.get_bootstrap_peers();
        return network.find_providers_via(&cid, bootstrap).await;
    }
    Err(e) => {
        error!("Network error: {:?}", e);
        return Err(e.into());
    }
}
```

**Resilience Strategies**:
- Peer reputation decay: stale peers ranked lower
- Multiple bootstrap nodes: don't depend on one
- Retry with exponential backoff: transient issues recover
- Fallback to cached provider list: if DHT down

---

### 3. Semantic Domain Errors

**Error**: ML model inference fails, HNSW index corrupted

```rust
pub enum SemanticError {
    ModelInferenceFailed(String),
    HNSWIndexCorrupted,
    EmbeddingCacheFull,
    QueryTimeout,
}

// Handler
match semantic.search(&query, k).await {
    Ok(results) => Ok(results),
    Err(SemanticError::ModelInferenceFailed(reason)) => {
        // Fallback: return empty or cached results
        warn!("Model inference failed: {}, using cache", reason);
        
        // Check query cache for similar queries
        if let Some(cached) = query_cache.find_similar(&query) {
            return Ok(cached);
        }
        
        // Last resort: return empty results
        return Ok(Vec::new());
    }
    Err(SemanticError::HNSWIndexCorrupted) => {
        // Rebuild index from scratch
        warn!("HNSW index corrupted, rebuilding...");
        semantic.rebuild_index_from_storage().await?;
        
        // Retry search
        semantic.search(&query, k).await
    }
    Err(e) => Err(e.into()),
}
```

**Graceful Degradation**:
- Model error → use cache or skip semantic search
- Index error → rebuild from blocks in storage
- Timeout → return partial results

---

### 4. Logic Domain Errors

**Error**: Infinite loop, stack overflow, inconsistent rules

```rust
pub enum LogicError {
    MaxDepthExceeded,
    StackOverflow,
    InconsistentRules,
    InvalidRule,
}

// Handler
fn infer_with_limits(goal: &Predicate, rules: &[Rule], 
                     depth: usize, max_depth: usize) 
    -> Result<Vec<Substitution>> {
    
    if depth > max_depth {
        return Err(LogicError::MaxDepthExceeded);
    }
    
    // Detect infinite loops
    if self.visit_history.contains(goal) {
        return Err(LogicError::InfiniteLoop);
    }
    self.visit_history.insert(goal.clone());
    
    // ... normal inference ...
    
    self.visit_history.remove(goal);
    Ok(solutions)
}

// Limits:
// - max_depth = 1000 (prevent stack overflow)
// - max_solutions = 10000 (prevent memory explosion)
// - timeout = 5 seconds per query
```

**Safety Guarantees**:
- Bounded recursion: max_depth stops infinite loops
- Solution limits: prevent memory explosion
- Timeout: avoid hanging queries
- Rule validation: catch inconsistencies at load time

---

### 5. Transport Domain Errors

**Error**: Session timeout, peer sends bad block, ledger imbalance

```rust
pub enum TransportError {
    SessionTimeout,
    BadBlockReceived { expected_cid: Cid, got_hash: Hash },
    PeerLedgerImbalance(PeerId),
    WantListFull,
}

// Handler
match transport.fetch_blocks(&cids, peer).await {
    Ok(blocks) => {
        // Verify each block
        for block in blocks {
            if hash(&block.data) != block.cid {
                // Malicious peer detected!
                transport.penalize_peer(&peer)?;
                return Err(TransportError::BadBlockReceived { ... });
            }
        }
        Ok(blocks)
    }
    Err(TransportError::SessionTimeout) => {
        // Switch to different peer
        let next_peer = peer_selection.next_best(&[peer])?;
        transport.switch_to_peer(&next_peer).await
    }
    Err(TransportError::PeerLedgerImbalance(peer)) => {
        // Peer taking without giving
        transport.throttle_peer(&peer)?;
        transport.try_next_peer().await
    }
    Err(e) => Err(e.into()),
}
```

**Defenses**:
- Hash verification: every block checked
- Peer scoring: malicious peers downranked
- Ledger fairness: throttle leechers
- Session timeout: don't wait forever
- Parallel peers: don't depend on one slow peer

---

## Retry Strategies

### 1. Exponential Backoff with Jitter

```rust
fn backoff(attempt: u32, base_ms: u64) -> Duration {
    let exponential = base_ms * 2_u64.pow(attempt.min(8));  // Cap at 2^8
    let jitter = rand::random::<u64>() % (exponential / 2);
    Duration::from_millis(exponential + jitter)
}

// Attempt 1: 10ms + jitter
// Attempt 2: 20ms + jitter
// Attempt 3: 40ms + jitter
// ...
// Attempt 8+: 2560ms + jitter
// Max total: ~5 seconds with 10 retries
```

### 2. Circuit Breaker

```rust
pub struct CircuitBreaker {
    state: State,  // Closed / Open / HalfOpen
    failure_count: u32,
    last_failure_time: Instant,
    threshold: u32,      // Open after N failures
    timeout: Duration,   // Try again after timeout
}

pub enum State {
    Closed,       // Normal: requests go through
    Open,         // Failing: reject requests
    HalfOpen,     // Testing: allow one request to probe
}

impl CircuitBreaker {
    pub async fn call<F, T>(&mut self, f: F) -> Result<T>
    where F: Fn() -> Future<Output = Result<T>>
    {
        match self.state {
            State::Closed => {
                match f().await {
                    Ok(result) => {
                        self.failure_count = 0;
                        Ok(result)
                    }
                    Err(e) => {
                        self.failure_count += 1;
                        if self.failure_count >= self.threshold {
                            self.state = State::Open;
                            self.last_failure_time = now();
                        }
                        Err(e)
                    }
                }
            }
            State::Open => {
                if now() - self.last_failure_time > self.timeout {
                    self.state = State::HalfOpen;
                    f().await  // Try once
                } else {
                    Err("Circuit breaker open")
                }
            }
            State::HalfOpen => {
                match f().await {
                    Ok(result) => {
                        self.state = State::Closed;
                        self.failure_count = 0;
                        Ok(result)
                    }
                    Err(e) => {
                        self.state = State::Open;
                        self.last_failure_time = now();
                        Err(e)
                    }
                }
            }
        }
    }
}

// Example: Peer consistently slow
// 1. Closed: try peer (5 failures) → state = Open
// 2. Open: reject requests for 30s
// 3. After 30s: HalfOpen, try one request
// 4. If succeeds: Closed again
```

---

## Observability & Alerting

```rust
pub enum Alert {
    CorruptionDetected { cid: Cid },
    PeerOffline { peer_id: PeerId },
    DHTPropagationDelay { cid: Cid },
    SessionTimeout { session_id: SessionId },
    LowDiskSpace { percentage: u32 },
    HighMemoryUsage { percentage: u32 },
}

pub struct MetricsCollector {
    // Errors
    corruption_detected: Counter,
    corruption_repaired: Counter,
    peer_failures: Counter,
    session_timeouts: Counter,
    
    // Recovery
    retries_succeeded: Counter,
    circuit_breaker_trips: Counter,
    
    // Latency percentiles
    block_fetch_p50: Histogram,
    block_fetch_p99: Histogram,
    block_fetch_p999: Histogram,
}

// Example alert flow:
// 1. Error detected
// 2. Increment metric
// 3. Emit alert if threshold exceeded
// 4. Operator sees in dashboard
// 5. Manual intervention if needed
```

---

## Failure Modes & Mitigations

| Failure Mode | Symptom | Mitigation |
|--------------|---------|-----------|
| **Peer offline** | DHT returns []  | Retry, use cached providers |
| **Block corrupt** | hash() mismatch | Fetch from other peers |
| **Storage full** | OutOfSpace error | Trigger GC, evict old blocks |
| **Network partition** | All peers unreachable | Wait for reconnect, cache locally |
| **Malicious peer** | Returns wrong hash | Penalize, blacklist peer |
| **Timeout** | No response | Exponential backoff, switch peer |
| **Logic infinite loop** | Inference hangs | Max depth limit, timeout |
| **HNSW corrupted** | Search fails | Rebuild from blocks |

---

## Testing Error Paths

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_corruption_detection() {
        // Simulate block corruption
        let cid = Cid::from("bafybeig...");
        let corrupted_data = b"wrong data";
        
        // Verify corruption is detected
        assert!(verify_hash(corrupted_data, &cid).is_err());
    }
    
    #[test]
    async fn test_peer_failure_retry() {
        // Peer 1 times out
        // Verify Transport switches to Peer 2
        let session = create_session(&[peer1, peer2]);
        
        simulate_timeout(peer1);
        let result = session.fetch_blocks().await;
        
        assert!(result.is_ok());
        assert_eq!(session.used_peer, peer2);
    }
    
    #[test]
    async fn test_circuit_breaker() {
        let mut breaker = CircuitBreaker::new(threshold: 3);
        
        // Fail 3 times
        for _ in 0..3 { breaker.call(failing_fn).await.ok(); }
        assert_eq!(breaker.state, State::Open);
        
        // Reject immediately
        let err = breaker.call(slow_fn).await;
        assert!(err.is_err());
        assert!(err.to_string().contains("Circuit breaker"));
    }
}
```

---

## Best Practices

✅ **Do**:
- Fail fast: detect errors early
- Verify invariants: hash(data) == cid always
- Use timeouts: prevent hanging forever
- Retry with backoff: transient errors recover
- Log errors: observability is crucial
- Test error paths: failures should be predictable

❌ **Don't**:
- Silent failures: don't hide errors
- Panic: use Result<T> always
- Retry forever: bounded attempts
- Trust peer: verify everything
- Assume network: always plan for partitions

---

## Что дальше?

→ [10-Performance](10-Performance.md) для метрик надёжности  
→ [03-Bounded Contexts](03-BoundedContexts.md) для domain-specific errors  
→ Real code: `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/*/src/error.rs`

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [04-StorageDomain](04-StorageDomain.md) | [05-NetworkDomain](05-NetworkDomain.md) | [10-Performance](10-Performance.md)
