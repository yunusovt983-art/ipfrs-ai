# 6-Слойная архитектура IPFRS

**Краткое резюме**: IPFRS имеет 6 чётко разделённых слоёв. Каждый слой может быть понят независимо, но вместе они образуют согласованную систему.

---

## Полная архитектурная пирамида

```
┌────────────────────────────────────────────────┐
│ LAYER 0: Пользовательский интерфейс             │
│  HTTP Gateway | CLI Tool | WASM | Node.js      │
└─────────────────┬────────────────────────────┘
                  │
┌─────────────────▼─────────────────────────────┐
│ LAYER 1: Application (Use Cases)              │
│  • add_file() - Добавить файл                 │
│  • get_file() - Получить файл                 │
│  • search_semantic() - Поиск по смыслу        │
│  • query_logic() - Логический запрос          │
│  • pin_add() / pin_rm() - Закрепление         │
│  Координирует между доменами                  │
└─────────────────┬─────────────────────────────┘
                  │
┌─────────────────▼─────────────────────────────┐
│ LAYER 2: Domain (5 Bounded Contexts)          │
│                                               │
│  ┌─────────────────┐  ┌────────────────┐      │
│  │ STORAGE DOMAIN  │  │ NETWORK DOMAIN │      │
│  │ (Блоки, CID)    │  │ (Peers, DHT)   │      │
│  └─────────────────┘  └────────────────┘      │
│                                               │
│  ┌─────────────────┐  ┌────────────────┐      │
│  │SEMANTIC DOMAIN  │  │ LOGIC DOMAIN   │      │
│  │ (HNSW, Поиск)   │  │ (Вывод, Правил)│      │
│  └─────────────────┘  └────────────────┘      │
│                                               │
│  ┌────────────────────────────────────────┐   │
│  │ TRANSPORT DOMAIN (Bitswap)             │   │
│  │ (Sessions, Want Lists, Peer Scoring)   │   │
│  └────────────────────────────────────────┘   │
└─────────────────┬─────────────────────────────┘
                  │
┌─────────────────▼────────────────────────────┐
│ LAYER 3: Infrastructure Abstraction          │
│  Blockstore trait | Network trait            │
│  Semantic trait | Logic trait                │
│  Определяет контракты между доменами         │
└─────────────────┬────────────────────────────┘
                  │
┌─────────────────▼────────────────────────────┐
│ LAYER 4: Implementation (Concrete Engines)   │
│  • Storage: Sled (embedded B+ tree DB)       │
│  • Network: libp2p (QUIC, TCP, WebSocket)    │
│  • Semantic: HNSW (hierarchical index)       │
│  • Logic: Backward chaining engine           │
│  • Runtime: Tokio (async executor)           │
└─────────────────┬────────────────────────────┘
                  │
┌─────────────────▼────────────────────────────┐
│ LAYER 5: Hardware & OS                       │
│  • CPU Cores - Tokio worker threads          │
│  • NVMe SSD - Sled database                  │
│  • Network Card - UDP/TCP stack              │
│  • RAM - Caches and indexes                  │
└──────────────────────────────────────────────┘
```

---

## Слой за слоем

### Layer 0: User Interface

**Что это**: Точки входа для пользователей системы

```
HTTP Gateway    → curl http://localhost:8080/ipfs/bafybeig...
CLI Tool        → ipfrs add file.txt
WASM Bindings   → Использование в браузерах
Node.js         → require('ipfrs')
Python          → import ipfrs
```

**Ответственность**:
- Парсинг пользовательского ввода
- Форматирование ответов
- Управление сессией/аутентификацией

### Layer 1: Application

**Что это**: Бизнес-логика и use cases

```rust
impl Node {
    pub async fn add_file(&self, path: &Path) -> Result<Cid> {
        // 1. Прочитать файл
        let data = tokio::fs::read(path).await?;
        
        // 2. Разбить на блоки
        let chunks = self.chunker.chunk(&data)?;
        
        // 3. Сохранить в Storage
        for chunk in chunks {
            self.storage.put(&chunk).await?;
        }
        
        // 4. Объявить в Network
        self.network.announce(&cid).await?;
        
        // 5. Индексировать в Semantic
        if let Some(embedding) = self.extract_embedding(&data) {
            self.semantic.index(&cid, embedding).await?;
        }
        
        Ok(cid)
    }
}
```

**Ответственность**:
- Ординирует между доменами
- Обрабатывает ошибки
- Управляет flow данных

### Layer 2: Domain Layer

**Что это**: Пять независимых bounded contexts

Каждый домен полностью независим:

```
STORAGE:    Знает о блоках и CID, ничего больше
NETWORK:    Знает о peer'ах и маршрутизации
SEMANTIC:   Знает о векторах и поиске
LOGIC:      Знает о правилах и выводе
TRANSPORT:  Знает о сессиях и обмене
```

Взаимодействуют **только** через трейты:

```rust
// Storage trait
pub trait Blockstore {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
}

// Application никогда не знает о Sled!
// Это позволяет свободно менять реализацию
```

→ Подробнее в [03-Bounded Contexts](03-BoundedContexts.md)

### Layer 3: Infrastructure Abstraction

**Что это**: Трейты, которые определяют контракты

```rust
// Storage абстракция
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
}

// Network абстракция
pub trait NetworkNode: Send + Sync {
    async fn connect_peer(&self, peer: &Peer) -> Result<()>;
    async fn announce_block(&self, cid: &Cid) -> Result<()>;
    async fn find_providers(&self, cid: &Cid) -> Result<Vec<PeerId>>;
}
```

**Ответственность**:
- Определять интерфейсы между доменами
- Гарантировать типобезопасность
- Позволять mock'ирование в тестах

### Layer 4: Implementation

**Что это**: Конкретные движки, реализующие трейты

```
Storage Layer:
  ├─ SledBlockstore (default)
  └─ ParityDB (alternative)

Network Layer:
  ├─ libp2p Swarm
  ├─ Kademlia DHT
  └─ mDNS discovery

Semantic Layer:
  ├─ HNSW Index
  └─ LRU Query Cache

Logic Layer:
  └─ Backward Chaining Engine

Runtime:
  └─ Tokio (8-16 worker threads)
```

**Ответственность**:
- Реализовать определённые интерфейсы
- Оптимизировать производительность
- Обрабатывать детали реализации

### Layer 5: Hardware & OS

**Что это**: Физические ресурсы

```
CPU:        8-16 cores (Tokio spawns worker threads)
RAM:        ~4.5 GB для 1TB данных
SSD:        NVMe (Sled требует быстрый диск)
Network:    Ethernet/WiFi (UDP/TCP stack)
```

**Ответственность**:
- Предоставлять вычислительную мощь
- Хранить данные
- Обеспечивать сетевую связь

---

## Поток управления через слои

```
User вводит: ipfrs add file.txt
      ↓
Layer 0 (CLI) парсит: path = "file.txt"
      ↓
Layer 1 (Application) вызывает: node.add_file("file.txt")
      ↓
Layer 2 (Storage Domain) вызывает: storage.put(block)
      ↓
Layer 3 (Storage Trait) маршрутизирует
      ↓
Layer 4 (Sled) реализует: db.insert(cid, data)
      ↓
Layer 5 (SSD) физически сохраняет
      ↓
Обратный путь: результат возвращается вверх
      ↓
User видит: "Added: bafybeig..."
```

---

## Почему этот порядок важен?

### Слои не должны "прыгать"

❌ Плохо:
```rust
// Application напрямую использует Sled
let db = sled::open("/tmp/db")?;
db.insert(key, value)?;  // ← Нарушает слои!
```

✅ Хорошо:
```rust
// Application использует трейт
let block = Block::new(data)?;
self.blockstore.put(&block).await?;  // ← Через интерфейс
```

### Слои обеспечивают модульность

Каждый слой может:
- Тестироваться в изоляции
- Изменяться независимо
- Заменяться альтернативой

---

## Размер слоёв в коде

```
Layer 0 (UI):                   ~500 lines (CLI + HTTP)
Layer 1 (Application):          ~1000 lines (use cases)
Layer 2 (Domains):              ~5000 lines (5 bounded contexts)
Layer 3 (Traits):               ~500 lines (interfaces)
Layer 4 (Implementation):        ~10000 lines (engines)
─────────────────────────────────────
Всего IPFRS:                    ~17000 lines Rust кода
```

---

## Асинхронное выполнение (Tokio)

Layer 4 запускает всё на Tokio async runtime:

```
┌────────────────────────────────────┐
│     Tokio Runtime (async)          │
│  8-16 worker threads               │
└────────────────────────────────────┘
          ↓
   ┌──────┴──────┬──────────┐
   ↓             ↓          ↓
Accept Loop  Send Loop   Receive Loop
(new conns) (outgoing) (incoming)
   │             │          │
   └──────┬──────┴──────┬──┘
          ↓
      (spawned tasks)
      Per-connection
      handlers
```

Каждая операция (get_block, add_block, etc.) — это async task, которая может быть приостановлена без блокирования других.

---

## Ключевые свойства этой архитектуры

| Свойство | Значение |
|----------|----------|
| **Модульность** | 5 независимых доменов |
| **Тестируемость** | Каждый домен тестируется отдельно |
| **Масштабируемость** | Async/await + lock-free structures |
| **Надёжность** | Circuit breakers, retry logic |
| **Производительность** | Zero-copy where possible |
| **Простота** | Чистые интерфейсы между слоями |

---

## Что дальше?

Углубитесь в интересующий вас слой:

- **Layer 2**: [03-Bounded Contexts](03-BoundedContexts.md)
- **Layer 4**: [04-Storage Domain](04-StorageDomain.md) и т.д.
- **Примеры**: [09-Data Flows](09-DataFlows.md)

---

**Связанные**: [01-Overview](01-Overview.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [09-Data Flows](09-DataFlows.md)

**Источник кода**: `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/`
