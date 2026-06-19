---
title: 03-SharedKernel
type: domain
summary: Shared Kernel (ipfrs-core) — Cid как универсальный токен границы, Block-агрегат, IPLD, DAG, хеши и кодеки
tags: [ipfrs, ddd, shared-kernel, cid, block, ipld]
source: ipfrs_source/crates/ipfrs-core/src/
related: ["[[02-StrategicDesign]]", "[[04-StorageContext]]", "[[10-DataFlows]]"]
read_time: 13 мин
updated: 2026-06-19
---

# Shared Kernel — `ipfrs-core`

**Краткое резюме**: `ipfrs-core` (≈17,6K строк, 30 модулей) — это набор типов, на
которых сходятся **все** контексты. Центральный из них — `Cid`, «универсальный
токен границы»: им говорят Storage, Network, Transport, Semantic и TensorLogic, не
разделяя реализацию друг друга.

---

## 1. Почему это Shared Kernel

`Cid` обладает тремя свойствами, делающими его чистой границей между контекстами:

1. **Значимостная семантика** — `Copy + Eq + Hash + Ord` (из внешнего крейта `::cid`,
   `cid.rs:6`). CID пересекает границы контекстов без согласования владения/жизненного
   цикла.
2. **Самоописание и самопроверка** — `(version, codec, multihash)`: любой контекст
   независимо проверяет `cid == hash(data)` (`block.rs:117`). Контексты доверяют
   *данным*, а не *друг другу*.
3. **Детерминизм** — `Ipld::Map` на `BTreeMap` (`ipld.rs:35`) + каноничный DAG-CBOR
   дают одинаковый CID на любом узле. Это предусловие распределённой дедупликации.

Ядро намеренно **минимально**: `Cid`, `Block`, `Ipld`, `Error`/`Result`, сервисы
хеша/кодека, примитивы DAG. Всё контекст-специфичное живёт вне `ipfrs-core`.

---

## 2. Корневой агрегат: `Block`

```rust
// block.rs:57 — оба поля приватны, мутаций нет
pub struct Block { cid: Cid, data: Bytes }

pub fn new(data: Bytes) -> Result<Self> {        // block.rs:70
    Self::validate_size(data.len())?;            // I2: 1 ≤ len ≤ 2 MiB (block.rs:77)
    let cid = CidBuilder::new().build(&data)?;   // I1: cid = H(data)
    Ok(Self { cid, data })
}
pub fn from_parts(cid: Cid, data: Bytes) -> Self // block.rs:94 — БЕЗ проверки и без валидации
pub fn verify(&self) -> Result<bool>             // block.rs:117 — пересчитать CID и сравнить
```

- **Идентичность = CID**: `PartialEq/Eq/Hash/Ord` определены **только по CID**
  (`block.rs:443`). Два блока равны ⟺ равны их CID.
- **Zero-copy**: `data: Bytes` → `Clone` за O(1) (refcount); `slice()`/`as_bytes()`
  не копируют (`block.rs:151`).
- **Лазейка `from_parts`** (`block.rs:94`): строит блок **без** проверки и валидации
  размера — используется для доверенной регидратации с диска/провода. Именно её
  применяет Storage на каждом чтении (см. ⚠️ I9 в [[04-StorageContext]]).

> ⚠️ **Тонкость `verify()`**: он всегда пересчитывает CID **дефолтным** билдером
> (SHA-256). Блок, созданный нестандартным алгоритмом (BLAKE3 через `BlockBuilder`),
> провалит `verify()`, хотя корректен. Это реальный «footgun» (`block.rs:118`).

---

## 3. Ключевые value objects

| Тип | Суть | Источник |
|-----|------|----------|
| `Cid` | Внешний тип, расширен трейтом `CidExt` (`to_v0/to_v1`, `codec_code`, `hash_algorithm_name`). Инвариант: CIDv0 ⟹ SHA-256 + DAG-PB. | `cid.rs:6,381,439` |
| `SerializableCid` | Newtype-обёртка, сериализующая CID как **строку** (а не бинарно). Встраивается в `Ipld::Link`, `DagLink`, `BlockMetadata`. | `cid.rs:519` |
| `HashAlgorithm` | 8 вариантов: Sha256(деф.), Sha512, Sha3_256/512, Blake2b256/512, Blake2s256, Blake3. | `cid.rs:12` |
| `Ipld` | 9 вариантов: Null, Bool, Integer(i128), Float(f64), String, Bytes, List, Map(**BTreeMap**), Link. Только `PartialEq` (нет `Eq` из-за f64). | `ipld.rs:19` |
| `TensorShape` / `TensorDtype` | Форма (`dims: Vec<usize>`) и dtype (F32,F16,F64,I8,I32,I64,U8,U32,Bool). | `tensor.rs:88,31` |
| `Priority` | Enum Low/Normal/High/Critical (4 уровня, `Ord`). | `types.rs:15` |

---

## 4. Доменные сервисы и фабрики

| Сервис / фабрика | Роль | Источник |
|------------------|------|----------|
| `CidBuilder` | Фабрика CID. Деф.: V1 / raw(0x55) / SHA-256. `build()` хеширует данные. | `cid.rs:267,337` |
| `BlockBuilder` | Фабрика блоков с настраиваемым алгоритмом/кодеком. | `block.rs:347` |
| `HashEngine` + 8 движков | Подключаемое хеширование с runtime SIMD-диспетчеризацией (AVX2/NEON в `Sha256Engine`). | `hash.rs:37,148` |
| `Codec` + `CodecRegistry` | `encode/decode` IPLD↔байты; реестр с заменой при коллизии, расширяемый в рантайме. | `codec_registry.rs:32,124` |
| `Chunker` / `RabinChunker` | Разбиение файлов: фиксированное или content-defined (rolling hash). Фан-аут ≤ 174 ссылок/узел. | `chunking.rs:459,194` |
| `DagBuilder` | Сборка DAG-узлов (директории/файлы) в DAG-CBOR. | `chunking.rs:620` |
| `CarWriter`/`CarReader` | Архивация DAG. **Внимание**: формат не чистый CARv1 — между CID и данными есть 1 байт флага сжатия (Zstd/LZ4). | `car.rs:334,362` |

---

## 5. Сущности DAG

- `DagLink { cid, size, name }` — типизированное ребро с размером поддерева
  (`chunking.rs:329`).
- `DagNode { links, total_size, data }` — лист несёт `data`, промежуточный узел —
  `links` + суммарный размер (`chunking.rs:361`).
- `ContentManifest` — агрегат мульти-файлового добавления; **детерминированная
  идентичность**: `manifest_id` = FNV-1a от отсортированных CID, `root_cid` = Merkle-корень
  (`manifest.rs:244,280`).
- `MerkleTree` — **внимание**: хеши здесь FNV-1a, **не криптографические**
  (`manifest.rs:136`); это быстрый контрольный индекс, не tamper-proof.

---

## 6. Сводка инвариантов ядра

| Инвариант | Где |
|-----------|-----|
| `cid = hash(codec, data)` при создании | `cid.rs:337`, `block.rs:70` |
| Размер блока ∈ [1 B, 2 MiB] | `block.rs:77` (обходится `from_parts`) |
| CIDv0 ⟹ SHA-256 + DAG-PB | `cid.rs:341` |
| Каноничный IPLD (отсортированные ключи map) | `ipld.rs:35,339` |
| Tensor: `len(data) = Π(dims) × dtype.size` | `tensor.rs:233` |
| Manifest: идентичность выводится, не задаётся | `manifest.rs:280` |
| CAR версия == 1 | `car.rs:123` |

---

## 7. Уникальная деталь: тензоры как контент-адресуемые блоки

`TensorBlock { block, metadata }` (`tensor.rs:192`) даёт ML-весам те же гарантии
контент-адресации и дедупликации, что и файлам. Вместе с `arrow.rs`/`safetensors.rs`
это и есть «…with ML» прямо в ядре — мост, через который TensorLogic
([[07-TensorLogicContext]]) адресует тензоры по CID.

---

## Что дальше?

- **Как блоки хранятся** → [[04-StorageContext]]
- **Как блоки путешествуют** → [[08-TransportContext]]
- **Сквозной путь ADD** → [[10-DataFlows]]

> ⚠️ Существующие вики (`Wiki_Arch_GLM/02-SharedKernel.md`) содержат ≥12 фактических
> ошибок в сигнатурах ядра (`CidBuilder::build`, `from_parts`, `TensorBlock`,
> `Error`, коды хешей, формат CAR). Полный список — [[11-RealityCheck]].

**Связанные**: [[02-StrategicDesign]] | [[04-StorageContext]] | [[07-TensorLogicContext]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-core/src/{cid,block,ipld,hash,codec_registry,chunking,manifest,tensor,dag,car,error}.rs`
