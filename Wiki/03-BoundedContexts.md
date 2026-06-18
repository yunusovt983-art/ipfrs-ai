# 5 Bounded Contexts: Пять независимых миров

**Краткое резюме**: IPFRS построена на Domain-Driven Design. Пять bounded contexts (доменов) полностью независимы и взаимодействуют только через чистые интерфейсы. Каждый имеет свой язык, концепции и модели.

---

## Таблица контекстов

| Контекст | Язык | Вопрос | Главная задача |
|----------|------|--------|-----------------|
| **Storage** | Block, CID, DAG, IPLD | "Что у нас есть?" | Хранить блоки, верифицировать целостность |
| **Network** | Peer, DHT, Multiaddr | "Где находятся узлы?" | Находить peer'ов, маршрутизировать контент |
| **Semantic** | Embedding, HNSW, Vector | "Что это означает?" | Индексировать, искать по смыслу |
| **Logic** | Term, Rule, Predicate | "Что мы можем вывести?" | Рассуждать, вычислять следствия |
| **Transport** | Session, WantList, Bitswap | "Как обмениваться надёжно?" | Координировать, доставлять блоки |

---

## Storage Domain (Хранилище)

### Язык
```
Block: Неизменяемая единица данных
CID: Криптографический идентификатор (hash)
DAG: Граф блоков с ссылками (direcciones)
IPLD: Формат сериализации (CBOR, JSON)
```

### Инвариант
```
hash(block.data) == block.cid   (ВСЕГДА)
```

### Основной поток
```
1. Пользователь: "Добавь данные"
2. Storage: compute_cid(data) → hash
3. Storage: create Block { cid, data }
4. Storage: verify cid matches
5. Storage: persist to Sled
6. User: получает CID
```

### Метрики
- Block PUT: 50µs
- Block GET (cache): 30µs
- Block GET (disk): 100µs

→ Полная информация в [04-StorageDomain.md](04-StorageDomain.md)

---

## Network Domain (Сеть)

### Язык
```
Peer: Удалённый узел
PeerId: Уникальный идентификатор peer'а (hash публичного ключа)
DHT: Распределённая таблица хеширования
Multiaddr: Адрес для достижения peer'а
```

### Инвариант
```
PeerId = hash(public_key)   (ВСЕГДА)
```

### Основной поток
```
1. Система: "Кто имеет блок XYZ?"
2. Network: запрос к DHT
3. DHT: перебирает peer'ов, ближайших к XYZ
4. Network: получает список peer'ов
5. System: пытается подключиться
```

### Метрики
- DHT lookup: 150-300ms
- Peer discovery: 100-200ms

→ Полная информация в [05-NetworkDomain.md](05-NetworkDomain.md)

---

## Semantic Domain (Семантика)

### Язык
```
Embedding: Вектор, представляющий смысл
HNSW: Индекс для быстрого поиска (иерархический)
Similarity: Расстояние между векторами (0.0 to 1.0)
Query: Запрос (также вектор)
```

### Инвариант
```
0.0 ≤ similarity_score ≤ 1.0   (ВСЕГДА)
```

### Основной поток
```
1. Пользователь: "Найди похожие документы"
2. Semantic: compute embedding(query)
3. Semantic: HNSW.search(query_vector, k=10)
4. Semantic: rank results by similarity
5. User: видит Top-10 похожих
```

### Метрики
- HNSW search: 1-10ms (100k vectors)
- Query cache hit: 85%

→ Полная информация в [06-SemanticDomain.md](06-SemanticDomain.md)

---

## Logic Domain (Логика)

### Язык
```
Term: Переменная, константа, или составное выражение
Predicate: Отношение (parent(alice, bob))
Rule: Если-то утверждение
Fact: Конкретное предложение (no variables)
Substitution: Привязка переменных
```

### Инвариант
```
Правила должны быть непротиворечивы (ВСЕГДА)
```

### Основной поток
```
1. Пользователь: "Найди всех предков Alice"
2. Logic: add facts (parent relations)
3. Logic: add rules (ancestor definition)
4. Logic: query(ancestor(alice, X))
5. Logic: backward chaining
6. User: видит [bob, charlie, ...]
```

### Метрики
- Типичный запрос: 1-5ms
- Recursion depth limit: 1000

→ Полная информация в [07-LogicDomain.md](07-LogicDomain.md)

---

## Transport Domain (Транспорт)

### Язык
```
Session: Батч запросов блоков
WantList: Приоритетная очередь блоков, которые хотим
Message: Сериализованное сообщение Bitswap
Ledger: Per-peer учёт (я им дал, они мне дали)
```

### Инвариант
```
FIFO доставка per-peer (ВСЕГДА)
```

### Основной поток
```
1. Application: "Нужны блоки [A, B, C]"
2. Transport: create Session
3. Transport: build want_list with priorities
4. Transport: select best peers by reputation
5. Transport: send Want messages
6. (network transfer)
7. Transport: receive blocks, verify CID
8. Transport: mark session complete
```

### Метрики
- Session creation: <1ms
- Peer selection: <1ms
- Full block fetch: 200-1000ms

→ Полная информация в [08-TransportDomain.md](08-TransportDomain.md)

---

## Как они взаимодействуют?

### Через трейты (чистые интерфейсы)

```rust
// Storage Domain определяет:
pub trait BlockStore {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
}

// Transport Domain использует:
impl TransportManager {
    async fn fetch_block(&self, cid: &Cid) {
        // Не знает о Sled!
        // Просто использует traitом
        let block = self.blockstore.get(cid).await?;
    }
}
```

### Через события

```rust
// Storage испускает событие
pub enum StorageEvent {
    BlockAdded(Cid),
    BlockRemoved(Cid),
}

// Semantic подписывается
semantic.subscribe_storage_events().await?;

// Автоматически индексирует новые блоки
```

### Через данные

```
Storage → дает Block
Network → узнаёт, где найти его
Semantic → индексирует по смыслу
Logic → может рассуждать о нём
Transport → обменивается им с peer'ами
```

---

## DDD в действии

### Языковый барьер между контекстами

В Storage говорим: "CID, Block, Verification"
В Network говорим: "Peer, DHT, Reputation"
В Semantic говорим: "Embedding, Similarity, HNSW"

Они НЕ смешиваются! Это их сила.

### Anti-Corruption Layer

Если контексты взаимодействуют, используют **adapter**:

```rust
// Transport (говорит о Sessions) взаимодействует с
// Network (говорит о Peers)

// Adapter переводит языки:
pub struct TransportNetworkAdapter {
    transport: Arc<TransportManager>,
    network: Arc<NetworkManager>,
}

impl TransportNetworkAdapter {
    async fn find_peer_for_block(&self, cid: &Cid) -> Result<Peer> {
        // Transport terms → Network terms
        let peers = self.network.find_providers(cid).await?;
        // Network terms → Transport terms
        self.transport.score_peers(peers)
    }
}
```

---

## Практический пример: add_file()

```
User: ipfrs add document.pdf

┌─ Application Layer ─────────────────────────┐
│ use case: add_file(path)                    │
└─────────────────────────────────────────────┘
    ↓
┌─ Storage Domain ────────────────────────────┐
│ 1. read file → Bytes                        │
│ 2. chunk(bytes) → [Block, Block, Block, ...]│
│ 3. for each block:                          │
│    - compute CID                            │
│    - verify CID matches                     │
│    - blockstore.put(block)                  │
└─────────────────────────────────────────────┘
    ↓
┌─ Network Domain ────────────────────────────┐
│ for each block:                             │
│   - network.announce(cid)                   │
│   - tell DHT "I have this"                  │
└─────────────────────────────────────────────┘
    ↓
┌─ Semantic Domain (если конфигурирован) ─────┐
│ for each block:                             │
│   - extract_embedding(block)                │
│   - semantic.index(cid, embedding)          │
└─────────────────────────────────────────────┘
    ↓
User: "Добавлено! CID = bafybeig..."
```

Каждый домен делает свою часть, не знаяо других деталях.

---

## Масштабируемость этого подхода

### Добавить новый домен?
Просто создай новый bounded context:
- Определи свой язык
- Реализуй свой трейт
- Подписывайся на события других

### Заменить реализацию?
Например, вместо Sled использовать RocksDB:
- Создай новую реализацию BlockStore trait'а
- Application не меняется!
- Network не меняется!

### Параллельное развитие
Каждый домен может развиваться независимо:
- Storage optimizations не влияют на Network
- Semantic improvements не влияют на Logic
- Transport improvements помогают всем

---

## Что дальше?

Выберите интересующий вас домен:

1. [04-Storage Domain](04-StorageDomain.md) - Как хранятся блоки?
2. [05-Network Domain](05-NetworkDomain.md) - Как находят peer'ов?
3. [06-Semantic Domain](06-SemanticDomain.md) - Как ищут по смыслу?
4. [07-Logic Domain](07-LogicDomain.md) - Как рассуждают?
5. [08-Transport Domain](08-TransportDomain.md) - Как обмениваются?

Или посмотрите [09-Data Flows](09-DataFlows.md) для полных примеров.

---

**Связанные**: [01-Overview](01-Overview.md) | [02-Architecture Stack](02-ArchitectureStack.md) | [04-Storage Domain](04-StorageDomain.md)
