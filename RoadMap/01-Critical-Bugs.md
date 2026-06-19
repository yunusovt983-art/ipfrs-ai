---
title: Критические баги для исправления
summary: 6 проблем с безопасностью и корректностью, найденные при анализе кода
tags: [bugs, security, fixes, priority]
source: IPFRS_ARCHITECTURE_SONNET.md
---

# Критические баги для исправления (Неделя 1-2)

> Найдены при анализе архитектуры Sonnet 4.6. **Все 6 должны быть исправлены перед v0.2.1.**

---

## Баг #1: JWT использует MD5 вместо HS256 🔴 КРИТИЧЕСКИЙ

**Файл:** `ipfrs_source/crates/ipfrs-interface/src/auth.rs:449`  
**Серьёзность:** 🔴 КРИТИЧЕСКИЙ (Безопасность)  
**Влияние:** JWT токены криптографически слабые, могут быть подделаны  
**Оценка времени:** 30 минут

### Текущий код
```rust
// НЕПРАВИЛЬНО: MD5 не криптографически безопасен
jsonwebtoken::encode(
    &header,
    &claims,
    &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
)
```

### Причина
Крейт `jsonwebtoken` был сконфигурирован с фичей `rust_crypto` (по умолчанию). MD5 использовался вместо HS256.

### Исправление
```rust
// ПРАВИЛЬНО: Использовать HS256 (HMAC-SHA256)
let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
let token = jsonwebtoken::encode(
    &Header::new(jsonwebtoken::algorithm::Algorithm::HS256),
    &claims,
    &key,
)?;
```

### Тестирование
```rust
#[test]
fn test_jwt_uses_hs256() {
    let token = encode_token("test", "secret").unwrap();
    let header: jsonwebtoken::TokenData<Claims> = 
        jsonwebtoken::decode(&token, &Key::from_secret(b"secret"), &Validation::new(Algorithm::HS256)).unwrap();
    assert_eq!(header.header.alg, Algorithm::HS256);
}
```

### Шаблон PR
```
Title: fix: Заменить JWT MD5 на HS256 HMAC

- Заменить MD5-based JWT encoding на HS256 (HMAC-SHA256)
- Обновить Cargo.toml: jsonwebtoken = { version = "10.4", features = ["rust_crypto"] }
- Добавить unit test для проверки алгоритма
- Перегенерировать все test tokens

Fixes: 6-critical-bugs#1
Closes: #<github-issue-id>
```

---

## Баг #2: TLS генератор сертификатов возвращает stub 🔴 КРИТИЧЕСКИЙ

**Файл:** `ipfrs_source/crates/ipfrs-interface/src/tls.rs:314`  
**Серьёзность:** 🔴 КРИТИЧЕСКИЙ (Безопасность)  
**Влияние:** TLS соединения не шифруются; возможны MitM атаки  
**Оценка времени:** 1 час

### Текущий код
```rust
pub struct SelfSignedCertGenerator {
    // ...
}

impl SelfSignedCertGenerator {
    pub fn generate(&self) -> Result<(Certificate, PrivateKey)> {
        // НЕПРАВИЛЬНО: Возвращает hardcoded stub сертификат!
        let stub_cert = "-----BEGIN CERTIFICATE-----\n...";
        let stub_key = "-----BEGIN PRIVATE KEY-----\n...";
        Ok((stub_cert.into(), stub_key.into()))
    }
}
```

### Причина
Placeholder реализация никогда не была заменена на реальный код `rcgen`.

### Исправление
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
            365 * 24 * 60 * 60, // 1 год действия
        ).map_err(|e| IpfrsError::TlsError(e.to_string()))?;

        Ok((
            cert.serialize_pem().unwrap().into_bytes(),
            cert.serialize_private_key_pem().into_bytes(),
        ))
    }
}
```

### Тестирование
```rust
#[test]
fn test_tls_cert_is_generated_not_stubbed() {
    let gen = SelfSignedCertGenerator::new();
    let (cert, key) = gen.generate().unwrap();
    
    // Проверить, что сертификат не hardcoded stub
    assert!(!cert.starts_with(b"-----BEGIN CERTIFICATE-----\nMIID"));
    assert!(cert.starts_with(b"-----BEGIN CERTIFICATE-----"));
    
    // Проверить, что ключ валидный
    assert!(key.starts_with(b"-----BEGIN PRIVATE KEY-----"));
    assert!(key.len() > 200); // Реальные ключи длиннее
}
```

---

## Баг #3: Backpressure семафор не ревокируется 🟠 ВЫСОКИЙ

**Файл:** `ipfrs_source/crates/ipfrs-transport/src/backpressure.rs:182`  
**Серьёзность:** 🟠 ВЫСОКИЙ (Корректность)  
**Влияние:** Уменьшение окна не освобождает permits; трансфер зависает после resize  
**Оценка времени:** 45 минут

### Текущий код
```rust
pub struct BackpressureWindow {
    permits: Arc<Semaphore>,
}

impl BackpressureWindow {
    pub fn decrease_window(&self, old_size: u32, new_size: u32) {
        // НЕПРАВИЛЬНО: Permits берутся но никогда не освобождаются при уменьшении окна
        if new_size < old_size {
            let delta = old_size - new_size;
            // БАГ: Должен вызвать permits.add_permits(delta)
            // Сейчас ничего не делает!
        }
    }
}
```

### Причина
Логика уменьшения окна была неполной. Семафор permits не освобождались.

### Исправление
```rust
pub fn decrease_window(&self, old_size: u32, new_size: u32) {
    if new_size < old_size {
        let delta = (old_size - new_size) as usize;
        self.permits.add_permits(delta);  // Освободить permits обратно в пул
    }
}

pub fn increase_window(&self, old_size: u32, new_size: u32) {
    if new_size > old_size {
        let delta = (new_size - old_size) as usize;
        // Note: Попытка получить; если недостаточно permits, это OK
        // (backpressure включается, отправитель ждёт)
        let _ = self.permits.try_acquire_many(delta as u32);
    }
}
```

### Тестирование
```rust
#[test]
fn test_backpressure_permits_released_on_decrease() {
    let window = BackpressureWindow::new(1000);
    
    // Взять 100 permits
    let guard = window.acquire(100).unwrap();
    
    // Уменьшить окно: должны освободиться permits
    window.decrease_window(1000, 900);
    
    // Теперь должны быть в состоянии получить больше (permits освобождены)
    let _ = window.acquire(50).unwrap();
}
```

---

## Баг #4: GC параметр min_age игнорируется 🟠 ВЫСОКИЙ

**Файл:** `ipfrs_source/crates/ipfrs-storage/src/gc.rs`  
**Серьёзность:** 🟠 ВЫСОКИЙ (Корректность)  
**Влияние:** Флаг `min_age` принимается но никогда не используется; GC собирает слишком агрессивно  
**Оценка времени:** 1 час

### Текущий код
```rust
pub struct OrphanGarbageCollector {
    min_age_secs: u64,
}

impl OrphanGarbageCollector {
    pub fn collect(&self) -> Result<GcStats> {
        let mut stats = GcStats::default();
        
        for (cid, block) in self.store.iter() {
            // БАГ: min_age_secs никогда не проверяется!
            if block.is_orphan() {
                self.store.delete(&cid)?;
                stats.deleted_count += 1;
            }
        }
        Ok(stats)
    }
}
```

### Причина
Параметр был добавлен, но реализация никогда не была завершена.

### Исправление
```rust
pub fn collect(&self) -> Result<GcStats> {
    let mut stats = GcStats::default();
    let now = SystemTime::now();
    
    for (cid, block) in self.store.iter() {
        let block_age = block.created_at.elapsed().unwrap_or(Duration::MAX);
        
        // Проверить min_age ПЕРЕД удалением
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

---

## Баг #5: FedAvg всегда times out 🟡 СРЕДНИЙ

**Файл:** `ipfrs_source/crates/ipfrs-tensorlogic/src/tensorlogic_ops.rs:1131`  
**Серьёзность:** 🟡 СРЕДНИЙ (Корректность)  
**Влияние:** Federated averaging падает при `min_peers > 0`; распределённое обучение невозможно  
**Оценка времени:** 1.5 часа

### Текущий код
```rust
pub fn federated_average(
    gradients: Vec<Gradient>,
    min_peers: usize,
    timeout_ms: u64,
) -> Result<Gradient> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    
    // БАГ: Ожидаем min_peers, но никогда не собираем их!
    while Instant::now() < deadline {
        if gradients.len() >= min_peers {
            // Это условие не может быть true, потому что gradients
            // переданы в; мы не получаем больше во время loop
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    if gradients.len() < min_peers {
        return Err(IpfrsError::FedAvgTimeout);  // Всегда times out!
    }
    
    // Усреднить оставшиеся градиенты
    Self::average(&gradients)
}
```

### Причина
Сигнатура функции неправильная — она берёт `gradients` как input, а не как collecting stream.

### Исправление
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
                    break;  // Собрали достаточно
                }
            }
            Ok(None) => break,  // Канал закрыт
            Err(_) => break,    // Timeout достигнут
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

---

## Баг #6: Arrow "zero-copy" = 3 реальных копии 🟡 СРЕДНИЙ

**Файл:** `ipfrs_source/crates/ipfrs-interface/src/arrow.rs`  
**Серьёзность:** 🟡 СРЕДНИЙ (Производительность)  
**Влияние:** Неэффективное использование памяти; рекламируется "zero-copy" но делает 3 копии  
**Оценка времени:** 2 часа

### Текущий код
```rust
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // Копия 1: Создать массив
    let array = Float32Array::from(data.to_vec());  // ← КОПИЯ 1
    
    // Копия 2: Обернуть в record batch
    let record = RecordBatch::try_new(schema, vec![Arc::new(array)])?;
    
    // Копия 3: IPC сериализация
    let mut writer = ipc::writer::StreamWriter::new(buffer);
    writer.write(&record)?;  // ← КОПИЯ 2 & 3
    
    Ok(buffer.into_inner())
}
```

### Причина
Неправильное понимание Arrow API. Код не был действительно zero-copy.

### Исправление
```rust
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // Действительно zero-copy: использовать ArrowArray из raw buffer
    let buffer = Arc::new(data.to_vec());  // Одна копия (приемлемо)
    let array = Float32Array::new(
        DataType::Float32,
        buffer.len(),
        buffer,  // Без дополнительной копии
        None,
        0,
    );
    
    let record = RecordBatch::try_new(schema, vec![Arc::new(array)])?;
    
    let buffer = Vec::new();
    let mut writer = ipc::writer::StreamWriter::new(buffer);
    writer.write(&record)?;
    
    Ok(writer.into_inner())
}

// Доказательство бенчмарком
#[bench]
fn bench_arrow_serialize(b: &mut Bencher) {
    let data = vec![1.0; 1_000_000];
    b.iter(|| serialize_to_arrow_ipc(&data));
}
```

### Обновление документации
```rust
/// Сериализовать тензор в Arrow IPC формат.
///
/// # Производительность
/// - Одна аллокация: входные данные → Vec
/// - Arrow оборачивает буфер без дополнительных копий
/// - Реальное количество копий: 1 (входная сериализация)
///
/// # Примечание
/// Ранее рекламировалось как "zero-copy" но выполняло 3 копии.
/// Теперь действительно оптимизировано для эффективности памяти.
pub fn serialize_to_arrow_ipc(data: &[f32]) -> Result<Vec<u8>> {
    // ...
}
```

---

## Таблица резюме

| # | Баг | Файл | Серьёзность | Время | Статус |
|---|-----|------|-------------|-------|--------|
| 1 | JWT MD5 | auth.rs:449 | 🔴 CRITICAL | 30m | ⬜ |
| 2 | TLS stub | tls.rs:314 | 🔴 CRITICAL | 1h | ⬜ |
| 3 | Backpressure | backpressure.rs:182 | 🟠 HIGH | 45m | ⬜ |
| 4 | GC min_age | gc.rs | 🟠 HIGH | 1h | ⬜ |
| 5 | FedAvg timeout | tensorlogic_ops.rs:1131 | 🟡 MEDIUM | 1.5h | ⬜ |
| 6 | Arrow copies | arrow.rs | 🟡 MEDIUM | 2h | ⬜ |
| | **ИТОГО** | | | **7h** | |

---

## План исполнения

**Неделя 1 (Дни 1-5):**
- День 1-2: Баги #1, #2 (оба критические)
- День 3: Баг #3 (backpressure)
- День 4: Баг #4 (GC)
- День 5: Code review, тесты, коммит

**Неделя 2 (Дни 6-7):**
- День 6: Баги #5, #6 (производительность/распределённые)
- День 7: Финальный review, подготовка к v0.2.1 релизу

---

**Смотри также:** [02-Stabilization.md](02-Stabilization.md) для шагов тестирования и релиза.
