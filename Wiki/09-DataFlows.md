# 4 Полных потока данных: End-to-End примеры

**Краткое резюме**: Понимание архитектуры через конкретные примеры. Здесь показано, как данные проходят через все 5 доменов в 4 реальных сценариях.

---

## Сценарий 1: User добавляет файл (100 MB)

**Что происходит**: Пользователь добавляет большой PDF. Система разбивает его, хранит, объявляет в сети, индексирует семантически.

```
User: ipfrs add document.pdf (100 MB)
    ↓
CLI читает файл 100 MB → Bytes
    ↓
┌─ STORAGE ──────────────────────────────────────┐
│ Chunker разбивает на 391 блок (256KB каждый)   │
│ Для каждого блока:                             │
│  1. compute_cid = hash(chunk_data)             │
│  2. block = Block { cid, data }                │
│  3. verify: hash(block.data) == cid ✓          │
│  4. storage.put(block) → Sled DB               │
│  5. update_cache(block) → LRU (now 99% hits)   │
│ Time: ~20ms (parallel chunking)                │
└────────────────────────────────────────────────┘
    ↓
┌─ NETWORK ──────────────────────────────────────┐
│ Для каждого CID:                               │
│  1. network.announce(cid)                      │
│  2. DHT.put_provider(cid, my_peer_id)          │
│  3. Распространяется на 20 DHT узлов           │
│  4. Connected peers: "я имею эти CID"          │
│ Time: ~100ms (async background)                │
└────────────────────────────────────────────────┘
    ↓
┌─ SEMANTIC (если конфигурирован) ───────────────┐
│ Если это текстовый документ:                   │
│  1. extract_text(pdf) → "..."                  │
│  2. ml_model.encode(text) → [0.1, -0.2, ...]   │
│  3. hnsw.insert(cid, embedding)                │
│  4. Update query cache                         │
│ Time: ~500ms (ML inference)                    │
│ Параллельно с добавлением!                     │
└────────────────────────────────────────────────┘
    ↓
User видит: "Успешно добавлено! 391 блок"
            "Root CID: bafybeig..."
            "Размер: 100 MB"
            "Объявлено в сети"

ИТОГОВОЕ ВРЕМЯ: ~900ms (параллельные операции)
```

### Что произошло в каждом домене?

**Storage**: ✅ 391 блок сохранён в Sled
**Network**: ✅ 391 CID объявлены в DHT
**Semantic**: ✅ 1 embedding создано и индексировано (если нужно)
**Logic**: — (не используется для файла)
**Transport**: — (используется для передачи из других узлов)

---

## Сценарий 2: User получает файл из сети

**Что происходит**: Пользователь запрашивает блок. Система проверяет локально, затем DHT, затем peer'ов.

```
User: ipfrs get bafybeig...

┌─ STORAGE (Local check) ─────────────────────┐
│ 1. check_cache(cid)?                        │
│    a) HIT: вернуть из LRU (~30µs) ✅        │
│    b) MISS: продолжить...                   │
│                                             │
│ 2. storage.get(cid)?                        │
│    a) HIT: вернуть из Sled (~100µs) ✅      │
│    b) MISS: нет локально, идём в сеть...    │
└─────────────────────────────────────────────┘
    ↓ (if not found locally)
┌─ NETWORK (DHT lookup) ──────────────────────┐
│ 1. dht.find_providers(cid)                  │
│    Итеративный поиск:                       │
│    - Спросить bootstrap peers               │
│    - Они скажут: "спроси тех узлов"         │
│    - Спросить тех                           │
│    - Они ближе к CID → спроси их            │
│    - Continue until converge                │
│                                             │
│ 2. Result: [PeerId1, PeerId2, PeerId3, ...] │
│    Размер 20-100 peer'ов                    │
│ Time: 150-300ms (network latency)           │
└─────────────────────────────────────────────┘
    ↓
┌─ TRANSPORT (Peer selection) ────────────────┐
│ 1. Для каждого peer вычисли score:          │
│    score = success_rate ×                   │
│             recency ×                       │
│             speed ×                         │
│             availability                    │
│                                             │
│ 2. Выбери peer с максимальным score         │
│    Peer A: 0.95 × 0.99 × 1.0 × 1.0 = 0.94   │
│    Peer B: 0.70 × 0.80 × 0.3 × 0.5 = 0.08   │
│    → Выбираем Peer A                        │
│                                             │
│ 3. transport.create_session([cid])          │
│ Time: <1ms                                  │
└─────────────────────────────────────────────┘
    ↓
┌─ TRANSPORT (Block exchange) ────────────────┐
│ 1. Отправить Bitswap сообщение:             │
│    Want(cid=bafybeig, priority=100)         │
│ Time: 50-100ms (network RTT to peer)        │
│                                             │
│ 2. Peer обрабатывает:                       │
│    a) storage.get(cid) на его машине        │
│    b) Отправляет Block(cid, data)           │
│ Time: 50-100ms (network back)               │
│                                             │
│ 3. Мы получаем блок:                        │
│    a) Verify: hash(data) == cid ✓           │
│    b) storage.put(block) → наше Sled        │
│    c) Обновить cache                        │
│    d) Обновить peer reputation (success++)  │
│ Time: <1ms                                  │
└─────────────────────────────────────────────┘
    ↓
User видит: [file bytes]

ИТОГОВОЕ ВРЕМЯ:
  ✓ Cache hit:      30µs
  ✓ Local disk:     200µs  
  ✓ Network fetch:  200-1000ms (в основном сетевая задержка)
```

### Что произошло в каждом домене?

**Storage**: ✅ Проверили локально, сохранили полученный блок
**Network**: ✅ Нашли peer'ов через DHT
**Transport**: ✅ Выбрали лучшего peer'а, обменялись блоком
**Semantic**: — (не используется)
**Logic**: — (не используется)

---

## Сценарий 3: User выполняет семантический поиск

**Что происходит**: Пользователь ищет документы по смыслу. HNSW находит k-NN в одном шаге.

```
User: "Найди документы, похожие на машинное обучение"

┌─ APPLICATION ───────────────────────────────┐
│ query = "machine learning"                  │
└─────────────────────────────────────────────┘
    ↓
┌─ SEMANTIC (Embedding) ──────────────────────┐
│ 1. ml_model.encode(query)                   │
│    Output: [0.14, -0.09, 0.23, ...]         │
│    (768-dimensional vector)                 │
│ Time: ~100ms (ML inference)                 │
└─────────────────────────────────────────────┘
    ↓
┌─ SEMANTIC (Query cache check) ──────────────┐
│ 1. hash(embedding) → "abc123def"            │
│ 2. cache.get("abc123def")?                  │
│    a) HIT (85%): вернуть сразу (~1µs) ✅    │
│    b) MISS: идём в HNSW...                  │
└─────────────────────────────────────────────┘
    ↓ (if cache miss)
┌─ SEMANTIC (HNSW search) ────────────────────┐
│ HNSW = Hierarchical Navigable Small World   │
│ (иерархический индекс для fast k-NN)        │
│                                             │
│ Algorithm:                                  │
│ 1. Start at Layer 0 (top)                   │
│ 2. Find nearest neighbor in layer           │
│ 3. Descend to Layer 1                       │
│ 4. Expand search radius                     │
│ 5. Continue to Layer N (all vectors)        │
│ 6. Converge on k=10 nearest neighbors       │
│                                             │
│ Result: [(cid1, 0.95), (cid2, 0.92), ...]   │
│ Time: 1-10ms (зависит от размера индекса)   │
│                                             │
│ 3. Cache result: cache.set(hash, results)   │
└─────────────────────────────────────────────┘
    ↓
┌─ STORAGE (Fetch metadata) ──────────────────┐
│ Для каждого результата CID:                 │
│  1. storage.get(cid)                        │
│  2. Extract title/preview/metadata          │
│ Time: 30-100µs per block                    │
└─────────────────────────────────────────────┘
    ↓
User видит:
  [
    { cid: "bafybeig...", similarity: 0.95, title: "Deep Learning" },
    { cid: "bafybeih...", similarity: 0.92, title: "Neural Networks" },
    { cid: "bafybeii...", similarity: 0.88, title: "Transformers" },
    ...
  ]

ИТОГОВОЕ ВРЕМЯ:
  ✓ Cache hit:      ~1ms (embedding + cache lookup)
  ✓ HNSW search:    1-10ms (depending on index size)
  ✓ Metadata:       1-5ms (fetching blocks)
  ✓ Total:          ~1-15ms
```

### Что произошло в каждом домене?

**Semantic**: ✅ Создали embedding, искали в HNSW, кешировали
**Storage**: ✅ Получили метаданные блоков
**Network**: — (не используется)
**Logic**: — (не используется)
**Transport**: — (не используется)

---

## Сценарий 4: User выполняет логический запрос

**Что происходит**: Пользователь запрашивает логическое вывод. Engine выполняет backward chaining.

```
User: "Найди всех предков Alice"

Knowledge Base (предварительно загруженная):
  Facts:
    - parent(alice, bob)
    - parent(bob, charlie)
    - parent(charlie, diana)
  
  Rules:
    - ancestor(X, Y) :- parent(X, Y)
    - ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)

┌─ LOGIC (Backward chaining) ─────────────────────────────────────┐
│ Query: ancestor(alice, X)?                                      │
│                                                                 │
│ Step 1: Try Rule 1: ancestor(A,B) :- parent(A,B)                │
│   ancestor(alice, X) matches with A=alice, B=X                  │
│   Subgoal: parent(alice, X)?                                    │
│   Check facts: parent(alice, bob) ✓                             │
│   Solution 1: ancestor(alice, bob) ✓                            │
│                                                                 │
│ Step 2: Try Rule 2: ancestor(A,Z) :- parent(A,Y), ancestor(Y,Z) │
│   ancestor(alice, X) matches with A=alice, Z=X                  │
│   Subgoal 1: parent(alice, Y)?                                  │
│   Check facts: parent(alice, bob) ✓                             │
│   Y = bob                                                       │
│   Subgoal 2: ancestor(bob, X)?                                  │
│                                                                 │
│   Recurse on ancestor(bob, X):                                  │
│     Try Rule 1: ancestor(bob, B) :- parent(bob, B)              │
│     parent(bob, X)? Check: parent(bob, charlie) ✓               │
│     Solution: ancestor(bob, charlie) ✓                          │
│     Propagate: ancestor(alice, charlie) ✓                       │
│                                                                 │
│  Try Rule 2: ancestor(bob, Z) :- parent(bob,Y2), ancestor(Y2,Z) │
│     parent(bob, Y2)? Y2=charlie                                 │
│     ancestor(charlie, X)?                                       │
│       Try Rule 1: parent(charlie, X)?                           │
│       parent(charlie, diana) ✓                                  │
│       Solution: ancestor(charlie, diana) ✓                      │
│       Propagate: ancestor(bob, diana) ✓                         │
│       Propagate: ancestor(alice, diana) ✓                       │
│                                                                 │
│ All solutions found!                                            │
│ Time: 1-5ms for typical queries                                 │
└─────────────────────────────────────────────────────────────────┘
    ↓
User видит:
  [
    { substitute: "X = bob", confidence: 1.0 },
    { substitute: "X = charlie", confidence: 1.0 },
    { substitute: "X = diana", confidence: 1.0 },
  ]

ИТОГОВОЕ ВРЕМЯ: ~3ms (для этого размера базы знаний)
```

### Что произошло в каждом домене?

**Logic**: ✅ Выполнили backward chaining, построили proof tree
**Storage**: — (знания загружены в памяти для этого примера)
**Network**: — (не используется)
**Semantic**: — (не используется)
**Transport**: — (не используется)

---

## Сравнение потоков

| Сценарий | Домены | Время | Сетевая задержка |
|----------|--------|-------|------------------|
| Add 100MB | Storage, Network, Semantic | ~900ms | 100ms |
| Get блок | Storage, Network, Transport | 200-1000ms | 100-500ms |
| Semantic search | Storage, Semantic | ~1-15ms | 0ms |
| Logic query | Logic | ~1-5ms | 0ms |

---

## Ключевые инсайты

1. **Параллелизм**: Многие операции идут параллельно (Storage + Network + Semantic)
2. **Кеширование**: Кеши на каждом уровне (LRU, query cache) дают 10-100x ускорение
3. **Сетевое время доминирует**: Fetch из сети = 200-1000ms vs. локальные операции = микросекунды
4. **Каждый домен независим**: Они могут оптимизироваться отдельно
5. **End-to-end**: Весь поток управляется Application слоем

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [10-Performance](10-Performance.md) | [03-Bounded Contexts](03-BoundedContexts.md)
