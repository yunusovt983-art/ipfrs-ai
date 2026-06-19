---
title: 04-StorageContext
type: domain
summary: Storage Context (ipfrs-storage) — порт BlockStore, реальный стек декораторов Bloom→Cache→Sled, GC, пины, тиринг
tags: [ipfrs, ddd, storage, blockstore, gc, decorator]
source: ipfrs_source/crates/ipfrs-storage/src/
related: ["[[03-SharedKernel]]", "[[09-ApplicationLayer]]", "[[11-RealityCheck]]"]
read_time: 14 мин
updated: 2026-06-19
---

# Storage Context — `ipfrs-storage`

**Краткое резюме**: Storage — это контекст **долговечного контент-адресуемого
хранения**. Его единственный настоящий порт-Репозиторий — трейт `BlockStore`
(`traits.rs:9`), а канонический агрегат — `Block` из ядра. Вокруг ядра —
композируемый стек **декораторов** (Bloom, Cache, Dedup, TTL, Quota…) и большой
спутниковый слой сервисов.

---

## 1. Порт-Репозиторий: `BlockStore`

```rust
// traits.rs:9 — Repository в стиле коллекции (ключ = CID, без языка запросов)
#[async_trait] pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;            // :11
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;     // :23
    async fn has(&self, cid: &Cid) -> Result<bool>;              // :36
    async fn delete(&self, cid: &Cid) -> Result<()>;             // :49
    fn list_cids(&self) -> Result<Vec<Cid>>;                     // :61
    fn len(&self) -> usize;                                      // :64
    // + put_many/get_many/has_many/delete_many, flush, close
}
// blanket impl для Arc<S: BlockStore> — traits.rs:87 (делает стек композируемым)
```

Адаптеры (concrete repositories): `SledBlockStore` (деф., `blockstore.rs:178`),
`MemoryBlockStore` (`memory.rs:14`), `ParityDbBlockStore` (`paritydb.rs:112`),
`S3BlockStore` (`s3.rs:88`), `MmapBlockStore` (`mmap.rs`).

**Реальная форма `SledBlockStore`** (`blockstore.rs:178`) — всего 3 поля:
```rust
pub struct SledBlockStore { db: Db, dedup_stats: Arc<…>, compaction_scheduler: Arc<…> }
```
Никакого встроенного кэша или `tree`/`config` — кэш это **отдельный декоратор**.
Ключ Sled = `cid.to_bytes()`, значение = байты блока (плоский k/v, `blockstore.rs:223`).

---

## 2. Главный паттерн: Декоратор

Около 20 типов реализуют `BlockStore for X<S: BlockStore>` и делегируют внутрь:

| Декоратор | Забота | Источник |
|-----------|--------|----------|
| `CachedBlockStore<S>` | L1 LRU-кэш (порог допуска по размеру) | `cache.rs:222` |
| `BloomBlockStore<S>` | Bloom-фильтр для быстрого «нет» | `bloom.rs:547` |
| `DedupBlockStore<S>` | CDC-дедуп на уровне чанков (FastCDC) | `dedup.rs:371` |
| `CompressionBlockStore<S>` | Zstd/Lz4/Snappy | `compression.rs:489` |
| `EncryptedBlockStore<S>` | ChaCha20/AES-GCM | `encryption.rs:276` |
| `TtlBlockStore<S>` / `QuotaBlockStore<S>` | TTL-протухание / лимиты тенанта | `ttl.rs:291`, `quota.rs:470` |
| `MetricsBlockStore<S>` / `OtelBlockStore<S>` | метрики / трейсинг | `metrics.rs:337`, `otel.rs:66` |
| `TieredStore<H,C>` | горячий/холодный (2 внутренних стора) | `tiering.rs:515` |

**Реальный собранный стек** (`helpers.rs:136`, `build_full`):
```
BloomBlockStore  ─►  CachedBlockStore  ─►  SledBlockStore
   (внешний)            (L1 LRU)              (L2 диск)
```
`get` сначала бьёт по Bloom (быстрый отсев отсутствующих — `bloom.rs:560`), затем L1,
затем Sled.

> ⚠️ Старые вики рисовали другой порядок (`Cache→Tiering→Sled` или
> `Ttl→Quota→…→Sled`). **Реальность: Bloom — самый внешний.** Также `CircuitBreaker`
> (`circuit_breaker.rs:86`) — это standalone-утилита, а **не** декоратор стора, хотя
> вики намекают на обратное. Детали — [[11-RealityCheck]].

---

## 3. Доменные сервисы

### 3.1 Сборка мусора — три(!) разные реализации

| Реализация | Особенность | min_age | Источник |
|------------|-------------|---------|----------|
| `gc::GarbageCollector<S>` | DAG-aware mark-sweep по живому стору + `PinManager` | **нет** возрастного порога | `gc.rs:163,255` |
| `block_garbage_collector` | работает на собственном реестре | **есть** (деф. 300 c) | `block_garbage_collector.rs:619` |
| `garbage_collector::StorageGarbageCollector` | объектный граф | **нет** поля возраста | `garbage_collector.rs:118,140` |

**Инвариант I4** (пины/достижимое не удаляются) enforced во всех трёх: пины
засевают mark-множество. Но **возрастной порог `min_age`** реально применяется
только в `block_garbage_collector` и `gc_planner`. ⚠️ Это означает риск гонки в
`gc::GarbageCollector` и `StorageGarbageCollector`: блок, созданный микросекунду
назад с `ref_count==0`, может быть собран немедленно ([[11-RealityCheck]]).

### 3.2 Дедупликация
- **При записи** (store-level): `put` пропускает существующий CID (`blockstore.rs:319`).
  Безопасно благодаря контент-адресации (I5).
- **На уровне чанков** (CDC): `DedupBlockStore` режет блок FastCDC и хранит чанки с
  refcount (`dedup.rs:237,336`).

### 3.3 Тиринг (4 параллельные подсистемы — дублирование)
Только `TieredStore<H,C>` (`tiering.rs:515`) — настоящий `BlockStore`: `get`
авто-промоутит горячие блоки из cold в hot (`tiering.rs:526`). Остальные три
(`cold_storage`, `tier_manager`, `tier_migration_engine`) работают на метаданных-реестрах,
не двигая байты.

### 3.4 Целостность и валидация
Пять перекрывающихся сервисов. ⚠️ Важно: только `integrity_checker.rs`
**пересчитывает CID** из данных; остальные используют FNV/CRC/Adler — это контрольные
суммы, **не** криптогарантия. Истинная гарантия — `Block::verify` из ядра — на
горячем пути чтения **не вызывается**.

---

## 4. Ключевые инварианты Storage

| Инвариант | Статус |
|-----------|--------|
| Размер блока ∈ [1 B, 2 MiB] | enforced в `Block::new` (обходится `from_parts`) |
| Дедуп при записи (CID не перезаписывается) | enforced `blockstore.rs:319` |
| GC не удаляет пины/достижимое | enforced во всех 3 GC |
| GC возрастной порог `min_age` | enforced **только** в `block_garbage_collector` / `gc_planner` |
| Жёсткая квота отклоняет запись | enforced `quota.rs:262` |
| ⚠️ **Целостность при чтении** (байты хешируются в CID) | **НЕ enforced**: `get` → `from_parts(*cid, data)` без `verify()` (`blockstore.rs:350`) |

---

## 5. Долговечность (Sled)

- Каждый `put`/`delete` вызывает `db.flush_async()` (`blockstore.rs:241`) —
  синхронная per-op долговечность (корректно, но ограничивает throughput).
- `put_many`/`delete_many` используют `sled::Batch` для атомарности (`blockstore.rs:407`).
- `CompactionScheduler` (`compaction.rs:64`) — lock-free, триггер по объёму/простою.
- `SledSnapshotPinRegistry` — единственная вспом. структура, реально делящая Sled
  `Db` (crash-consistent пины снапшотов).

---

## 6. Интеграция

- **Потребляет из ядра**: `Block`, `Cid`, `Error`, `Result` (`traits.rs:4`) —
  узкая Conformist-зависимость.
- **Экспонирует вверх**: порт `BlockStore` (как `BlockStoreTrait`, `lib.rs:498`) +
  готовые сборки стека (`production_stack`, `ml_model_stack` и т.д. в `helpers.rs`).
- `StreamingBlockStore` (`streaming.rs:180`) добавляет range-чтение больших блоков.

> Storage также содержит чужеродный домену код: Raft-консенсус, мульти-ДЦ,
> tensor-VCS («Git для тензоров») — кандидаты на выделение в отдельные контексты
> ([[02-StrategicDesign]] §4).

---

## Что дальше?

- **Кто кладёт сюда блоки и забирает их** → [[09-ApplicationLayer]], [[10-DataFlows]]
- **Как блоки приходят от пиров** → [[08-TransportContext]]
- **Полный список расхождений с вики** → [[11-RealityCheck]]

**Связанные**: [[03-SharedKernel]] | [[08-TransportContext]] | [[09-ApplicationLayer]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-storage/src/{traits,blockstore,cache,bloom,dedup,gc,pinning,tiering,helpers}.rs`
