---
title: 12-MasterArchitecture
type: architecture
summary: Мастер-документ DDD — полный анализ кода Opus 4.8, контекстная карта, все 5 доменов, потоки данных, инварианты
tags: [ipfrs, ddd, architecture, overview, master]
source: cool-japan/IPFRS_ARCHITECTURE_MASTER.md
related: ["[[01-Overview]]", "[[03-BoundedContexts]]", "[[09-DataFlows]]", "[[10-Performance]]"]
read_time: 60 мин
updated: 2026-06-18
---

# IPFRS: Мастер-архитектура (Полный DDD-анализ)

> **Анализ Opus 4.8** — архитектура, обоснованная реальным кодом  
> **Версия**: 0.2.0 «Network Release»  
> **Аудитория**: архитекторы, senior-инженеры, системные проектировщики  
> **Статус**: ✅ Эталонный документ (rus)

---

## Содержание

1. [[#Стратегическая карта контекстов]]
2. [[#Общее ядро — ipfrs-core]]
3. [[#Домен Storage]]
4. [[#Домен Network]]
5. [[#Домен Semantic]]
6. [[#Домен Logic (TensorLogic)]]
7. [[#Домен Transport]]
8. [[#Фасад приложения — ipfrs Node]]
9. [[#Как данные реально движутся]]
10. [[#Ключевые инварианты и ограничения]]
11. [[#Дублирование репутации — автономность вместо DRY]]
12. [[#Event Sourcing vs мутация состояния]]
13. [[#Компромиссы производительности]]
14. [[#Точки миграции и расширения]]

---

## Краткое резюме

IPFRS — это **модульный монолит**, построенный как Cargo workspace из 12 крейтов, организованных по принципам **Domain-Driven Design** (DDD). Система объединяет распределённое хранилище с машинным интеллектом через контентную адресацию (CID). Каждый артефакт — блок, тензор, правило, доказательство — сводится к криптографическому хешу, который является *ubiquitous language*-токеном, пересекающим все границы контекстов.

**Ключевые выводы из анализа кода**:
1. **CID — универсальный граничный токен** — каждый anti-corruption layer сводится к «передай CID»
2. **Нейро-символьное слияние в TensorLogic** — отличительная черта, отсутствующая в традиционном IPFS
3. **Репутация намеренно дублируется** между Network и Transport для автономности (не DRY-нарушение)
4. **Storage реализован как стек декораторов** — элегантная абстракция с заменяемыми бэкендами
5. **Мутация состояния + журналы, а НЕ event sourcing** — журналы событий для наблюдаемости, не источник истины

---

## Стратегическая карта контекстов

### Структура workspace

```
crates/
├─ ipfrs-core              → SHARED KERNEL (реэкспортируется всем)
├─ ipfrs-storage           → Ограниченный контекст Storage
├─ ipfrs-network           → Ограниченный контекст Network
├─ ipfrs-semantic          → Ограниченный контекст Semantic
├─ ipfrs-tensorlogic       → Ограниченный контекст Logic
├─ ipfrs-transport         → Ограниченный контекст Transport
├─ ipfrs                   → ФАСАД ПРИЛОЖЕНИЯ (Node)
├─ ipfrs-interface         → ПРЕДСТАВЛЕНИЕ (gRPC/GraphQL/HTTP/WS)
├─ ipfrs-cli               → ПРЕДСТАВЛЕНИЕ (CLI)
└─ ipfrs-{wasm,nodejs,python} → АДАПТЕРЫ / ХОСТ-БИНДИНГИ
```

### Единый язык (Ubiquitous Language)

| Термин | Значение | Кто использует |
|--------|----------|----------------|
| **CID** | Content Identifier = hash(bytes), определяет идентичность | Все контексты |
| **Block** | Иммутабельная единица хранения, ≤2 МиБ | Storage, Transport |
| **Peer** | Удалённый узел с идентичностью (PeerId = hash(pubkey)) | Network, Transport |
| **Session** | Пакетный запрос блоков с машиной состояний | Transport |
| **Embedding** | Векторное представление смысла | Semantic |
| **Term/Rule/Fact** | Логическое IR с унификацией/выводом | Logic |
| **HNSW** | Иерархический индекс для k-NN поиска | Semantic |

### Отношения между контекстами (таксономия Evans/Vernon)

```
              СЛОЙ ПРЕДСТАВЛЕНИЯ И ХОСТА
       (CLI, gRPC, GraphQL, HTTP, WASM, Python, FFI)
                          │
                          ▼
           ┌──────────────────────────┐
           │  ФАСАД ПРИЛОЖЕНИЯ (Node) │
           │  Оркестрирует все домены │
           └──────┬──────────┬──┬──┬──┘
                  │          │  │  │
    ┌─────────────▼┐ ┌───────▼┐ │  │
    │   STORAGE    │ │NETWORK │ │  │
    │   Domain     │ │ Domain │ │  │
    └──────┬───────┘ └────┬───┘ │  │
           │              │     │  │
           │        ┌─────┴─────▼──▼──────────┐
           │        │     TRANSPORT Domain     │
           │        │  (использует Storage+    │
           │        │         Network)         │
           │        └───────────┬──────────────┘
           │                    │
           ▼                    ▼
    ┌──────┴──────────────────────────────┐
    │  SEMANTIC Domain  │  LOGIC Domain   │
    │  (HNSW)           │  (TensorLogic)  │
    └─────────────┬─────────────────┬─────┘
                  │                 │
          (все зависят от SHARED KERNEL)
                  │                 │
    ┌─────────────┴─────────────────▼─────┐
    │  ipfrs-core: Cid, Block, Ipld, ...  │
    │       (импортируется везде)         │
    └─────────────────────────────────────┘
```

**Ключевые паттерны отношений**:

| От → До | Паттерн | Реализация |
|---------|---------|------------|
| Все → Core | **Shared Kernel** | `Cid`, `Block`, `Result` импортируются везде |
| Все → Storage | **Conformist / OHS** | Трейт `BlockStore` — опубликованный интерфейс |
| Transport → Storage | **Customer/Supplier + ACL** | Transport вызывает `store.get/put` только через трейт |
| Transport → Network | **Customer/Supplier** | Transport реплицирует peer-скоринг (не делит) |
| Все → libp2p | **Anti-Corruption Layer** | Network оборачивает `libp2p::PeerId` в доменный `String` VO |
| Logic → Storage | **Published Language (IPLD)** | Правила сериализованы как контентно-адресованные блоки |
| Presentation → App | **Open Host Service** | gRPC/GraphQL/CLI направляются через фасад `Node` |
| Bindings → App | **Anti-Corruption Layer** | FFI/Python используют непрозрачные `#[repr(C)]` указатели |

---

## Общее ядро — ipfrs-core

**Ответственность**: предоставить универсальные доменные примитивы, с которыми согласны все контексты.

### Объекты-значения (Value Objects)

#### `Cid` — Content Identifier

```rust
pub use ::cid::Cid;  // Реэкспорт из внешнего крейта

pub enum HashAlgorithm {
    Sha256, Sha512, Sha3_256, Sha3_512,
    Blake2b256, Blake2b512, Blake2s256, Blake3,
}

// CID вычисляется через:
// cid = Cid::new(
//     Version::V1,
//     Codec::Raw,
//     hash_algorithm.digest(bytes)
// )
//
// Инвариант: CID ИММУТАБЕЛЕН и ДЕТЕРМИНИРОВАН
// hash(data) == cid  (проверяется при каждом чтении)
```

**Почему это Value Object**:
- Идентичность определяется исключительно хешем содержимого
- Два CID равны тогда и только тогда, когда содержимое идентично
- Нет мутабельного состояния
- Используется как ключ в мапах, аргумент функций

#### `Block` — Иммутабельная единица хранения

```rust
pub struct Block {
    cid: Cid,
    data: Bytes,
    metadata: BlockMetadata,
}

pub struct BlockMetadata {
    size: u64,
    created_at: Instant,
    access_count: u64,
}

// Инварианты:
// 1. data.len() <= 2 МиБ (настраивается)
// 2. cid == hash(data)
// 3. data НИКОГДА не мутируется после создания
```

**Жизненный цикл**:
1. Создание: `Block::new(bytes)` → вычисляет CID, оборачивает в Block
2. Сохранение: `storage.put(&block)` → иммутабельная запись в Sled
3. Извлечение: `storage.get(&cid)` → проверка `hash(retrieved) == cid`, возврат или ошибка
4. GC: если не закреплён и устарел → удаление

#### `Ipld` — InterPlanetary Linked Data

```rust
pub enum Ipld {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>),
    Link(Cid),  // ← Ссылка на другой блок
}

// Каноническое кодирование через DAG-CBOR гарантирует:
// - Детерминированную сериализацию (BTreeMap, отсортированные ключи)
// - Предсказуемость CID(ipld)
// - Возможность контентно-адресованных графов
```

### Сущности (Entities)

#### `TensorBlock` — Метаданные и данные тензора

```rust
pub struct TensorBlock {
    shape: Vec<u64>,           // например, [1024, 768]
    dtype: TensorDtype,        // f32, f64, i32 и т.д.
    data: Bytes,               // Сырые байты тензора
    metadata: TensorMetadata,  // квантование, информация о сжатии
}

pub enum TensorDtype {
    Float32, Float64, Int32, Int64,
    BFloat16, Float16, Int8, UInt8,
}

// Используется Semantic (embeddings) и Logic (вычислительные графы)
```

> Связанные статьи: [[04-StorageDomain]] | [[06-SemanticDomain]] | [[07-LogicDomain]]

---

## Домен Storage

### Агрегат-корень: Block

**Инварианты**:
- CID совпадает с хешем данных: `hash(data) == cid`
- Данные иммутабельны: нет методов `&mut data`
- Размер ≤ 2 МиБ
- Создаётся и сохраняется атомарно

**Машина состояний жизненного цикла**:

```
Входные данные пользователя (bytes)
    ↓
[Создание]  → вычислить CID, создать Block
    ↓
[Сохранение] → put в Sled + LRU-кэш
    ↓
[Использование] → извлечение по CID с верификацией хеша
    ↓
[Сборка мусора] ← если не закреплён и устарел
    ↓
[Удалён] (если не pinned)
```

### Domain Services

#### Трейт `BlockStore` (опубликованный порт)

```rust
#[async_trait]
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    async fn delete(&self, cid: &Cid) -> Result<()>;
    async fn all_cids(&self) -> Result<Vec<Cid>>;
}

// Реализации:
// - SledBlockStore (по умолчанию, embedded)
// - ParityDBStore (высокая производительность, оптимизация для блокчейна)
// - (Другие — подключаемые)
```

#### Паттерн стека декораторов

```
┌─────────────────────────────────┐
│  Код пользователя (App Layer)   │
└──────────────┬──────────────────┘
               │ (вызов)
┌──────────────▼─────────────────────────┐
│ Декоратор: CorruptionRepair            │
│  • Верификация CID при чтении          │
│  • Обнаружение и восстановление        │
└──────────────┬─────────────────────────┘
               │ (делегирует)
┌──────────────▼─────────────────────────┐
│ Декоратор: LRU Cache Layer             │
│  • Оптимизация горячего пути (99% hit) │
│  • Async-safe concurrent access        │
└──────────────┬─────────────────────────┘
               │ (делегирует)
┌──────────────▼─────────────────────────┐
│ Декоратор: Tiering / Hot-Cold Split    │
│  • Часто используемые → memory/SSD     │
│  • Старые/холодные → cold storage      │
└──────────────┬─────────────────────────┘
               │ (делегирует)
┌──────────────▼─────────────────────────┐
│ Реализация: SledBlockStore             │
│  • Embedded B+ tree база данных        │
│  • ACID-транзакции                     │
└────────────────────────────────────────┘
```

**Почему декораторы?**
- Каждая задача (кэширование, тиеринг, восстановление) тестируется независимо
- Легко менять реализации
- Следует принципу Open-Closed

### Сборка мусора (GC)

**Domain Event**: `BlockUnpinned(cid: Cid)`  
**Процесс GC**:
1. Сканировать все блоки
2. Проверить статус pin (из агрегата Pin)
3. Если не закреплён + создан > TTL → удалить
4. Эмитировать `BlockDeletedEvent`

> Подробности: [[04-StorageDomain]]

---

## Домен Network

### Агрегат-корень: Peer

```rust
pub struct Peer {
    peer_id: PeerId,              // = hash(public_key)
    multiaddrs: Vec<Multiaddr>,   // Как достичь
    reputation: ReputationScore,  // Скоринг для маршрутизации
    known_blocks: HashSet<Cid>,   // Что, по слухам, есть у пира
    last_seen: Instant,
    connection_state: ConnState,
}

pub enum ConnState {
    Idle,
    Connecting,
    Connected { since: Instant },
    Active { session: SessionId },
}

// Инвариант: PeerId = hash(public_key) — иммутабельная идентичность
```

### Скоринг репутации (позиция Network)

```rust
// Граф-ориентированная EMA (Exponentially Moving Average)
reputation = (success_count × recent_weight)
           / (total_interactions + epsilon)

// Распад: старые взаимодействия весят меньше
recent_weight = exp(-age_seconds / HALF_LIFE)

// Граф доверия: скоринг пира влияет на тех, кто его рекомендовал
```

### Domain Services

#### DHT (Distributed Hash Table) — Kademlia

```
Операция: dht.find_providers(cid: Cid) -> [PeerId]

1. Хешировать CID в 256-битный ключ
2. Итеративно опрашивать соседей по XOR-расстоянию:
   - Начало: спросить bootstrap-пиры
   - Они отвечают: "попробуй этих, они ближе"
   - Повторять до сходимости
3. Вернуть top-k пиров (обычно k=20)

Инвариант: Все узлы согласованы по XOR-метрике
```

#### Маршрутизация по содержимому

```
Фаза объявления:
  Storage: "Я только что сохранил блок CID"
  Network: dht.put_provider(cid, my_peer_id)
  DHT: хранит (CID → [my_peer_id]) на k репликах

Фаза обнаружения:
  Пользователь: "У кого есть CID?"
  Network: dht.find_providers(CID)
  DHT: возвращает [peer1, peer2, peer3, ...]
```

> Подробности: [[05-NetworkDomain]]

---

## Домен Semantic

### Доменная модель: HNSW-индекс

```rust
pub struct HnswIndex<T> {
    vectors: Vec<T>,            // Сохранённые векторы (индексы важны)
    layers: Vec<Layer<T>>,      // Иерархическая граф-структура
    entry_point: usize,         // Корень для поиска
    max_connections: usize,     // Параметр M (по умолчанию: 16)
    ef_construction: usize,     // Радиус поиска при вставке
    ef_search: usize,           // Радиус поиска при запросе
}

struct Layer<T> {
    nodes: Vec<Node<T>>,        // Узлы этого уровня
    neighbors: Vec<Vec<usize>>, // Списки смежности (рёбра)
}

// Инварианты:
// 1. Верхние слои разреженнее (меньше узлов)
// 2. Количество связей ≤ max_connections
// 3. Точка входа всегда присутствует
```

### Алгоритм поиска

```
query(q: Vec<f32>, k: usize) -> [Cid] {
    // Послойный спуск (алгоритм HNSW)
    let mut nearest = [entry_point];
    
    for layer in layers.iter().rev() {  // сверху вниз
        nearest = expand_search(nearest, q, ef_search, layer);
    }
    
    // Вернуть k ближайших из нижнего слоя
    return nearest.top_k(k, |v| distance(q, v));
}

// Расстояние: cosine, L2, Jaccard (настраивается)
// Аппроксимация: ~99% recall vs точный k-NN за 1–10мс на 100k векторов
```

### Кэширование запросов

```rust
pub struct QueryCache {
    cache: Arc<DashMap<EmbeddingHash, Vec<SearchResult>>>,
    config: CacheConfig,  // maxSize, ttl, ...
}

// Hit rate: ~85% при типичной нагрузке
// Почему не 100%? Запросы немного меняются (переобучение модели и т.д.)
```

> Подробности: [[06-SemanticDomain]]

---

## Домен Logic (TensorLogic)

### Доменная модель: Logic IR

```rust
pub enum Term {
    Const(Constant),              // "alice", 42, true
    Var(String),                  // "X", "Y"
    Compound(String, Vec<Term>),  // f(X, Y) = "f" с аргументами
}

pub struct Predicate {
    name: String,
    args: Vec<Term>,
}

// Пример: parent(alice, bob)
//   = Predicate { name: "parent", 
//                  args: [Const("alice"), Const("bob")] }

pub struct Rule {
    head: Predicate,
    body: Vec<Predicate>,
    // Пример: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
}

pub struct Fact {
    predicate: Predicate,  // Не должна содержать переменных
}
```

### Унификация и вывод

```
unify(goal: Predicate, fact: Fact) -> Option<Substitution> {
    // Попытка сопоставления с образцом
    // Возвращает привязки переменных при успехе
    
    // Пример:
    // goal = parent(X, bob)
    // fact = parent(alice, bob)
    // → возвращает {X: alice}
}

infer(goal: Predicate, rules: [Rule], facts: [Fact]) 
    -> [Substitution] {
    // Обратный вывод (SLD resolution)
    
    // 1. Попытаться сопоставить цель с фактами
    // 2. Попытаться унифицировать с каждой головой правила
    // 3. Для совпавших правил рекурсивно вывести каждую подцель
    // 4. Накопить решения
    
    // Предел глубины: 1000 (предотвращение бесконечных циклов)
}
```

### Нейро-символьное слияние

**Отличительная черта** (отсутствует в традиционном IPFS):

```rust
pub enum InferenceMode {
    Symbolic,          // Чистая логика (обратный вывод)
    Hybrid(f32),       // Символьная + векторный fallback
                       // fallback_threshold = 0.7
    Neural(Vec<f32>),  // Пространство векторных embeddings (семантика)
}

pub struct ComputationGraph {
    nodes: Vec<Operation>,
    edges: Vec<(NodeId, NodeId)>,
    autograd: bool,    // Отслеживать градиенты для обучения
}

pub enum Operation {
    Unify { term1: Term, term2: Term },
    Embed { predicate: Predicate },         // Запрос к семантическому индексу
    ConvexCombination { w1: f32, w2: f32 }, // Смешение символьного + нейронного
}

// Пример: "Докажи father(bob, X)" с fallback на семантическое сходство
// 1. Попробовать чистую логику (обратный вывод)
// 2. Если нет решений, встроить "father" и искать семантически
// 3. Вернуть top-k похожих фактов
```

> Подробности: [[07-LogicDomain]]

---

## Домен Transport

### Агрегат-корень: BlockExchangeSession

```rust
pub struct BlockExchangeSession {
    session_id: SessionId,          // Уникален для каждого пакета запросов
    requested_blocks: Vec<Cid>,     // Что хочет пользователь
    received_blocks: HashSet<Cid>,  // Что пришло на данный момент
    want_list: WantList,            // Приоритетная очередь
    peer_selection: PeerScores,     // Ранжирование по репутации
    state: SessionState,            // Машина состояний
    created_at: Instant,
}

pub enum SessionState {
    Created,           // Только инициализирован
    Active,            // Получение в процессе
    Paused,            // Временно остановлен
    Completed,         // Получены все блоки
    Failed(String),    // Отказ
}

// Инвариант:
// received_blocks ⊆ requested_blocks
// переходы состояний только по допустимым путям
```

### WantList и приоритетная очередь

```rust
pub struct WantList {
    entries: Vec<WantEntry>,
    priority_queue: BinaryHeap<(Priority, Cid)>,
}

pub struct WantEntry {
    cid: Cid,
    priority: i32,      // Выше = срочнее (0–100)
    send_dont_have: bool,
    cancel: bool,
}

// Примеры приоритетов:
// первые чанки тензора: 100 (нужны первыми)
// средние чанки: 50
// хвостовые чанки: 10

// Bitswap-сообщения пиру:
// "Хочу CID#1 (приоритет 100), CID#2 (50), CID#3 (10)"
// Пир отдаёт приоритет CID#1
```

### Peer-скоринг (позиция Transport)

**Отличается от скоринга Network:**

```rust
// Network оценивает: "Надёжен ли этот пир для долгосрочной маршрутизации?"
// Transport оценивает: "Будет ли этот пир быстрым ДЛЯ ЭТОЙ СЕССИИ?"

transport_score(peer: Peer) = 
    (success_rate) * 
    (1.0 / (latency_ms + 1)) *  // Более быстрые пиры ранжируются выше
    (availability_factor) *      // Connected > not connected
    (connection_age_factor)      // Новые соединения лучше

// Скоринг Transport — per-session; Network — персистентный
```

> Подробности: [[08-TransportDomain]]

---

## Фасад приложения — ipfrs Node

### Node — центральный оркестратор

```rust
pub struct Node {
    storage: Arc<dyn BlockStore>,
    network: Arc<NetworkNode>,
    semantic: Arc<SemanticIndex>,
    tensorlogic: Arc<KnowledgeBase>,
    auth: Arc<AuthManager>,
    tls: Arc<TlsManager>,
    pin_manager: Arc<PinManager>,
    metrics: Arc<MetricsCollector>,
}

impl Node {
    pub async fn add_file(&self, path: Path) -> Result<Cid> {
        // 1. Читаем файл
        // 2. Режем на чанки
        // 3. Для каждого чанка:
        //    - Storage: put block
        //    - Network: announce(cid)
        //    - Semantic: index(cid, embedding) если включено
        // 4. Возвращаем корневой CID
    }
    
    pub async fn get_file(&self, cid: Cid) -> Result<Bytes> {
        // 1. Storage: попытка локального get (кэш/диск)
        // 2. Промах: Network: find_providers(cid)
        // 3. Transport: запрос у лучшего пира (на основе сессии)
        // 4. Storage: сохранить полученный блок
        // 5. Вернуть байты
    }
    
    pub async fn search_semantic(&self, query: String, k: usize) 
        -> Result<Vec<SearchResult>> {
        // 1. ML-модель: embed(query)
        // 2. Semantic: hnsw_search(embedding, k)
        // 3. Storage: загрузить метаданные для top-k CID
        // 4. Вернуть с заголовками/превью
    }
    
    pub async fn query_logic(&self, goal: Predicate)
        -> Result<Vec<Substitution>> {
        // 1. Logic: infer(goal, rules, facts)
        // 2. Если нет решений + hybrid включён:
        //    - Semantic: fallback к векторному сходству
        // 3. Вернуть решения
    }
}
```

---

## Как данные реально движутся

### Поток «Добавить файл»

```
Пользователь: ipfrs add document.pdf (100 МБ)
    ↓
CLI (Layer 0)
    ↓
Node.add_file() (Layer 1: Application)
    ↓
read file(100 МБ) → Bytes
    ↓
Chunker.chunk() → [Block1, Block2, ..., Block391] (по 256 КБ)
    ↓
ДЛЯ КАЖДОГО блока:
    ├─ Storage.put(block)
    │   ├─ Вычислить CID
    │   ├─ Верификация: hash(data) == CID
    │   ├─ Persist в Sled
    │   ├─ Обновить LRU-кэш
    │   └─ Emit: BlockAddedEvent
    │
    ├─ Network.announce(cid)
    │   ├─ DHT.put_provider(cid, my_peer_id)
    │   ├─ Сообщить подключённым пирам: "У меня есть cid"
    │   └─ Emit: BlockAnnouncedEvent
    │
    └─ [ЕСЛИ semantic включён] Semantic.index(cid, embedding)
        ├─ Извлечь текст из блока
        ├─ ML-модель: embed(text) → [0.1, -0.2, 0.3, ...]
        ├─ HNSW: insert(cid, embedding)
        └─ Обновить кэш запросов
    ↓
Пользователю: "Добавлено! Root CID = bafybeig..."

Время:
  Чтение файла: ~50мс
  Нарезка (параллельно, 8 ядер): ~150мс
  Storage (391 блок × 50мкс): ~20мс
  Network announce (async): ~100мс
  Semantic индексирование (если включено): ~500мс
  ─────────
  Итого: ~900мс (с semantic) или ~300мс (без)
```

### Поток «Получить файл»

```
Пользователь: ipfrs get bafybeig...
    ↓
Node.get_file(cid)
    ↓
[БЫСТРЫЙ ЛОКАЛЬНЫЙ ПУТЬ]
    Storage.get(cid)
    ├─ Проверить LRU-кэш: попадание? (30мкс) → вернуть
    ├─ Проверить Sled DB: попадание? (100мкс) → кэш & вернуть
    └─ Промах: продолжить в сеть...
    ↓
[СЕТЕВОЙ ПУТЬ]
    Network.find_providers(cid)
    ├─ DHT.lookup(cid) → итеративный XOR-поиск (150–300мс)
    ├─ Возврат: [PeerId1, PeerId2, PeerId3, ...]
    └─ Emit: ProvidersFoundEvent
    ↓
[TRANSPORT СЕССИЯ]
    Transport.create_session([cid])
    ├─ Скоринг пиров: reputation_score(peer) для каждого
    ├─ Выбор лучшего пира (высший score)
    ├─ Session state: Active
    └─ Emit: SessionCreatedEvent
    ↓
[BITSWAP ОБМЕН]
    Отправить Bitswap-сообщение: Want(cid=bafybeig, priority=100)
    ↓ (50–100мс RTT сети)
    
    Удалённый пир:
    ├─ Storage.get(cid) на своей машине
    ├─ Отправить Block(cid, data)
    └─ Emit: BlockSentEvent
    ↓ (50–100мс RTT обратно)
    
    Получение блока:
    ├─ Верификация: hash(block.data) == cid
    ├─ Storage.put(block)
    ├─ Обновить репутацию пира (success++)
    ├─ Отметить прогресс сессии (received_blocks.insert(cid))
    ├─ Session state: Completed (если получены все)
    └─ Emit: SessionCompletedEvent
    ↓
Пользователю: [байты файла]

Время:
  Попадание в локальный кэш: ~30мкс
  Попадание на локальный диск: ~200мкс
  Сетевой путь: 200–1000мс (зависит от DHT + RTT)
```

> Подробности всех 4 сценариев: [[09-DataFlows]]

---

## Ключевые инварианты и ограничения

| Инвариант | Домен | Следствие |
|-----------|-------|-----------|
| `hash(data) == cid` | Storage | Каждое чтение хеш-верифицируется; повреждения обнаруживаются |
| `PeerId = hash(public_key)` | Network | Идентичность пира неизменна |
| `received_blocks ⊆ requested_blocks` | Transport | Сессия завершается только когда всё получено |
| `0.0 ≤ similarity_score ≤ 1.0` | Semantic | Нормализованная метрика расстояния |
| `rules are consistent` | Logic | Нет противоречий (проверяется при assert) |
| `FIFO per-peer messages` | Transport | Bitswap-сообщения одному пиру упорядочены |
| `pinned blocks exempt from GC` | Storage | Пользователь может защитить важный контент |

---

## Дублирование репутации — автономность вместо DRY

**Стратегическое решение**: Network и Transport поддерживают **отдельные модели peer-скоринга**.

```
Контекст Network (долгосрочное доверие маршрутизации):
  score = (success_in_past_year × recency_decay)
        / total_lookups_requested
  → Медленно меняется; отражает историческое поведение
  → Используется для: "Надёжен ли этот пир для обнаружения контента?"

Контекст Transport (производительность текущей сессии):
  score = (success_in_session)
        * (1.0 / latency_ms)
        * (connected_now ? 1.0 : 0.1)
  → Быстро обновляется; отражает текущие условия
  → Используется для: "У какого пира запросить этот блок СЕЙЧАС?"
```

**Почему дублировать, а не делить?**
- **Автономность**: каждый контекст принимает решения по скорингу независимо
- **Устойчивость**: сбой Network не влияет на Transport (и наоборот)
- **Эффективность**: Transport может использовать другие метрики (задержка важна здесь, а не долгосрочная история)
- **Тестируемость**: каждый скорер можно мокать изолированно

---

## Event Sourcing vs мутация состояния

**Вопреки некоторой DDD-ортодоксии**, IPFRS использует **мутацию состояния + журналы аудита**, а НЕ event sourcing.

```
Состояние: Block { cid, data, metadata }
           ├─ Прямая мутация: metadata.access_count++
           │
Аудит: Системные логи: "block.get(cid) succeeded" → Метрики
```

**Почему не event sourcing?**
1. **Инвариант контентной адресации**: идентичность Block никогда не меняется (CID = hash(data))
2. **Иммутабельное хранилище**: блоки никогда не мутируют после создания
3. **Наблюдаемость достаточна**: события используются только для метрик, не для реконструкции состояния
4. **Простота**: проще рассуждать (текущее состояние = авторитетное)

---

## Компромиссы производительности

| Операция | Время | Узкое место | Компромисс |
|----------|-------|-------------|------------|
| Block PUT | ~50мкс | Задержка SSD I/O | Скорость vs. надёжность; Sled гарантирует ACID |
| Block GET (cache hit) | ~30мкс | CPU + поиск в L3 кэше | Размер кэша vs. использование памяти |
| HNSW insert | ~100мкс | Обновление графа HNSW | Точность (~99% vs. 100%) vs. задержка |
| HNSW k-NN search | 1–10мс | Обход графа | Больше ef_search = медленнее, но точнее |
| DHT lookup | 150–300мс | RTT сети × хопы | Параллелизм (α=3) vs. общее число запросов |
| Bitswap fetch | 200–1000мс | RTT сети + ответ пира | Приоритетная очередь гарантирует приход критичных блоков первыми |

> Подробная таблица метрик: [[10-Performance]]

---

## Точки миграции и расширения

### Как заменить бэкенд Storage?

```rust
// Определить новый бэкенд
pub struct RocksDBBlockStore { /* ... */ }

impl BlockStore for RocksDBBlockStore {
    async fn put(&self, block: &Block) -> Result<()> { /* ... */ }
    async fn get(&self, cid: &Cid) -> Result<Option<Block>> { /* ... */ }
}

// Обновить конфиг Node
let mut config = NodeConfig::default();
config.blockstore = Arc::new(RocksDBBlockStore::new());

let mut node = Node::new(config)?;
// Всё остальное работает без изменений!
```

### Как добавить новый доменный контекст?

1. Создать новый крейт: `crates/ipfrs-{domain}/`
2. Определить агрегаты (с инвариантами)
3. Реализовать Domain Services
4. Добавить трейт в Application Facade
5. Обновить карту контекстов

---

## Заключение

IPFRS — это **хорошо выстроенный, модульный монолит**, в котором:
- **CID — lingua franca** — всё межконтекстное общение сводится к «передай хеш»
- **Пять автономных ограниченных контекстов** принимают архитектурные решения независимо
- **Намеренное дублирование** (репутация, журналирование событий) выбрано вместо преждевременного обобщения
- **Иммутабельность содержимого** движет дизайном — блоки не меняются после создания
- **Мутация состояния + журналы аудита** заменяют event sourcing (проще, достаточно)
- **Стек декораторов** разделяет задачи (кэш, восстановление, тиеринг) без связанности
- **Нейро-символьное слияние** в Logic/TensorLogic — отличительный AI-дифференциатор

Эта архитектура обеспечивает:
- ✅ Независимое масштабирование (Storage → ПБ; Semantic → 1М векторов; Transport → 1K пиров)
- ✅ Лёгкое тестирование (мокать любой трейт)
- ✅ Расширяемость (менять реализации, добавлять контексты)
- ✅ Отлаживаемость (чёткие доменные границы, следы аудита)

---

## Что дальше?

→ [[03-BoundedContexts]] — краткий обзор всех 5 контекстов  
→ [[04-StorageDomain]] — глубокое погружение в Storage  
→ [[05-NetworkDomain]] — Kademlia DHT и репутация  
→ [[06-SemanticDomain]] — HNSW и семантический поиск  
→ [[07-LogicDomain]] — обратный вывод и нейро-символьное слияние  
→ [[08-TransportDomain]] — Bitswap и управление сессиями  
→ [[09-DataFlows]] — полные сценарии потоков данных  
→ [[10-Performance]] — метрики и узкие места  

---

**Связанные**: [[01-Overview]] | [[02-ArchitectureStack]] | [[03-BoundedContexts]] | [[09-DataFlows]] | [[10-Performance]]

---

*Создан: перевод IPFRS_ARCHITECTURE_MASTER.md (Opus 4.8) → русский*  
*Дата: 2026-06-18. Версия кодовой базы: 0.2.0.*
