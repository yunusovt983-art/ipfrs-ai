---
title: README
type: navigation
summary: Точка входа в Wiki — навигация по ролям (архитектор/инженер/DevOps/контрибьютор)
tags: [meta, readme, navigation]
related: ["[[INDEX]]", "[[WIKI_SCHEMA]]", "[[01-Overview]]"]
updated: 2026-06-18
---

# IPFRS Wiki: Второй мозг распределённой системы

> Это wiki-документация IPFRS (Inter-Planetary File Rust System) в стиле заметок о глубоком обучении. Здесь собраны ключевые идеи, инсайты и понимание того, как работает система.

**Версия**: 0.2.0 "Network Release"  
**Статус**: Production Ready  
**Язык**: Русский  
**Стиль**: Заметки второго мозга

---

## Что такое IPFRS?

IPFRS решает фундаментальный вопрос:

> **Как объединить хранилище данных (человеческое знание) с распределённым интеллектом (автоматическое рассуждение) под одной архитектурой?**

Ответ: сделать интеллект **неотъемлемой частью** слоя хранения.

### Главная идея

```
Традиционная IPFS      →    IPFRS (эволюция)
────────────────────────────────────────────
Статический склад      →    Мыслящая магистраль
Только данные          →    Данные + смысл + рассуждение
Хеширование           →    Хеширование + семантика + логика
```

---

## Как организована эта Wiki

### 📚 Основные разделы

1. **[Обзор системы](01-Overview.md)** - Что это? Зачем?
2. **[6-слойная архитектура](02-ArchitectureStack.md)** - От железа до UI
3. **[5 Bounded Contexts (DDD)](03-BoundedContexts.md)** - Пять независимых доменов
4. **[Хранилище](04-StorageDomain.md)** - Sled DB, блоки, CID
5. **[Сеть](05-NetworkDomain.md)** - libp2p, DHT, peer discovery
6. **[Семантика](06-SemanticDomain.md)** - HNSW, векторный поиск
7. **[Логика](07-LogicDomain.md)** - Backward chaining, вывод
8. **[Транспорт](08-TransportDomain.md)** - Bitswap, обмен блоками
9. **[Полные потоки данных](09-DataFlows.md)** - 4 примера end-to-end
10. **[Производительность](10-Performance.md)** - Метрики, память, задержки
11. **[Обработка ошибок](11-ErrorHandling.md)** - Стратегии восстановления

### 🎯 Как выбрать, что читать?

**Если вы архитектор** (2-3 часа)
- Начните с [Обзора](01-Overview.md)
- Прочитайте [6-слойную архитектуру](02-ArchitectureStack.md)
- Изучите [5 Bounded Contexts](03-BoundedContexts.md)
- Посмотрите [Performance](10-Performance.md)

**Если вы инженер** (3+ часа)
- Начните с нужного вам домена (Storage/Network/Semantic/Logic/Transport)
- Прочитайте [Полные потоки](09-DataFlows.md)
- Изучите [Performance](10-Performance.md) для оптимизации

**Если вы DevOps** (1.5 часа)
- [6-слойная архитектура](02-ArchitectureStack.md)
- [Performance](10-Performance.md) (память, диск)
- [Обработка ошибок](11-ErrorHandling.md)

**Если вы контрибьютор** (5+ часов)
- Весь путь от начала до конца
- Глубокое погружение в интересующий домен
- Руки-на-клавиатуре изучение исходного кода

---

## Ключевые концепции

### Content Identifier (CID)

Криптографическая идентичность всех данных в системе.

```
CID = hash(data)

Свойства:
✓ Детерминированный: одни и те же данные → один и тот же CID
✓ Неизменяемый: CID не может измениться (иначе это другой CID)
✓ Глобально уникальный: 2^256 защита от коллизий
```

→ Подробнее в [Хранилище](04-StorageDomain.md)

### Block (Блок)

Неизменяемая единица хранения, идентифицируемая CID.

```
Block {
    cid: Cid,           // Идентичность
    data: Bytes,        // Содержание (неизменяемое)
    metadata: Metadata, // Метаданные
}

Инвариант: hash(block.data) == block.cid
```

→ Подробнее в [Хранилище](04-StorageDomain.md)

### Peer (Узел сети)

Удалённый компьютер с уникальным PeerId и репутацией.

```
Peer {
    peer_id: PeerId,        // Уникальная идентичность
    reputation: Score,      // Доверие (0.0 to 1.0)
    known_blocks: Vec<Cid>, // Что мы знаем, что у него есть
}

reputation = success_rate × recency × speed × availability
```

→ Подробнее в [Сеть](05-NetworkDomain.md)

### Semantic Index (HNSW)

Иерархический индекс для поиска по смыслу.

```
HNSW = Hierarchical Navigable Small World

Поиск k-NN за ~1ms в индексе 100k векторов.
Точность: ~99% от истинного k-NN (приближённый).
```

→ Подробнее в [Семантика](06-SemanticDomain.md)

### Inference (Вывод)

Автоматическое логическое рассуждение на основе правил.

```
Query: ancestor(alice, X)?

Backward chaining:
  Правило 1: ancestor(X,Y) :- parent(X,Y)
  Правило 2: ancestor(X,Z) :- parent(X,Y), ancestor(Y,Z)

Результаты: [bob, charlie, ...]
```

→ Подробнее в [Логика](07-LogicDomain.md)

---

## Быстрая справка по производительности

| Операция | Время | Пропускная способность |
|----------|-------|----------------------|
| Block GET (кеш) | 30µs | — |
| Block PUT | 50µs | 20k ops/sec |
| Block GET (диск) | 100µs | 33k ops/sec |
| HNSW поиск | 1-10ms | 1k queries/sec |
| DHT lookup | 150-300ms | 100 queries/sec |
| Логический вывод | 1-5ms | — |
| Полный fetch из сети | 200-1000ms | — |

→ Детали в [Performance](10-Performance.md)

---

## Архитектурная философия

IPFRS следует **Domain-Driven Design (DDD)** с пятью независимыми bounded contexts:

```
┌─────────────────────────────────────┐
│  STORAGE DOMAIN                     │
│  "Что у нас есть? Как это найти?"   │
├─────────────────────────────────────┤
│  NETWORK DOMAIN                     │
│  "Где находятся peer'ы?"            │
├─────────────────────────────────────┤
│  SEMANTIC DOMAIN                    │
│  "Что это означает?"                │
├─────────────────────────────────────┤
│  LOGIC DOMAIN                       │
│  "Что мы можем вывести?"            │
├─────────────────────────────────────┤
│  TRANSPORT DOMAIN                   │
│  "Как обмениваться надёжно?"        │
└─────────────────────────────────────┘
```

Каждый домен:
- Имеет свой язык и концепции
- Может развиваться независимо
- Через трейты взаимодействует с другими
- Тестируется в изоляции

→ Подробнее в [5 Bounded Contexts](03-BoundedContexts.md)

---

## Инварианты системы

Эти условия НИКОГДА не должны нарушаться:

1. **Storage**: `hash(block.data) == block.cid`
2. **Network**: `PeerId = hash(public_key)`
3. **Semantic**: `0.0 ≤ similarity_score ≤ 1.0`
4. **Logic**: Правила должны быть непротиворечивы
5. **Transport**: FIFO доставка сообщений per-peer

Если инвариант нарушен → критическая ошибка.

---

## Технический стек

```
Runtime:        Tokio 1.52 (async)
Network:        libp2p 0.56 (QUIC, TCP, WebSocket)
Storage:        Sled 0.34 (embedded B+ tree)
Semantic:       HNSW 0.3 (vector indexing)
HTTP Server:    Axum 0.8
Serialization:  serde, bincode
Compression:    oxiarc-* (Pure Rust)
```

→ Детали в [Architecture Stack](02-ArchitectureStack.md)

---

## Как использовать эту Wiki

### 📖 Стиль чтения

Эта wiki написана в стиле **зашифрованных заметок второго мозга**:

- Короткие, сфокусированные страницы
- Много диаграмм и примеров
- Ссылки между страницами (сильно связанный граф)
- Идеи перед деталями
- Практические примеры кода

### 🔗 Навигация

Каждая страница содержит:
- **Краткое резюме в начале** (один параграф)
- **Ключевые идеи** (выделены)
- **Диаграммы** (ASCII art)
- **Примеры кода**
- **Ссылки на связанные страницы** (в конце)

### 💡 Как читать эффективно

1. **Первый проход** (skim): Прочитайте все заголовки и ключевые идеи
2. **Второй проход** (dive): Изучите интересующие вас разделы
3. **Третий проход** (reference): Используйте как справочник

---

## Навигационная карта

```
README (вы здесь)
├── 01-Overview
│   └── Главная идея системы
├── 02-ArchitectureStack
│   └── 6 слоёв от железа до UI
├── 03-BoundedContexts
│   └── 5 доменов в DDD
├── 04-StorageDomain
│   ├── Sled, блоки, GC
│   └── Write/read пути
├── 05-NetworkDomain
│   ├── libp2p, DHT
│   └── Peer reputation
├── 06-SemanticDomain
│   ├── HNSW индекс
│   └── k-NN поиск
├── 07-LogicDomain
│   ├── Backward chaining
│   └── Proof tree
├── 08-TransportDomain
│   ├── Bitswap протокол
│   └── Session management
├── 09-DataFlows
│   ├── Add file (100 MB)
│   ├── Retrieve file
│   ├── Semantic search
│   └── Logic query
├── 10-Performance
│   ├── Latency profile
│   ├── Throughput
│   └── Memory breakdown
└── 11-ErrorHandling
    ├── Error taxonomy
    └── Recovery strategies
```

---

## Быстрые ссылки

**Теория**: [Overview](01-Overview.md) → [Architecture Stack](02-ArchitectureStack.md) → [Bounded Contexts](03-BoundedContexts.md)

**Практика**: [Data Flows](09-DataFlows.md) → [Performance](10-Performance.md)

**Углубление**: Любой из 5 доменов ([Storage](04-StorageDomain.md), [Network](05-NetworkDomain.md), [Semantic](06-SemanticDomain.md), [Logic](07-LogicDomain.md), [Transport](08-TransportDomain.md))

---

## О стиле этой Wiki

Эта документация вдохновлена подходом Андрея Карпати к ведению записей:

> "Идея вести подробные записи о своём исследовании, чтобы создать второй мозг. Не для публикации, а для глубокого понимания."

Здесь вы найдёте:
- ✓ Главные идеи, а не все детали
- ✓ Много диаграмм и примеров
- ✓ Связи между концепциями
- ✓ Практические инсайты
- ✓ Честный анализ trade-off'ов

---

**Последнее обновление**: 2026-06-18  
**Версия**: 0.2.0 "Network Release"  
**Статус**: ✅ Production Ready

---

### Начните отсюда 👇
- Новичок? Прочитайте [01-Overview.md](01-Overview.md)
- Есть опыт? Перейдите в интересующий домен
- Хотите полную картину? Читайте по порядку

```
Добро пожаловать в IPFRS Wiki! 🚀
Приготовьтесь к深遠な путешествию в распределённых системах.
```
