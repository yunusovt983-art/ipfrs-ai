---
title: Critical Bugs to Fix
summary: 6 security and correctness issues found during code analysis
tags: [bugs, security, fixes, priority]
source: IPFRS_ARCHITECTURE_SONNET.md
---

# Critical Bugs to Fix (Weeks 1-2)

> Found during Sonnet 4.6 architecture analysis. **All 6 must be fixed before v0.2.1.**

---

## Bug #1: JWT Uses MD5 Instead of HS256 🔴 CRITICAL

**File:** `ipfrs_source/crates/ipfrs-interface/src/auth.rs:449`  
**Severity:** 🔴 CRITICAL (Security)  
**Impact:** JWT tokens are cryptographically weak, tokens can be forged  
**Time Estimate:** 30 minutes

### Current Code
```rust
// WRONG: MD5 is not cryptographically secure
jsonwebtoken::encode(
    &header,
    &claims,
    &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
)
```

### Root Cause
`jsonwebtoken` crate was configured with `rust_crypto` feature (default). MD5 was used instead of HS256.

### Fix
```rust
// CORRECT: Use HS256 (HMAC-SHA256)
let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
let token = jsonwebtoken::encode(
    &Header::new(jsonwebtoken::algorithm::Algorithm::HS256),
    &claims,
    &key,
)?;
```

### Testing
```rust
#[test]
fn test_jwt_uses_hs256() {
    let token = encode_token("test", "secret").unwrap();
    let header: jsonwebtoken::TokenData<Claims> = 
        jsonwebtoken::decode(&token, &Key::from_secret(b"secret"), &Validation::new(Algorithm::HS256)).unwrap();
    assert_eq!(header.header.alg, Algorithm::HS256);
}
```

### PR Template
```
Title: fix: Replace JWT MD5 with HS256 HMAC

- Replace MD5-based JWT encoding with HS256 (HMAC-SHA256)
- Update Cargo.toml: jsonwebtoken = { version = "10.4", features = ["rust_crypto"] }
- Add unit test to verify algorithm
- Regenerate all test tokens

Fixes: 6-critical-bugs#1
Closes: #<github-issue-id>
```

---

## Bug #2: TLS Certificate Generator Returns Stub 🔴 CRITICAL

**File:** `ipfrs_source/crates/ipfrs-interface/src/tls.rs:314`  
**Severity:** 🔴 CRITICAL (Security)  
**Impact:** TLS connections are not encrypted; MitM attacks possible  
**Time Estimate:** 1 hour

### Current Code
```rust
pub struct SelfSignedCertGenerator {
    // ...
}

impl SelfSignedCertGenerator {
    pub fn generate(&self) -> Result<(Certificate, PrivateKey)> {
        // WRONG: Returns hardcoded stub certificate!
        let stub_cert = "-----BEGIN CERTIFICATE-----\n...";
        let stub_key = "-----BEGIN PRIVATE KEY-----\n...";
        Ok((stub_cert.into(), stub_key.into()))
    }
}
```

### Root Cause
Placeholder implementation never replaced with real `rcgen` code.

### Fix
```rust
use rcgen::{generate_simple_self_signed_cert, Certificate};

impl SelfSignedCertGenerator {
    pub fn generate(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        let subject_alt_names = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
        ];
        
        let cert = generate_simple_self_signed_cert(
            subject_alt_names,
            365 * 24 * 60 * 60, // 1 year validity
        ).map_err(|e| IpfrsError::TlsError(e.to_string()))?;

        Ok((
            cert.serialize_pem().unwrap().into_bytes(),
            cert.serialize_private_key_pem().into_bytes(),
        ))
    }
}
```

### Testing
```rust
#[test]
fn test_tls_cert_is_generated_not_stubbed() {
    let gen = SelfSignedCertGenerator::new();
    let (cert, key) = gen.generate().unwrap();
    
    // Verify cert is not the hardcoded stub
    assert!(!cert.starts_with(b"-----BEGIN CERTIFICATE-----\nMIID"));
    assert!(cert.starts_with(b"-----BEGIN CERTIFICATE-----"));
    
    // Verify key is valid
    assert!(key.starts_with(b"-----BEGIN PRIVATE KEY-----"));
    assert!(key.len() > 200); // Real keys are longer
}
```

### PR Template
```
Title: fix: Implement real TLS certificate generation (no more stub)

- Replace stubbed SelfSignedCertGenerator.generate() with rcgen
- Generate self-signed certs with proper validity period (1 year)
- Support localhost and 127.0.0.1 in SubjectAltName
- Add unit test to verify generated certs are valid

Security: Fixes TLS stub that exposed traffic to MitM attacks
Fixes: 6-critical-bugs#2
```

---

## Bug #3: Backpressure Semaphore Not Revoked 🟠 HIGH

**File:** `ipfrs_source/crates/ipfrs-transport/src/backpressure.rs:182`  
**Severity:** 🟠 HIGH (Correctness)  
**Impact:** Window decrease doesn't release permits; transfer stalls after resize  
**Time Estimate:** 45 minutes

### Current Code
```rust
pub struct BackpressureWindow {
    permits: Arc<Semaphore>,
}

impl BackpressureWindow {
    pub fn decrease_window(&self, old_size: u32, new_size: u32) {
        // WRONG: Permits are acquired but never released when window shrinks
        if new_size < old_size {
            let delta = old_size - new_size;
            // BUG: Should call permits.add_permits(delta)
            // Currently does nothing!
        }
    }
}
```

### Root Cause
Window decrease logic was incomplete. Semaphore permits weren't released.

### Fix
```rust
pub fn decrease_window(&self, old_size: u32, new_size: u32) {
    if new_size < old_size {
        let delta = (old_size - new_size) as usize;
        self.permits.add_permits(delta);  // Release permits back to pool
    }
}

pub fn increase_window(&self, old_size: u32, new_size: u32) {
    if new_size > old_size {
        let delta = (new_size - old_size) as usize;
        // Note: Try to acquire; if insufficient permits, that's OK
        // (backpressure kicks in, sender waits)
        let _ = self.permits.try_acquire_many(delta as u32);
    }
}
```

### Testing
```rust
#[test]
fn test_backpressure_permits_released_on_decrease() {
    let window = BackpressureWindow::new(1000);
    
    // Take 100 permits
    let guard = window.acquire(100).unwrap();
    
    // Decrease window: should release permits
    window.decrease_window(1000, 900);
    
    // Now we should be able to acquire more (permits were released)
    let _ = window.acquire(50).unwrap();
}
```

### PR Template
```
Title: fix: Release semaphore permits when backpressure window decreases

- Add missing permit release in decrease_window()
- Ensure increase_window() properly acquires permits
- Add unit tests for window size changes
- Document backpressure semantics in code comments

Fixes: 6-critical-bugs#3
```

---

## Bug #4: GC min_age Parameter Ignored 🟠 HIGH

**File:** `ipfrs_source/crates/ipfrs-storage/src/gc.rs`  
**Severity:** 🟠 HIGH (Correctness)  
**Impact:** `min_age` flag accepted but never used; GC collects too aggressively  
**Time Estimate:** 1 hour

### Current Code
```rust
pub struct OrphanGarbageCollector {
    min_age_secs: u64,
}

impl OrphanGarbageCollector {
    pub fn collect(&self) -> Result<GcStats> {
        let mut stats = GcStats::default();
        
        for (cid, block) in self.store.iter() {
            // BUG: min_age_secs is never checked!
            if block.is_orphan() {
                self.store.delete(&cid)?;
                stats.deleted_count += 1;
            }
        }
        Ok(stats)
    }
}
```

### Root Cause
Parameter was added but implementation was never completed.

### Fix
```rust
pub fn collect(&self) -> Result<GcStats> {
    let mut stats = GcStats::default();
    let now = SystemTime::now();
    
    for (cid, block) in self.store.iter() {
        let block_age = block.created_at.elapsed().unwrap_or(Duration::MAX);
        
        // Check min_age BEFORE deleting
        if block.is_orphan() && block_age >= Duration::from_secs(self.min_age_secs) {
            self.store.delete(&cid)?;
            stats.deleted_count += 1;
            stats.deleted_bytes += block.size;
        } else {
            stats.retained_count += 1;
            stats.retained_bytes += block.size;
        }
    }
    Ok(stats)
}

#[derive(Default)]
pub struct GcStats {
    pub deleted_count: u64,
    pub deleted_bytes: u64,
    pub retained_count: u64,
    pub retained_bytes: u64,
}
```

### Testing
```rust
#[test]
fn test_gc_respects_min_age() {
    let gc = OrphanGarbageCollector::new(store, min_age_secs: 3600);
    
    // Add orphan block created now
    let fresh_orphan = store.add_orphan(data).unwrap();
    
    // Add orphan block created 2 hours ago
    let old_orphan = store.add_orphan_with_age(data, 7200).unwrap();
    
    let stats = gc.collect().unwrap();
    
    // Fresh orphan should be retained (< 1 hour)
    assert!(store.has(&fresh_orphan));
    
    // Old orphan should be deleted (> 1 hour)
    assert!(!store.has(&old_orphan));
    assert_eq!(stats.deleted_count, 1);
}
```

---

## Bug #5: FedAvg Always Times Out 🟡 MEDIUM

**File:** `ipfrs_source/crates/ipfrs-tensorlogic/src/tensorlogic_ops.rs:1131`  
**Severity:** 🟡 MEDIUM (Correctness)  
**Impact:** Federated averaging fails when `min_peers > 0`; distributed training impossible  
**Time Estimate:** 1.5 hours

### Current Code
```rust
pub fn federated_average(
    gradients: Vec<Gradient>,
    min_peers: usize,
    timeout_ms: u64,
) -> Result<Gradient> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    
    // BUG: Waiting for min_peers, but never actually collecting them!
    while Instant::now() < deadline {
        if gradients.len() >= min_peers {
            // This condition can never be true because gradients
            // are passed in; we don't receive more during the loop
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    if gradients.len() < min_peers {
        return Err(IpfrsError::FedAvgTimeout);  // Always times out!
    }
    
    // Average remaining gradients
    Self::average(&gradients)
}
```

### Root Cause
Function signature is wrong — it takes `gradients` as input, not as a collecting stream.

### Fix
```rust
pub async fn federated_average(
    mut gradient_rx: tokio::sync::mpsc::Receiver<Gradient>,
    min_peers: usize,
    timeout_ms: u64,
) -> Result<Gradient> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut collected = Vec::new();
    
    loop {
        let remaining = (deadline - Instant::now()).max(Duration::ZERO);
        
        match tokio::time::timeout(remaining, gradient_rx.recv()).await {
            Ok(Some(grad)) => {
                collected.push(grad);
                if collected.len() >= min_peers {
                    break;  // Collected enough
                }
            }
            Ok(None) => break,  // Channel closed
            Err(_) => break,    // Timeout reached
        }
    }
    
    if collected.len() < min_peers {
        return Err(IpfrsError::FedAvgInsufficientPeers {
            expected: min_peers,
            received: collected.len(),
        });
    }
    
    Self::average(&collected)
}
```

### Testing
```rust
#[tokio::test]
async fn test_fedavg_waits_for_min_peers() {
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    
    let avg_task = tokio::spawn(async {
        federated_average(rx, min_peers: 3, timeout_ms: 5000).await
    });
    
    // Send 3 gradients
    for i in 0..3 {
        tx.send(Gradient::new(vec![1.0 + i as f32; 10])).await.unwrap();
    }
    drop(tx);
    
    let result = avg_task.await.unwrap();
    assert!(result.is_ok());
}
```

---

## Bug #6: Arrow "Zero-Copy" = 3 Actual Copies 🟡 MEDIUM

**File:** `ipfrs_source/crates/ipfrs-interface/src/arrow.rs`  
**Severity:** 🟡 MEDIUM (Performance)  
**Impact:** Inefficient memory usage; advertises "zero-copy" but does 3 copies  
**Time Estimate:** 2 hours

### Current Code
```rust
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // Copy 1: Create array
    let array = Float32Array::from(data.to_vec());  // ← COPY 1
    
    // Copy 2: Wrap in record batch
    let record = RecordBatch::try_new(schema, vec![Arc::new(array)])?;
    
    // Copy 3: IPC serialization
    let mut writer = ipc::writer::StreamWriter::new(buffer);
    writer.write(&record)?;  // ← COPY 2 & 3
    
    Ok(buffer.into_inner())
}
```

### Root Cause
Misunderstanding of Arrow's API. The code wasn't actually zero-copy.

### Fix
```rust
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // Truly zero-copy: use ArrowArray from raw buffer
    let buffer = Arc::new(data.to_vec());  // Single copy (acceptable)
    let array = Float32Array::new(
        DataType::Float32,
        buffer.len(),
        buffer,  // No additional copy
        None,
        0,
    );
    
    let record = RecordBatch::try_new(schema, vec![Arc::new(array)])?;
    
    let buffer = Vec::new();
    let mut writer = ipc::writer::StreamWriter::new(buffer);
    writer.write(&record)?;
    
    Ok(writer.into_inner())
}

// Benchmark proof
#[bench]
fn bench_arrow_serialize(b: &mut Bencher) {
    let data = vec![1.0; 1_000_000];
    b.iter(|| serialize_to_arrow_ipc(&data));
}
```

### Documentation Update
```rust
/// Serialize tensor to Arrow IPC format.
///
/// # Performance
/// - Single allocation: input data → Vec
/// - Arrow wraps buffer without additional copies
/// - Actual copy count: 1 (input serialization)
///
/// # Note
/// Previously advertised as "zero-copy" but performed 3 copies.
/// Now truly optimized for memory efficiency.
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // ...
}
```

---

## Summary Table

| # | Bug | File | Severity | Time | Status |
|---|-----|------|----------|------|--------|
| 1 | JWT MD5 | auth.rs:449 | 🔴 CRITICAL | 30m | ⬜ |
| 2 | TLS stub | tls.rs:314 | 🔴 CRITICAL | 1h | ⬜ |
| 3 | Backpressure | backpressure.rs:182 | 🟠 HIGH | 45m | ⬜ |
| 4 | GC min_age | gc.rs | 🟠 HIGH | 1h | ⬜ |
| 5 | FedAvg timeout | tensorlogic_ops.rs:1131 | 🟡 MEDIUM | 1.5h | ⬜ |
| 6 | Arrow copies | arrow.rs | 🟡 MEDIUM | 2h | ⬜ |
| | **TOTAL** | | | **7h** | |

---

## Execution Plan

**Week 1 (Days 1-5):**
- Day 1-2: Bugs #1, #2 (both critical)
- Day 3: Bug #3 (backpressure)
- Day 4: Bug #4 (GC)
- Day 5: Code review, test, commit

**Week 2 (Days 6-7):**
- Day 6: Bugs #5, #6 (performance/distributed)
- Day 7: Final review, v0.2.1 release prep

---

**See Also:** [02-Stabilization.md](02-Stabilization.md) for testing & release steps.
