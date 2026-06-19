---
title: 13-DeepArchitecture
type: architecture
summary: Полная глубокая архитектура системы — 6 слоёв, 5 контекстов, все потоки данных, рантайм, хранилище, сеть, семантика, логика, память, обработка ошибок
tags: [ipfrs, ddd, architecture, deep-dive, runtime, performance]
source: cool-japan/IPFRS_DEEP_ARCHITECTURE.md
related: ["[[12-MasterArchitecture]]", "[[03-BoundedContexts]]", "[[09-DataFlows]]", "[[10-Performance]]", "[[11-ErrorHandling]]"]
read_time: 90 мин
updated: 2026-06-18
---

# IPFRS: Полная Глубокая Архитектура

> **Версия**: 0.2.0 «Network Release» — Production Ready  
> **Цель**: понять, как IPFRS реально функционирует на каждом уровне  
> **Статус**: ✅ Полная справочная документация

---

## Содержание

1. [[#Обзор системы]]
2. [[#Многоуровневая архитектура]]
3. [[#Пять ограниченных контекстов]]
4. [[#Паттерны потоков данных]]
5. [[#Взаимодействие компонентов]]
6. [[#Агрегаты и инварианты]]
7. [[#Модель исполнения в рантайме]]
8. [[#Хранилище — глубокое погружение]]
9. [[#Сеть — глубокое погружение]]
10. [[#Семантика — глубокое погружение]]
11. [[#Логика — глубокое погружение]]
12. [[#Полные сквозные потоки операций]]
13. [[#Модель памяти и производительности]]
14. [[#Обработка ошибок и восстановление]]

---

## Обзор системы

### Что такое IPFRS?

IPFRS — это распределённая файловая система, отвечающая на фундаментальный вопрос:

> **Как объединить человеческое знание (хранение данных) с машинным интеллектом (рассуждение) в рамках единого протокола?**

Ответ: **Сделав интеллект встроенным в сам слой хранения.**

Традиционный IPFS хранит данные. IPFRS хранит **смысл** вместе с данными через:
- Контентно-адресованные блоки (детерминированная идентичность)
- Семантические векторы (извлечение смысла)
- Логическое программирование (автоматическое рассуждение)
- Распределённый консенсус (согласование без центра власти)

### Основная философия: двухуровневая архитектура

```
┌──────────────────────────────────────────────────────┐
│         ЛОГИЧЕСКИЙ СЛОЙ (Мозг)                       │
│  - Semantic Router: HNSW-поиск по векторам           │
│  - TensorLogic Store: правила + вывод                │
│  - Knowledge Base: факты + рассуждение               │
├──────────────────────────────────────────────────────┤
│         ФИЗИЧЕСКИЙ СЛОЙ (Тело)                       │
│  - Block Storage: контентно-адресованные блоки (CID) │
│  - Network Stack: libp2p + QUIC + DHT                │
│  - Transport Protocols: Bitswap, TensorSwap          │
└──────────────────────────────────────────────────────┘
```

Ключевой инсайт: эти слои **не разделены** — они работают в унисон:
- Семантические индексы направляют сетевую маршрутизацию
- Логическое программирование оптимизирует размещение блоков
- Распределённый вывод использует возможности пиров
- Сетевое обнаружение информирует семантическое индексирование

---

## Многоуровневая архитектура

### Полный стек (6 уровней)

```
┌──────────────────────────────────────────────────────────────────┐
│ УРОВЕНЬ 0: Интерфейс пользователя                                │
│  HTTP Gateway | CLI | WASM | Node.js | Python биндинги           │
└────────────────────┬─────────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────────┐
│ УРОВЕНЬ 1: Приложение (Use Cases & Оркестрация)                  │
│  add_file | get_file | search_semantic | query_logic             │
│  pin_add | pin_rm | dag_import | dag_export                      │
│  Координирует работу доменов                                     │
└────────────────────┬─────────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────────┐
│ УРОВЕНЬ 2: Доменный слой (5 ограниченных контекстов)             │
│                                                                  │
│  ┌──────────────┐ ┌──────────────┐ ┌───────────────┐            │
│  │Storage Domain│ │Network Domain│ │Semantic Domain│            │
│  └──────────────┘ └──────────────┘ └───────────────┘            │
│                                                                  │
│  ┌──────────────┐ ┌──────────────────────────────┐              │
│  │Logic Domain  │ │Transport Domain              │              │
│  └──────────────┘ └──────────────────────────────┘              │
└────────────────────┬─────────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────────┐
│ УРОВЕНЬ 3: Абстракция инфраструктуры                             │
│  BlockStore Trait | Network Trait | Semantic Trait               │
│  Определяют интерфейсы, которые реализации обязаны выполнять     │
└────────────────────┬─────────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────────┐
│ УРОВЕНЬ 4: Реализации (конкретные движки)                        │
│  Sled (хранилище) | libp2p (сеть) | HNSW (семантика) | tokio    │
└────────────────────┬─────────────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────────────┐
│ УРОВЕНЬ 5: Аппаратура и ОС                                       │
│  NVMe SSD | Ethernet | CPU Cores | Memory | Kernel Scheduler     │
└──────────────────────────────────────────────────────────────────┘
```

---

## Пять ограниченных контекстов

### Контекст 1: STORAGE DOMAIN

**Ответственность**: «Какие данные у нас есть и как их найти?»

**Язык и концепции**:
- **Block**: Иммутабельная единица данных (обычно 256 КБ)
- **CID (Content Identifier)**: Криптографический хеш-идентификатор
- **Dag Node**: Связи между блоками, образующие DAG
- **Ipld**: InterPlanetary Linked Data (структурированный формат)

**Ключевой инвариант**:
```
hash(block.data) == block.cid
```
Проверяется при каждом чтении. Нарушение означает обнаружение повреждения.

**Реализация: крейт `ipfrs-storage`**

```
ipfrs-storage/
├── backends/
│   ├── sled/        # По умолчанию: embedded, pure Rust
│   ├── parity-db/   # Опция: высокая производительность
│   └── rocksdb/     # Опция: проверенный C++ бэкенд
├── cache/
│   └── lru.rs       # LRU-кэш над персистентным хранилищем
├── versioning/
│   ├── git.rs       # Git для тензоров (контроль версий моделей)
│   └── snapshots/   # Путешествие во времени к историческим состояниям
└── traits/
    └── blockstore.rs # Трейт, который реализуют все бэкенды
```

**Как работает Storage**:

1. **Запись блока**:
   ```
   Пользователь предоставляет: Bytes
   Storage вычисляет: CID = hash(bytes)
   Storage сохраняет: (CID → Bytes) в Sled
   Storage возвращает: CID пользователю
   ```

2. **Извлечение блока**:
   ```
   Пользователь предоставляет: CID
   Storage проверяет: (1) LRU-кэш (99% запросов попадает сюда)
                      (2) базу данных Sled
                      (3) вернуть None, если не найдено
   Пользователь получает: Block | None
   ```

3. **Верификация при чтении**:
   ```
   retrieved_cid = hash(block_bytes)
   if retrieved_cid != requested_cid:
       обнаружено повреждение!
       attempt_repair()
   ```

**Отслеживаемая статистика Storage**:
```rust
pub struct StorageStats {
    total_blocks: u64,              // Количество хранимых блоков
    total_size_bytes: u64,          // Итоговое занятое место
    block_distribution: HashMap<BlockSize, u64>,
    cache_hit_rate: f64,            // Частота попаданий в LRU
    cache_evictions: u64,           // Вытеснений из кэша
    garbage_collected: u64,         // Байт освобождено GC
    corruption_repairs: u64,        // Восстановленных блоков
}
```

**Трейт BlockStore**:
```rust
#[async_trait]
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    async fn delete(&self, cid: &Cid) -> Result<()>;
    async fn all(&self) -> Result<Vec<Cid>>;
    async fn pin(&self, cid: &Cid) -> Result<()>;
    async fn unpin(&self, cid: &Cid) -> Result<()>;
}
```

> Подробности: [[04-StorageDomain]]

---

### Контекст 2: NETWORK DOMAIN

**Ответственность**: «Как найти пиров и узнать, что у них есть?»

**Язык и концепции**:
- **Peer**: Удалённый узел с уникальным PeerId
- **Multiaddr**: Адрес достижимости пира (например, `/ip4/1.2.3.4/tcp/30333`)
- **DHT (Distributed Hash Table)**: Глобальный индекс «кто что имеет»
- **PeerInfo**: Оценка репутации, адреса, известные блоки
- **Capability**: Какой контент может отдавать пир

**Ключевой инвариант**:
```
PeerId = hash(public_key)
```
Иммутабелен и глобально уникален для каждого узла.

**Реализация: крейт `ipfrs-network`** (1250+ строк)

```
ipfrs-network/
├── node.rs              # Основной NetworkNode (обёртка libp2p)
├── behaviors/
│   ├── identify.rs      # Протокол идентификации пира
│   ├── kademlia.rs      # DHT для маршрутизации контента
│   ├── mdns.rs          # Обнаружение в локальной сети
│   ├── autonat.rs       # Определение NAT
│   ├── dcutr.rs         # Пробивание дыр (hole punching)
│   └── gossipsub.rs     # Pub/sub для распределённого вывода
├── peer/
│   ├── manager.rs       # Трекинг и скоринг пиров
│   └── reputation.rs    # Вычисление доверия
├── protocols/
│   ├── identify.rs      # /ipfs/id/1.0.0
│   ├── kad.rs           # Kademlia DHT
│   └── custom.rs        # IPFRS-специфичные протоколы
└── routing/
    └── content.rs       # Обнаружение контента
```

**Как работает Network**:

1. **Запуск узла**:
   ```
   1. Генерировать уникальный PeerId из ключевой пары
   2. Привязаться к адресам прослушивания (0.0.0.0:30333)
   3. Связаться с bootstrap-пирами
   4. Запустить mDNS-обнаружение в локальной сети
   5. Запустить Kademlia DHT
   6. Вступить в GossipSub для тем вывода
   7. Готов к обмену блоками
   ```

2. **Поток обнаружения пиров**:
   ```
   Network.bootstrap()
       └─→ Подключиться к bootstrap-пирам
           └─→ Спросить их: «Кого вы знаете?»
               └─→ Получить 20–30 рекомендаций
                   └─→ Подключиться к перспективным
                       └─→ Спросить: «У кого блок X?»
                           └─→ DHT возвращает список пиров
   ```

3. **Анонс контента**:
   ```
   Storage: «Я только что сохранил блок CID=xyz»
   Network: Сообщает DHT «У меня есть xyz»
   DHT: Хранит (xyz → [my_peer_id]) на 20 пирах
   Результат: любой поиск xyz в DHT находит нас
   ```

4. **Скоринг пиров**:
   ```
   reputation_score = success_rate × time_decay × behavior_bonus
   
   success_rate = successful_blocks / total_requested
   time_decay = exp(-age_days / 30)  // Недавние успехи весят больше
   behavior_bonus = +5 за быстрые ответы, -10 за таймауты
   
   Пиры с репутацией > 0.7 получают приоритет в запросах
   ```

**Статистика Network**:
```rust
pub struct NetworkStats {
    peer_count: usize,
    bootstrap_peers: Vec<PeerId>,
    content_providers: HashMap<Cid, Vec<PeerId>>,
    bytes_sent: u64,
    bytes_received: u64,
    average_latency_ms: f64,
}
```

> Подробности: [[05-NetworkDomain]]

---

### Контекст 3: SEMANTIC DOMAIN

**Ответственность**: «Что означают данные? Можно ли найти похожий контент?»

**Язык и концепции**:
- **Embedding**: Высокоразмерный вектор, представляющий смысл контента
- **Vector Space**: 768-мерное пространство, где похожий контент близок
- **HNSW Index**: Hierarchical Navigable Small World-граф для быстрого поиска
- **Similarity Score**: Расстояние между векторами (0.0 до 1.0)
- **Query Filter**: Ограничения на результаты поиска

**Ключевой инвариант**:
```
0.0 ≤ similarity_score ≤ 1.0
```

**Реализация: крейт `ipfrs-semantic`** (931 строка)

```
ipfrs-semantic/
├── router.rs            # Основной SemanticRouter (обёртка HNSW)
├── index/
│   ├── hnsw.rs          # Иерархический граф малого мира
│   ├── persistent.rs    # Сохранение/загрузка индекса с диска
│   └── analyzer.rs      # Проверка состояния индекса
├── cache/
│   ├── query_cache.rs   # LRU-кэш последних запросов
│   └── embedding_cache.rs  # Кэш вычисленных embeddings
├── metrics/
│   ├── similarity.rs    # Вычисление расстояний
│   ├── filtering.rs     # Фильтры запросов
│   └── ranking.rs       # Ранжирование результатов
└── config.rs            # Конфигурация и настройка
```

**Как работает семантический поиск**:

1. **Фаза индексирования**:
   ```
   Блок: «Обзорный документ по машинному обучению»
   
   ML-модель: конвертирует текст → embedding
   Вывод: [0.142, -0.089, 0.234, ...] (768 измерений)
   
   HNSW: «Вставить этот вектор в граф»
   Граф поддерживает: ближайших соседей, иерархические слои
   Результат: блок доступен для поиска по смысловому сходству
   ```

2. **Фаза запроса**:
   ```
   Запрос пользователя: «Какие документы обсуждают глубокое обучение?»
   
   Модель: конвертирует запрос → embedding
   Вывод: [0.151, -0.091, 0.227, ...] (то же пространство)
   
   HNSW: «Найти k ближайших соседей к этому запросу»
   Алгоритм:
     1. Начать с верхнего слоя
     2. Найти ближайшую точку
     3. Спуститься в слой ниже, повторить
     4. Продолжать до сходимости на k-NN
   
   Результат: Top 10 похожих документов
   ```

3. **Оптимизация через кэш**:
   ```
   Кэш запросов хранит: (hash(embedding) → results)
   
   Запрос: «бумаги по deep learning»
   Похожий запрос 10 секунд назад? ДА → вернуть кэшированные результаты
   Никогда не встречался? НЕТ → запустить HNSW, закэшировать результат
   
   Hit Rate: ~85% при типичных паттернах
   ```

**Статистика Semantic**:
```rust
pub struct SemanticStats {
    indexed_blocks: u64,
    index_size_mb: f64,
    cache_hit_rate: f64,
    average_query_latency_ms: f64,
    most_similar_pairs: Vec<(Cid, Cid, f64)>,
}
```

**Метрики расстояния**:
```rust
pub enum DistanceMetric {
    Cosine,      // (x·y) / (|x||y|) — наиболее распространённая
    L2,          // sqrt(sum((x-y)²))  — Евклидова
    Jaccard,     // |A∩B| / |A∪B|      — сходство множеств
    Manhattan,   // sum(|x-y|)         — такси-расстояние
}
```

> Подробности: [[06-SemanticDomain]]

---

### Контекст 4: LOGIC DOMAIN

**Ответственность**: «Что можно вывести из данных? Возможно ли автоматическое рассуждение?»

**Язык и концепции**:
- **Term**: Переменная, константа или составное выражение
- **Predicate**: Отношение между термами (например, `parent(alice, bob)`)
- **Rule**: Утверждение если-то (например, `ancestor(X,Z) :- parent(X,Y), ancestor(Y,Z)`)
- **Fact**: Предикат без переменных
- **Substitution**: Привязки переменных из успешной унификации
- **Proof**: Трассировка шагов вывода

**Ключевой инвариант**:
```
Правила должны быть непротиворечивы
Вывод должен завершаться (well-founded semantics)
```

**Реализация: крейт `ipfrs-tensorlogic`** (1334 строки)

```
ipfrs-tensorlogic/
├── store.rs             # Основная KnowledgeBase
├── engine/
│   ├── unify.rs         # Сопоставление с образцом
│   ├── infer.rs         # Обратный вывод (backward chaining)
│   ├── forward.rs       # Прямой вывод (forward chaining)
│   └── abductive.rs     # Абдуктивное рассуждение
├── ir/
│   ├── term.rs          # Представление термов
│   ├── predicate.rs     # Определение предикатов
│   ├── rule.rs          # Определение правил
│   └── program.rs       # Полная логическая программа
├── analysis/
│   ├── dependencies.rs  # Граф зависимостей правил
│   ├── termination.rs   # Проверка завершаемости вывода
│   └── consistency.rs   # Проверка противоречий
└── utils/
    └── pretty_print.rs  # Читаемый вывод
```

**Как работает логическое программирование**:

1. **Наполнение базы знаний**:
   ```rust
   // Добавить факты
   add_fact(parent(alice, bob))
   add_fact(parent(bob, charlie))
   
   // Добавить правила
   add_rule(ancestor(X, Y) :- parent(X, Y))
   add_rule(ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z))
   ```

2. **Запрос вывода**:
   ```
   Запрос: ancestor(alice, ?)
   
   Движок:
     1. ancestor(alice, ?) совпадает с головой правила: X=alice, Z=?
     2. Попробовать: parent(alice, Y) — унификация с Y=bob ✓
     3. Попробовать: ancestor(bob, ?) — рекурсивное правило
        a. parent(bob, Y2) — унификация с Y2=charlie ✓
        b. ancestor(charlie, ?) — рекурсия
           - parent(charlie, ?) — совпадений нет, стоп
        c. ancestor(bob, charlie) доказано ✓
     4. Вернуть: ancestor(alice, bob) ✓
                ancestor(alice, charlie) ✓
   ```

3. **Алгоритм обратного вывода**:
   ```
   infer(goal):
       for each rule (head :- body):
           if unify(goal, head) succeeds with substitution σ:
               for each subgoal in apply(σ, body):
                   solutions = infer(subgoal)
                   for each solution:
                       yield solution composed with σ
   ```

**Статистика Logic**:
```rust
pub struct TensorLogicStats {
    facts: u64,
    rules: u64,
    inference_depth_limit: u64,
    last_inference_time_ms: f64,
    proof_tree_depth: usize,
}
```

> Подробности: [[07-LogicDomain]]

---

### Контекст 5: TRANSPORT DOMAIN

**Ответственность**: «Как надёжно обмениваться блоками между пирами?»

**Язык и концепции**:
- **Session**: Пакетный запрос нескольких блоков
- **WantList**: Список желаемых блоков с приоритетами
- **Message**: Сериализованное сообщение Bitswap или TensorSwap
- **Peer Scoring**: Алгоритм выбора, у какого пира запрашивать
- **Circuit Breaker**: Паттерн для работы с неисправными пирами

**Ключевой инвариант**:
```
FIFO-доставка сообщений на каждое соединение
(упорядочено, но без гарантий; нужно обрабатывать потери/дубли)
```

**Реализация: крейт `ipfrs-transport`**

```
ipfrs-transport/
├── bitswap/
│   ├── exchange.rs      # Машина состояний протокола Bitswap
│   ├── messages.rs      # Типы сообщений Want/Have/Block
│   ├── wantlist.rs      # Приоритетная очередь запросов
│   └── ledger.rs        # Per-peer учёт
├── tensorswap/
│   ├── streaming.rs     # Стриминг для тензоров
│   ├── chunking.rs      # Работа с метаданными тензоров
│   └── pipeline.rs      # Параллельные запросы чанков
├── session/
│   ├── manager.rs       # Жизненный цикл сессии
│   ├── state.rs         # Машина состояний (created→active→completed)
│   └── progress.rs      # Отслеживание прогресса в процентах
└── peer_scoring/
    ├── reputation.rs    # Вычисление per-peer score
    ├── strategies.rs    # Разные алгоритмы скоринга
    └── circuit_breaker.rs  # Паттерн быстрого отказа
```

**Как работает обмен блоками (протокол Bitswap)**:

1. **Инициализация сессии**:
   ```
   Пользователь: «Мне нужны блоки [CID1, CID2, CID3]»
   TransportManager: Создаёт BlockExchangeSession
   Сессия: 
     - requested_blocks: [CID1, CID2, CID3]
     - state: Active
     - want_list: Приоритетная очередь
     - peers: Выбраны по репутации
   ```

2. **Управление WantList**:
   ```
   WantList (приоритетная очередь):
   ┌──────────────────────────────────────┐
   │ CID1  priority=100 (нужен первым)    │
   │ CID2  priority=50  (нужен вторым)    │
   │ CID3  priority=10  (может подождать) │
   └──────────────────────────────────────┘
   
   Отправить пиру: «Хочу CID1 (100), CID2 (50), CID3 (10)»
   Пир: Приоритизирует CID1 в ответе
   ```

3. **Поток сообщений**:
   ```
   Client                           Peer
      │                              │
      │ Want(CID1, prio=100)         │
      ├─────────────────────────────>│
      │                              │ (проверяет хранилище)
      │                              │
      │ Have(CID1) [опционально]     │
      │<─────────────────────────────┤
      │                              │
      │ Block(CID1, data...)         │
      │<─────────────────────────────┤
      │ (блок получен)               │
      │                              │
      │ Want(CID2, prio=50)          │
      ├─────────────────────────────>│
      │ Cancel(CID1)                 │
      ├─────────────────────────────>│
   ```

4. **Скоринг пиров для выбора**:
   ```
   score(peer) = 
       success_rate(0.0-1.0) × 
       response_speed(latency_factor) × 
       availability(0.0-1.0) × 
       freshness(time_decay)
   
   peer_a: score = 0.95 × 1.2 × 0.98 × 0.99 = 1.12 ✓ ВЫБРАТЬ
   peer_b: score = 0.70 × 0.8 × 0.85 × 0.92 = 0.44 ✗ ПРОПУСТИТЬ
   ```

5. **Завершение сессии**:
   ```
   Когда все блоки получены:
   1. Верифицировать CID каждого блока
   2. Сохранить в storage
   3. Обновить семантический индекс
   4. Отметить сессию как Completed
   5. Обновить репутацию пира (success++)
   6. Вернуть пользователю
   ```

> Подробности: [[08-TransportDomain]]

---

## Паттерны потоков данных

### Паттерн 1: Пользователь добавляет файл

```
Пользователь: «Add ~/document.pdf»
         │
         ▼
CLI (ipfrs-cli)
  read_file(path) → Bytes
         │
         ▼
Application Layer (node.rs)
  add_file(bytes)
         │
         ├─→ Storage: compute_cid(bytes)
         │   - Хешировать через BLAKE3
         │   - Создать структуру Block
         │   - Сохранить в Sled DB
         │   ✓ Возвращает CID
         │
         ├─→ Network: announce(cid)
         │   - Сообщить DHT «У меня есть этот блок»
         │   - Сохранить на 20 DHT-узлах
         │   - Сообщить подключённым пирам
         │   ✓ Знание распределено
         │
         ├─→ Semantic (если настроено): index(cid, embedding)
         │   - Извлечь смысл если применимо
         │   - Вставить в HNSW-граф
         │   ✓ Поиск по смыслу доступен
         │
         └─→ Пользователю: «Добавлено: CID=bafybeig...»
```

**Время**: ~50мс локально, +200мс распространение в сети  
**Гарантии**: CID глобально уникален и детерминирован

---

### Паттерн 2: Пользователь извлекает файл

```
Пользователь: «Get bafybeig...»
         │
         ▼
Application Layer
  get_block(cid)
         │
         ├─→ Storage: check_local()
         │   - Проверить LRU-кэш ← 30мкс при попадании
         │   - Проверить Sled DB ← 100мкс при промахе
         │   ✓ Если найдено: вернуть немедленно
         │
         ├─→ Network (если промах): find_peers(cid)
         │   - Запросить DHT: «У кого CID?»
         │   - DHT возвращает список пиров
         │   ✓ Получено: [PeerId1, PeerId2, PeerId3]
         │
         ├─→ Transport: create_session([cid])
         │   - Создать BlockExchangeSession
         │   - Оценить пиров
         │   - Отправить Want(CID) лучшему пиру
         │   ✓ Запрос блока в полёте
         │
         ├─→ (Путь сетевого пакета)
         │   Пир получает Want(CID)
         │   Хранилище пира: get(CID)
         │   Транспорт пира: отправить Block(CID, data)
         │
         ├─→ (Клиент получает Block)
         │   Верифицировать: hash(data) == CID ✓
         │   Сохранить в локальное хранилище
         │   Обновить репутацию пира (success++)
         │   ✓ Блок готов для пользователя
         │
         └─→ Пользователю: «[байты файла]»
```

**Время**: ~30мкс (попадание в кэш) до ~1000мкс (сетевое извлечение)  
**Гарантии**: целостность CID проверяется перед возвратом

---

### Паттерн 3: Семантический поиск

```
Пользователь: «Find documents similar to this topic»
         │
         ▼
Application Layer
  search_semantic(topic, k=10)
         │
         ├─→ ML Model: embed(topic)
         │   - Конвертировать текст в вектор
         │   - Вывод: [0.142, -0.089, ...] (768 dim)
         │   ✓ Вектор запроса готов
         │
         ├─→ Semantic: check_cache(query_vector)
         │   - Хешировать query_vector
         │   - Искать в LRU-кэше
         │   ✓ Если закэшировано: вернуть немедленно (85% hit rate)
         │
         ├─→ Semantic (промах кэша): hnsw_search(query_vector, k=10)
         │   Алгоритм:
         │   1. Начать с уровня 0 (верхний)
         │   2. Найти ближайшего соседа
         │   3. Перейти на уровень 1
         │   4. Повторять до сходимости
         │   ✓ Получено: [(CID1, 0.92), (CID2, 0.88), ...]
         │
         ├─→ Semantic: rank_and_filter(results)
         │   - Отсортировать по similarity score
         │   - Применить фильтры пользователя (если есть)
         │   ✓ Ранжированные результаты
         │
         ├─→ Storage: fetch_metadata()
         │   - Для каждого CID результата — получить блок
         │   - Извлечь заголовок/превью
         │   ✓ Обогащённые результаты
         │
         └─→ Пользователю: [
                 {cid: "bafybeig...", similarity: 0.92, title: "..."},
                 {cid: "bafybeih...", similarity: 0.88, title: "..."},
                 ...
             ]
```

**Время**: ~1мс (кэш) до ~10мс (HNSW-поиск)  
**Гарантии**: результаты отсортированы по семантическому сходству

---

### Паттерн 4: Логический запрос

```
Пользователь: «Find all ancestors of Alice»
         │
         ▼
Application Layer
  query_logic(Goal)
         │
         ├─→ Logic: add_facts()  [если нужно]
         │   - parent(alice, bob)
         │   - parent(bob, charlie)
         │   ✓ База знаний обновлена
         │
         ├─→ Logic: add_rule()  [если нужно]
         │   - ancestor(X, Y) :- parent(X, Y)
         │   - ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
         │   ✓ Правила на месте
         │
         ├─→ Logic: infer(ancestor(alice, ?))
         │   Алгоритм (Backward Chaining):
         │   1. ancestor(alice, ?) не совпадает с фактами напрямую
         │   2. Попробовать правило 1: ancestor(X, Y) :- parent(X, Y)
         │      - Унифицировать ancestor(alice, ?) с ancestor(X, Y)
         │      - X=alice, Y=?
         │      - Доказать parent(alice, Y): Y=bob ✓
         │   3. Попробовать правило 2: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
         │      - X=alice, Z=?
         │      - Доказать parent(alice, Y): Y=bob ✓
         │      - Доказать ancestor(bob, Z):
         │        * Унифицировать с правилом 1: Y2=?
         │        * Доказать parent(bob, Y2): Y2=charlie ✓
         │        * Значит ancestor(bob, charlie) ✓
         │   4. Вернуть решения: [bob, charlie, ...]
         │
         └─→ Пользователю: [{substitute: "Y=bob"}, {substitute: "Y=charlie"}, ...]
```

**Время**: ~1–5мс для типичных запросов  
**Гарантии**: находит все решения через depth-first search

> Дополнительные сценарии: [[09-DataFlows]]

---

## Взаимодействие компонентов

### Как домены взаимодействуют в рантайме

```
┌──────────────────────────────────────────────────────────┐
│                   СЛОЙ ПРИЛОЖЕНИЯ                        │
│  Оркестрирует use cases, вызывает методы доменов         │
└──────────────────────────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  STORAGE    │  │  NETWORK    │  │  SEMANTIC   │
│  Domain     │  │  Domain     │  │  Domain     │
└─────────────┘  └─────────────┘  └─────────────┘
        │                │                │
        └────────────────┼────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼
┌─────────────┐  ┌─────────────┐
│  LOGIC      │  │  TRANSPORT  │
│  Domain     │  │  Domain     │
└─────────────┘  └─────────────┘
```

### Паттерны межконтекстного взаимодействия

**Паттерн A: Repository Pattern (слабая связанность)**
```rust
// Storage-домен определяет интерфейс
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
}

// Semantic-домен не знает о реализации Sled
// Просто вызывает методы трейта
pub struct SemanticRouter {
    storage: Arc<dyn BlockStore>,  // Трейт-объект, не конкретный тип
}

impl SemanticRouter {
    async fn index_block(&self, cid: &Cid) -> Result<()> {
        if let Some(block) = self.storage.get(cid).await? {
            // Обрабатываем блок...
        }
    }
}
```

**Паттерн B: Эмиттинг событий (асинхронное уведомление)**
```rust
pub enum StorageEvent {
    BlockAdded(Cid),
    BlockRemoved(Cid),
}

// Storage эмиттит событие
pub struct StorageBackend {
    event_tx: broadcast::Sender<StorageEvent>,
}

impl StorageBackend {
    async fn put(&self, block: &Block) -> Result<()> {
        // ... сохранить ...
        self.event_tx.send(StorageEvent::BlockAdded(*block.cid()))?;
    }
}

// Semantic слушает события
pub struct SemanticRouter {
    mut event_rx: broadcast::Receiver<StorageEvent>,
}

// Автоматически индексирует новые блоки при добавлении
async fn watch_storage() {
    while let Ok(StorageEvent::BlockAdded(cid)) = event_rx.recv().await {
        self.index_content(&cid, embedding).await?;
    }
}
```

**Паттерн C: Dependency Injection (конструктор)**
```rust
pub struct Node {
    storage: Arc<dyn BlockStore>,
    network: Arc<NetworkNode>,
    semantic: Arc<SemanticRouter>,
    logic: Arc<KnowledgeBase>,
    transport: Arc<TransportManager>,
}

impl Node {
    pub fn new(
        storage: Arc<dyn BlockStore>,
        network: Arc<NetworkNode>,
        semantic: Arc<SemanticRouter>,
        logic: Arc<KnowledgeBase>,
        transport: Arc<TransportManager>,
    ) -> Self {
        Self { storage, network, semantic, logic, transport }
    }
}
```

---

## Агрегаты и инварианты

### Агрегат 1: Block

**Корневая сущность**: `Block`  
**Инвариант**: `hash(data) == cid`

```rust
pub struct Block {
    cid: Cid,           // Content identifier (иммутабелен)
    data: Bytes,        // Фактические данные (иммутабельны)
    metadata: BlockMetadata,
}

impl Block {
    pub fn new(data: Bytes) -> Result<Self> {
        let cid = Cid::new(
            hash_algorithm::BLAKE3,
            codec::RAW,
            blake3::hash(&data),
        )?;
        Ok(Block { cid, data, metadata: BlockMetadata::new(data.len()) })
    }
    
    pub fn verify(&self) -> Result<()> {
        let computed_cid = Self::compute_cid(&self.data)?;
        if computed_cid != self.cid {
            return Err(Error::CidMismatch);
        }
        Ok(())
    }
}
```

**Жизненный цикл**:
1. **Создание**: пользователь предоставляет данные, CID вычисляется
2. **Персистентность**: хранится в Sled, анонсируется в DHT
3. **Использование**: ссылка по CID, извлечение и верификация
4. **Очистка**: незакреплённые блоки подлежат сборке мусора

---

### Агрегат 2: Peer

**Корневая сущность**: `Peer`  
**Инвариант**: `PeerId = hash(public_key)`

```rust
pub struct Peer {
    peer_id: PeerId,
    multiaddrs: Vec<Multiaddr>,
    reputation: Score,
    known_blocks: HashSet<Cid>,
    last_seen: Instant,
    connection_state: ConnectionState,
}

impl Peer {
    pub fn score(&self) -> f64 {
        let age_days = self.last_seen.elapsed().as_secs_f64() / (24.0 * 3600.0);
        let recency = (-age_days / 30.0).exp();  // Экспоненциальный распад
        self.reputation * recency
    }
    
    pub fn update_on_success(&mut self) {
        self.reputation = (self.reputation + 1.0).min(100.0);
    }
    
    pub fn update_on_failure(&mut self) {
        self.reputation = (self.reputation * 0.9).max(0.0);
    }
}
```

**Жизненный цикл**:
1. **Обнаружение**: через bootstrap, mDNS или DHT
2. **Подключение**: установить libp2p-соединение
3. **Отслеживание**: мониторинг успехов/ошибок
4. **Скоринг**: обновление репутации после взаимодействия
5. **Вытеснение**: удалить при слишком низкой репутации

---

### Агрегат 3: BlockExchangeSession

**Корневая сущность**: `BlockExchangeSession`  
**Инвариант**: `received_blocks ⊆ requested_blocks`

```rust
pub struct BlockExchangeSession {
    session_id: SessionId,
    requested_blocks: Vec<Cid>,
    received_blocks: HashSet<Cid>,
    failed_blocks: HashMap<Cid, Error>,
    state: SessionState,
    created_at: Instant,
    updated_at: Instant,
}

pub enum SessionState {
    Created,           // Только создан
    Active,            // Получение блоков
    Paused,            // Временно остановлен
    Completed,         // Все блоки получены
    Failed(String),    // Отказ
}

impl BlockExchangeSession {
    pub fn progress_percent(&self) -> f64 {
        self.received_blocks.len() as f64 / self.requested_blocks.len() as f64 * 100.0
    }
    
    pub fn is_complete(&self) -> bool {
        self.received_blocks.len() == self.requested_blocks.len()
    }
}
```

---

## Модель исполнения в рантайме

### Архитектура Tokio async runtime

```
┌────────────────────────────────────────────────────────┐
│          Tokio Async Runtime (8–16 потоков)            │
└────────────────────────────────────────────────────────┘
                        │
      ┌─────────────────┼─────────────────┐
      │                 │                 │
      ▼                 ▼                 ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│   CPU Task   │ │   CPU Task   │ │   CPU Task   │
│   Queue 1    │ │   Queue 2    │ │   Queue 3    │
└──────────────┘ └──────────────┘ └──────────────┘
      │                 │                 │
      ├─→ Accept Loop   ├─→ Send Loop     ├─→ Receive Loop
      │   (новые конн.) │   (исходящие)   │   (обработка)
      │                 │                 │
      ▼                 ▼                 ▼
   Spawn           Spawn            Spawn
   Handler        Handler           Handler
   (per conn)     (per session)     (per message)
```

**Иерархия задач**:

```
main()
├─ network.start()
│  ├─ listen_loop()
│  │  └─ [для каждого входящего соединения]
│  │     └─ connection_handler()
│  │        ├─ identify protocol
│  │        ├─ upgrade connection
│  │        └─ message_handler() [цикл]
│  ├─ dht_query_loop()
│  │  └─ [периодическое обслуживание DHT]
│  └─ peer_scoring_loop()
│     └─ [периодическое обновление репутации]
├─ storage.start()  [если нужно, например, компакция]
├─ semantic.gc_loop()
│  └─ [периодическая очистка индекса]
├─ http_gateway.listen()
│  └─ [для каждого HTTP-запроса]
│     └─ request_handler()
└─ signal_handler()
   └─ [ожидание Ctrl+C]
      └─ graceful_shutdown()
```

**Примитивы синхронизации**:

```rust
// Разделяемое состояние между задачами
Arc<parking_lot::RwLock<T>>      // Read-write lock
Arc<DashMap<K, V>>               // Lock-free concurrent hashmap
Arc<tokio::sync::Mutex<T>>       // Async mutex (можно await)
Arc<tokio::sync::mpsc::Channel>  // Передача сообщений

// Пример: множество задач читают storage без блокировки
let storage = Arc::new(SledBlockStore::new()?);

for i in 0..100 {
    let storage_clone = Arc::clone(&storage);
    tokio::spawn(async move {
        let block = storage_clone.get(&cid).await?;  // Неблокирующее чтение
    });
}
```

---

## Хранилище — глубокое погружение

### Архитектура базы данных Sled

```
Код пользователя
    │
    ▼
    ┌─────────────────────────────────────────┐
    │  Block Storage API                      │
    │  put(cid, block) / get(cid) / has(cid)  │
    └─────────────────────────────────────────┘
    │
    ▼
    ┌──────────────────────────────────────────┐
    │  LRU Cache Layer (Arc<DashMap>)          │
    │  • 99% попаданий для горячих блоков      │
    │  • Вытесняет least-recently-used         │
    │  • Async-доступ без блокировок           │
    └──────────────────────────────────────────┘
    │
    ▼
    ┌─────────────────────────────────────────┐
    │  Верификация контрольной суммы          │
    │  • Вычислить хеш извлечённого блока     │
    │  • Сверить с сохранённым CID            │
    │  • Обнаружить/восстановить повреждение  │
    └─────────────────────────────────────────┘
    │
    ▼
    ┌─────────────────────────────────────────┐
    │  Sled Embedded Database                 │
    │  • Embedded B+ tree                     │
    │  • Атомарные транзакции                 │
    │  • Crash-safe с WAL                     │
    │  • ~30мкс get, ~50мкс put               │
    └─────────────────────────────────────────┘
    │
    ▼
    ┌──────────────────────────────────────────┐
    │  Файловая система (NVMe SSD)             │
    │  • Файлы в директории data/blocks        │
    │  • Одна пара ключ-значение на запись     │
    │  • Кэш страниц ОС обеспечивает буфер    │
    └──────────────────────────────────────────┘
```

### Путь записи (put)

```
1. Ввод: Block { cid, data }
   ↓
2. Верификация: hash(data) == cid (проверка целостности)
   ↓
3. Сериализация: Block → Bytes (через bincode)
   ↓
4. Sled: db.insert(cid_bytes, block_bytes)?
   ↓
5. WAL (Write-Ahead Log): записать операцию
   ↓
6. Flush: синхронизировать на диск (async)
   ↓
7. Cache: обновить LRU-кэш блоком
   ↓
8. Вернуть: Ok(())
   
Время: ~50мкс (память) + ~1мс (задержка диска)
```

### Путь чтения (get)

```
1. Ввод: Cid
   ↓
2. Проверить LRU Cache
   ├─ ПОПАДАНИЕ: вернуть кэшированный блок (30мкс)
   └─ ПРОМАХ: продолжить...
   ↓
3. Sled: db.get(cid_bytes)?
   ├─ ПОПАДАНИЕ: байты блока (~30мкс)
   └─ ПРОМАХ: вернуть None
   ↓
4. Десериализация: Bytes → Block
   ↓
5. Верификация: hash(block.data) == cid
   ├─ ОК: продолжить...
   ├─ ОШИБКА: повреждение обнаружено! попытка ремонта
   └─ НЕВОССТАНОВИМО: вернуть Err(CidMismatch)
   ↓
6. Cache: обновить LRU-кэш
   ↓
7. Вернуть: Ok(Some(Block))
   
Время: ~30мкс (попадание в кэш) до ~1000мкс (диск + верификация)
```

### Сборка мусора (GC)

```
Цикл GC (каждые 5 минут):

1. Получить все CID в хранилище
   ↓
2. Получить все закреплённые (pinned) CID
   ↓
3. Для каждого CID не в pinned:
   - Проверить: ссылается ли на него закреплённый блок?
   - Проверить: свежий ли он (< 7 дней)?
   - Решение: удалить если нет ссылок И старый
   ↓
4. Удалить помеченные блоки
   ↓
5. Компактировать базу данных
   ↓
6. Обновить метрики: «освобождено X байт, удалено Y блоков»
```

---

## Сеть — глубокое погружение

### Архитектура LibP2P Swarm

```
Пользователь: «Connect to peer XYZ»
    │
    ▼
┌─────────────────────────────────┐
│  NetworkNode (swarm manager)    │
│  • Поддерживает соединения      │
│  • Маршрутизирует сообщения     │
│  • Управляет behaviour'ами      │
└─────────────────────────────────┘
    │
    ├─→ Identify Protocol
    │   • Обменяться PeerId + адресами
    │   • Узнать multiaddrs пира
    │
    ├─→ Kademlia DHT
    │   • Хранить/получать (cid → [peer_ids])
    │   • XOR-метрика расстояния
    │   • 20-узловая репликация
    │
    ├─→ mDNS Discovery
    │   • Broadcast в локальной сети
    │   • Найти пиров в одной LAN
    │
    ├─→ AutoNAT
    │   • Проверить наличие NAT
    │   • Узнать публичный адрес
    │
    ├─→ DCUtR (Hole Punching)
    │   • Установить соединение через NAT
    │   • Координировать с relay-пиром
    │
    ├─→ Circuit Relay
    │   • Переадресация через relay-пир
    │   • Fallback когда прямое невозможно
    │
    └─→ GossipSub (Pub/Sub)
        • Темы распределённого вывода
        • Широковещательные запросы/ответы
    │
    ▼
┌─────────────────────────────────┐
│  QUIC Transport (quinn)         │
│  • На базе UDP, мультиплексирование
│  • Быстрая установка соединения │
│  • Управление перегрузкой       │
└─────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────┐
│  TCP/Websocket Fallback         │
│  • Для брандмауэров, блокирующих QUIC
│  • Медленнее, но совместимее    │
└─────────────────────────────────┘
```

### Поиск контента в DHT

```
Запрос: «У кого блок CID=abc?»

1. Хешировать CID в ключ: key = hash(cid)
   
2. Найти k ближайших пиров к ключу (k=20):
   - Начало: спросить bootstrap-пиры
   - Они отвечают: «попробуй этих пиров» (ближе к ключу)
   - Спросить тех: «кто ещё ближе?»
   - Продолжать итеративно до сходимости
   
3. Время: ~100–500мс в зависимости от размера DHT
   
4. Результат: Список PeerIds, хранящих этот CID
   
5. Подключиться к пиру с наивысшей репутацией
   
6. Отправить Bitswap Want-сообщение
```

### Скоринг репутации пиров

```
reputation_score(peer) = 
    success_rate × 
    recency_factor × 
    speed_factor × 
    availability_factor

Где:
  success_rate = successful_blocks / total_requested
  recency_factor = exp(-age_seconds / (30*86400))
  speed_factor = 1.0 если задержка < 100мс
                 0.5 если задержка 100–500мс
                 0.2 если задержка > 500мс
  availability_factor = 1.0 если подключён
                       0.1 если не подключён

Пример:
  Peer A: 0.98 × 0.95 × 1.0 × 1.0 = 0.93 ← хороший пир
  Peer B: 0.70 × 0.50 × 0.3 × 0.1 = 0.01 ← плохой пир
  
Выбрать Peer A для запросов.
```

---

## Семантика — глубокое погружение

### Структура HNSW-индекса

```
Векторное пространство (768 измерений)
    │
    ├─ Уровень 2 (верхний)
    │  ├─ Узел: [0.5, 0.3, ...]
    │  └─ Узел: [-0.2, 0.8, ...]
    │
    ├─ Уровень 1 (средний)
    │  ├─ Узел: [0.5, 0.3, ...]
    │  ├─ Узел: [-0.2, 0.8, ...]
    │  ├─ Узел: [0.1, -0.5, ...]
    │  └─ ...
    │
    └─ Уровень 0 (нижний — все векторы)
       ├─ Узел: [0.5, 0.3, ...]
       ├─ Узел: [-0.2, 0.8, ...]
       ├─ Узел: [0.1, -0.5, ...]
       ├─ Узел: [0.7, 0.2, ...]
       ├─ ...
       └─ 100,000+ векторов здесь
```

### Алгоритм поиска k-NN

```
Запрос: найти 10 ближайших соседей к [0.4, 0.2, ...]

1. Начать с верхнего уровня (Уровень 2)
   • Текущий = [0.5, 0.3, ...]  (ближайший на уровне 2)
   
2. Спуститься на Уровень 1
   • Сохранить текущую ближайшую точку
   • Исследовать соседей
   • Найти ещё ближе: [-0.2, 0.8, ...]
   
3. Спуститься на Уровень 0 (все векторы)
   • Начать от текущего ближайшего
   • Расширять радиус поиска по необходимости
   • Найти 10 кандидатов с наименьшим расстоянием
   
4. Вернуть отсортированными по расстоянию (ближайший первым)
   
Время: ~1мс для 100k векторов
Точность: ~99% истинного k-NN (аппроксимация)
```

### Кэширование запросов

```
Кэш запросов (LRU, макс. 10k записей):

Запрос: «бумаги по deep learning»
       │
       ├─ Модель: embed(query) → [0.14, -0.09, ...]
       │
       ├─ Хеш: hash(embedding) → "abc123def..."
       │
       ├─ Проверить кэш:
       │  ├─ ПОПАДАНИЕ: то же embedding видели раньше
       │  │        Вернуть кэшированные результаты
       │  │        ~85% hit rate
       │  │
       │  └─ ПРОМАХ: новое embedding
       │           Запустить HNSW-поиск
       │           Закэшировать результаты
       │
       └─ Вернуть результаты

Инвалидация кэша:
  - При добавлении новых блоков в индекс
  - Через 24 часа (устаревшие данные)
  - При размере кэша > 10k (вытеснить oldest)
```

---

## Логика — глубокое погружение

### Движок вывода

```
Запрос: ancestor(alice, X)?

Данные:
  факты: {parent(alice, bob), parent(bob, charlie)}
  правила: {
    ancestor(X, Y) :- parent(X, Y)
    ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
  }

Backward Chaining:
  
  Цель: ancestor(alice, X)?
  │
  ├─ Правило 1: ancestor(A, B) :- parent(A, B)
  │  │ Унифицировать ancestor(alice, X) с ancestor(A, B)
  │  │ A=alice, B=X
  │  │
  │  └─ Подцель: parent(alice, X)?
  │     ├─ Проверить факты: parent(alice, bob) ✓
  │     │  X = bob
  │     │  Решение 1: ancestor(alice, bob) ✓
  │     │
  │     └─ Нет более фактов parent(alice, *)
  │
  ├─ Правило 2: ancestor(A, Z) :- parent(A, Y), ancestor(Y, Z)
  │  │ Унифицировать ancestor(alice, X) с ancestor(A, Z)
  │  │ A=alice, Z=X
  │  │
  │  ├─ Подцель 1: parent(alice, Y)?
  │  │  └─ Факты: parent(alice, bob) ✓ Y=bob
  │  │
  │  └─ Подцель 2: ancestor(bob, X)?
  │     │ (Рекурсивный вызов)
  │     │
  │     ├─ Правило 1: parent(bob, charlie) ✓
  │     │  X = charlie → Решение 2: ancestor(alice, charlie) ✓
  │     │
  │     └─ Правило 2 → ancestor(charlie, X) → нет фактов → нет решений

Итоговые решения:
  ✓ ancestor(alice, bob)
  ✓ ancestor(alice, charlie)
```

### Дерево доказательств

```
ancestor(alice, X)
├─ Правило 1: ancestor(A,B) :- parent(A,B)
│  └─ parent(alice, bob) ✓ [Факт]
│     Решение 1: X=bob
│
└─ Правило 2: ancestor(A,Z) :- parent(A,Y), ancestor(Y,Z)
   ├─ parent(alice, Y) ✓ [Y=bob]
   └─ ancestor(bob, X) ← Рекурсия
      ├─ Правило 1: ancestor(B,C) :- parent(B,C)
      │  └─ parent(bob, charlie) ✓ [Факт]
      │     Решение 2: X=charlie
      │
      └─ Правило 2: ancestor(B,Z) :- parent(B,Y2), ancestor(Y2,Z)
         ├─ parent(bob, Y2) ✓ [Y2=charlie]
         └─ ancestor(charlie, X)
            ├─ Правило 1: parent(charlie, *) ✗ [Нет фактов]
            └─ Правило 2: parent(charlie, *) ✗ [Нет фактов]
               Нет решений
```

---

## Полные сквозные потоки операций

### Пример 1: Добавить большой файл

```
Пользователь: ipfrs-cli add document.pdf (100 МБ)
       │
       ▼
1. CLI Layer (ipfrs-cli/src/main.rs)
   └─ read_file("document.pdf") → Bytes
   
2. Application Layer (Node::add_file)
   │
   ├─→ НАРЕЗКА НА ЧАНКИ
   │   Chunker::chunk(bytes) → ChunkedFile {
   │       root_cid: Cid,
   │       chunks: [
   │           {block1, cid1}, {block2, cid2},
   │           {block3, cid3}, ...  // 391 блок
   │       ]
   │   }
   │
   ├─→ STORAGE DOMAIN (для каждого чанка)
   │   │
   │   ├─ Вычислить CID: hash(chunk_data)
   │   ├─ Создать Block: Block { cid, data }
   │   ├─ Верифицировать: hash(block.data) == cid
   │   ├─ Put в Sled: db.insert(cid_bytes, block_bytes)
   │   ├─ Обновить LRU cache
   │   └─ Результат: блок сохранён локально ✓
   │
   ├─→ SEMANTIC DOMAIN (опционально)
   │   │
   │   ├─ Извлечь текст: pdftotext(document.pdf) → "..."
   │   ├─ Вычислить embedding: model.encode(text) → [0.1, -0.2, ...]
   │   ├─ Вставить в HNSW: hnsw.insert(cid1, embedding)
   │   └─ Результат: блок индексирован для семантического поиска ✓
   │
   ├─→ NETWORK DOMAIN (в фоне)
   │   │
   │   ├─ Анонсировать все чанки в DHT
   │   │  for each cid: dht.put_provider(cid, my_peer_id)
   │   └─ Результат: пиры могут обнаружить наши блоки ✓
   │
   └─→ Результат возвращается
       └─ Пользователю: «Added 100 MB in 391 chunks»
          «Root CID: bafybeig...»
          
Разбивка по времени:
  - Чтение файла: 50мс
  - Нарезка на чанки: 150мс (параллельно)
  - Storage (391 блок × 50мкс): 20мс
  - Semantic индексирование: 500мс (если настроено)
  - Network announce: 200мс (async в фоне)
  - Итого: ~900мс (плюс фоновая сеть)
```

### Пример 2: Получить файл из сети

```
Пользователь: ipfrs-cli get bafybeig...
       │
       ▼
1. CLI Layer → Node::get_block(cid)
   
2. Application Layer
   │
   ├─→ STORAGE DOMAIN (локальная проверка)
   │   │
   │   ├─ Проверить LRU cache (30мкс при попадании)
   │   ├─ Проверить Sled DB (100мкс при попадании)
   │   ├─ Если найдено: верифицировать хеш, вернуть
   │   └─ Если нет: продолжить в сеть...
   │
   ├─→ NETWORK DOMAIN (если промах)
   │   │
   │   ├─ DHT-запрос: «У кого bafybeig?»
   │   │  └─ Итеративный поиск (100–500мс)
   │   │  └─ Результат: [PeerId1, PeerId2, ...]
   │   │
   │   ├─ Скоринг пиров
   │   │  score = success_rate × recency × speed × availability
   │   │  выбрать пира с наивысшим score
   │   │
   │   └─ Подключение
   │      если не подключён: libp2p.connect(best_peer) (100–200мс)
   │      иначе: переиспользовать соединение
   │
   ├─→ TRANSPORT DOMAIN (обмен блоком)
   │   │
   │   ├─ Создать сессию: BlockExchangeSession {
   │   │    requested_blocks: [bafybeig], state: Active
   │   │   }
   │   │
   │   ├─ Отправить Bitswap: Want(cid=bafybeig, priority=100)
   │   │
   │   ├─ (Сетевой пакет — путешествие 50–200мс)
   │   │
   │   ├─ Удалённый пир обрабатывает:
   │   │   ├─ Storage.get(bafybeig) → Block
   │   │   └─ Отправить: Block(cid, data)
   │   │
   │   ├─ (Сетевой пакет назад — 50–200мс)
   │   │
   │   └─ Клиент получает блок:
   │       ├─ Верифицировать: hash(data) == cid ✓
   │       ├─ Storage.put(block)
   │       ├─ Обновить репутацию пира (success++)
   │       ├─ Отметить сессию завершённой
   │       └─ Вернуть пользователю
   │
   └─→ Результат: [байты файла]
       
Итоговое время:
  Попадание в кэш: 30мкс
  Попадание на диск: 200мкс
  Сетевое попадание: 200–1000мс
```

---

## Модель памяти и производительности

### Потребление памяти

```
Узел IPFRS с 1 ТБ данных:

┌─────────────────────────────────┐
│ Итого: ~4.5 ГБ ОЗУ              │
├─────────────────────────────────┤
│ LRU Block Cache: 2.0 ГБ         │
│  • Кэш 10,000 горячих блоков    │
│  • ~200 КБ на блок в среднем    │
│  • Хранит часто используемые    │
│                                 │
│ HNSW Index: 1.5 ГБ              │
│  • 1М векторов × 768 измерений  │
│  • ~1.5 КБ на вектор            │
│  • Иерархические слои: ~50%     │
│    накладных расходов           │
│                                 │
│ Peer State: 100 МБ              │
│  • 10,000 пиров                 │
│  • Репутация, multiaddrs        │
│  • Метаданные соединения        │
│                                 │
│ Session State: 50 МБ            │
│  • Активный обмен блоками       │
│  • Want-листы, отслеживание     │
│                                 │
│ Sled Metadata: 200 МБ           │
│  • Структура B+ tree            │
│  • Bloom-фильтры                │
│  • Индексы блоков               │
│                                 │
│ OS/Runtime: 600 МБ              │
│  • Планировщик Tokio            │
│  • Машины состояний libp2p      │
│  • Буферы HTTP-сервера          │
│                                 │
└─────────────────────────────────┘

Использование диска:
├─ Блоки данных: 1.0 ТБ (контент)
├─ Sled DB: 10 ГБ (индексы, метаданные)
└─ Семантические индексы: 15 ГБ
   Итого: ~1.025 ТБ на диске
```

### Профиль задержки

```
Операция              P50        P99        P999
─────────────────────────────────────────────────
Block GET (кэш)       30мкс      50мкс      100мкс
Block GET (диск)      100мкс     500мкс     1мс
Block PUT             50мкс      100мкс     500мкс
Верификация CID       20мкс      40мкс      100мкс
Поиск в LRU           5мкс       10мкс      20мкс
Semantic search (k=10)1мс        5мс        10мс
DHT lookup            150мс      500мс      1000мс
Bitswap block fetch   100мс      300мс      1000мс
─────────────────────────────────────────────────

Итого для add_file(10МБ):
  - Нарезка: ~20мс
  - Storage: ~10мс
  - Semantic: ~200мс (опционально)
  - Network: ~100мс (async фон)
  = ~330мс end-to-end

Итого для get_block (сеть):
  - DHT lookup: ~150–300мс
  - Block fetch: ~100–200мс
  = ~250–500мс end-to-end
```

### Ограничения пропускной способности

```
Операция                 Макс. пропуск.   Ограничивающий фактор
─────────────────────────────────────────────────────────────────
Block PUT (одиночный)    20,000 ops/sec   Disk I/O
Block PUT (×8 парал.)    100,000 ops/sec  CPU cores
Block GET (одиночный)    33,000 ops/sec   Cache/CPU
Block GET (×8 парал.)    200,000 ops/sec  Network
Semantic indexing        2,000 docs/sec   ML model
DHT queries              100/sec          Network
Block transfer           100 Mbps         Network
─────────────────────────────────────────────────────────────────

Анализ узких мест:
• CPU-bound: нарезка, хеширование, semantic indexing
• I/O-bound: put/get хранилища, компактирование диска
• Network-bound: DHT-запросы, передача блоков
• Memory-bound: ограничение размером HNSW-индекса
```

> Дополнительные метрики: [[10-Performance]]

---

## Обработка ошибок и восстановление

### Таксономия ошибок

```
IPFRSError
├─ StorageError
│  ├─ CidMismatch          (повреждение обнаружено)
│  ├─ DatabaseError        (сбой Sled)
│  ├─ BlockNotFound        (нет локально)
│  └─ CorruptionRepaired   (исправлено автоматически)
│
├─ NetworkError
│  ├─ PeerConnectionFailed (недостижим)
│  ├─ DHTPeerNotFound      (никто не имеет блок)
│  ├─ TransportTimeout     (пир слишком долго)
│  └─ NATTraversalFailed   (за брандмауэром)
│
├─ SemanticError
│  ├─ EmbeddingFailed      (ошибка модели)
│  ├─ IndexCorrupted       (проблема HNSW)
│  └─ InsufficientDims     (несоответствие размерности)
│
├─ LogicError
│  ├─ UnificationFailed    (образец не совпал)
│  ├─ InferenceDepthLimit  (бесконечная рекурсия)
│  ├─ InconsistentRules    (противоречие)
│  └─ ProofNotFound        (цель недоказуема)
│
└─ TransportError
   ├─ SessionFailed        (обмен блоками неполный)
   ├─ AllPeersFailed       (ни один пир не имеет блок)
   ├─ CircuitBreakerOpen   (слишком много сбоев)
   └─ BlockVerificationFailed (несоответствие хеша)
```

### Стратегии восстановления

**Стратегия 1: Автоматический retry**
```rust
// Для временных сетевых ошибок
async fn get_with_retry(cid: &Cid, max_retries: usize) {
    for attempt in 0..max_retries {
        match self.get_block(cid).await {
            Ok(block) => return Ok(block),
            Err(e) if e.is_transient() => {
                // Временный сбой сети, повторить
                tokio::time::sleep(Duration::from_millis(100 * attempt)).await;
                continue;
            }
            Err(e) => return Err(e),  // Постоянная ошибка
        }
    }
}
```

**Стратегия 2: Fallback к другим пирам**
```rust
// Если пир не справился — попробовать следующего
let peers = dht.lookup(cid).await?;
let mut last_error = None;

for peer in peers {  // Отсортированы по репутации
    match self.fetch_from_peer(peer, cid).await {
        Ok(block) => return Ok(block),
        Err(e) => {
            last_error = Some(e);
            continue;  // Попробовать следующего
        }
    }
}

Err(last_error.unwrap_or(Error::NoPeersAvailable))
```

**Стратегия 3: Восстановление после повреждения**
```rust
// CID не совпадает
async fn attempt_repair(cid: &Cid, block_bytes: &[u8]) -> Result<()> {
    let computed_cid = hash(block_bytes)?;
    if computed_cid != cid {
        // Загрузить с другого пира
        if let Ok(correct_block) = self.fetch_from_network(cid).await {
            self.storage.put(&correct_block).await?;
            self.metrics.record_corruption_repair();
            return Ok(());
        }
        return Err(Error::CorruptionUnrepairable(cid));
    }
    Ok(())
}
```

**Стратегия 4: Circuit Breaker**
```rust
// Прекратить использовать плохо ведущих себя пиров
let peer_score = reputation_manager.score(peer);
if peer_score < 0.1 {      // Слишком много сбоев
    circuit_breaker.open(peer);
    continue;
} else if peer_score < 0.5 { // Пир восстанавливается
    circuit_breaker.half_open(peer);
} else {                     // Пир в норме
    circuit_breaker.close(peer);
}
```

> Подробности: [[11-ErrorHandling]]

---

## Заключение

IPFRS достигает цели **объединения данных с интеллектом** через:

1. **Детерминированную контентную адресацию**: CID обеспечивает глобальный консенсус по идентичности
2. **Распределённую архитектуру**: нет центрального авторитета, peer-to-peer консенсус
3. **Семантический интеллект**: HNSW-поиск по векторам добавляет смысл к хранению
4. **Логическое программирование**: распределённый вывод позволяет автоматическое рассуждение
5. **Надёжный транспорт**: Bitswap-протокол обеспечивает надёжный обмен блоками
6. **Async-эффективность**: Tokio runtime поддерживает тысячи параллельных операций

Пять ограниченных контекстов работают вместе, каждый специализируясь в своём домене и поддерживая чистые интерфейсы для межконтекстного взаимодействия.

**Результат**: распределённая сеть знаний, где данные не просто хранятся, а понимаются и автоматически анализируются.

---

## Что дальше?

→ [[12-MasterArchitecture]] — DDD-анализ (Evans/Vernon, контекстные отношения)  
→ [[03-BoundedContexts]] — краткий обзор всех 5 контекстов  
→ [[04-StorageDomain]] — глубже в Storage  
→ [[05-NetworkDomain]] — глубже в Network  
→ [[09-DataFlows]] — все 4 сценария потоков  
→ [[10-Performance]] — метрики и узкие места  
→ [[11-ErrorHandling]] — обработка ошибок подробно  

---

**Связанные**: [[12-MasterArchitecture]] | [[03-BoundedContexts]] | [[09-DataFlows]] | [[10-Performance]] | [[11-ErrorHandling]]

---

*Создан: перевод IPFRS_DEEP_ARCHITECTURE.md → русский*  
*Дата: 2026-06-18. Версия кодовой базы: 0.2.0.*
