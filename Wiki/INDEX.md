---
title: INDEX
type: navigation
summary: Полный каталог Wiki по категориям с одностроночными summary и метаданными
tags: [meta, index, navigation]
related: ["[[README]]", "[[WIKI_SCHEMA]]", "[[log]]"]
updated: 2026-06-18
---

# IPFRS Wiki - Полный индекс и навигация

> Добро пожаловать в IPFRS Wiki - второй мозг распределённой системы.

**Версия**: 0.2.0  
**Язык**: Русский  
**Стиль**: Заметки Андрея Карпати ([LLM Wiki pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f))  
**Статус**: ✅ Complete

---

## ⚙️ Meta-слой (паттерн Карпати «LLM Wiki»)

| Файл | Роль |
|------|------|
| **[[WIKI_SCHEMA]]** | Схема — правила поддержки Wiki, трёхслойная архитектура, workflows (ingest/query/lint) |
| **[[log]]** | Append-only хроника эволюции (`## [date] op \| title`) |
| **[[README]]** | Точка входа, навигация по ролям |
| **[[INDEX]]** | Этот файл — каталог по категориям |

> 🤖 **Для LLM-агента**: прочитай [[WIKI_SCHEMA]] перед любой операцией. Источник истины — код в `Vendor/ipfrs/crates/**` (Слой 1), Wiki компилируется из него.

---

## Все страницы (13 статей)

### 📖 Начните отсюда

**[README.md](README.md)** - Главная страница Wiki
- Обзор всей системы
- Ключевые концепции
- Навигация по ролям (архитектор/инженер/DevOps/контрибьютор)

---

### 🎯 Основное содержание (в порядке чтения)

1. **[01-Overview.md](01-Overview.md)** - Система, которая мыслит
   - Что такое IPFRS?
   - Двухслойная архитектура
   - 5 независимых доменов (DDD)
   - Инварианты системы
   - ⏱️ Read time: 20 мин

2. **[02-ArchitectureStack.md](02-ArchitectureStack.md)** - 6-слойная архитектура
   - Layer 0: User Interface
   - Layer 1: Application
   - Layer 2: Domain (5 contexts)
   - Layer 3-5: Infrastructure/Implementation/Hardware
   - ⏱️ Read time: 30 мин

3. **[03-BoundedContexts.md](03-BoundedContexts.md)** - 5 независимых миров (DDD)
   - Storage Domain (Хранилище)
   - Network Domain (Сеть)
   - Semantic Domain (Семантика)
   - Logic Domain (Логика)
   - Transport Domain (Транспорт)
   - ⏱️ Read time: 40 мин

---

### 🔬 Глубокие погружения по доменам

4. **[[04-StorageDomain]]** - Хранилище (Sled, блоки, CID)
   - Block-агрегат, BlockStore-порт, stacked decorators, GC
   - source: `crates/ipfrs-storage/src/` · ⏱️ 40 мин · ✅ Complete

5. **[[05-NetworkDomain]]** - Сеть (libp2p, DHT, peer discovery)
   - Peer-агрегат, Kademlia DHT, два уровня репутации, ACL libp2p
   - source: `crates/ipfrs-network/src/` · ⏱️ 40 мин · ✅ Complete

6. **[[06-SemanticDomain]]** - Семантика (HNSW, векторный поиск)
   - VectorIndex, k-NN, квантование, reranking
   - source: `crates/ipfrs-semantic/src/` · ⏱️ 40 мин · ✅ Complete

7. **[[07-LogicDomain]]** - Логика (backward chaining, вывод)
   - IR Term/Rule/KB, нейро-символический синтез
   - source: `crates/ipfrs-tensorlogic/src/` · ⏱️ 45 мин · ✅ Complete

8. **[[08-TransportDomain]]** - Транспорт (Bitswap, сессии)
   - Session-агрегат, WantList с приоритетами, peer scoring
   - source: `crates/ipfrs-transport/src/` · ⏱️ 40 мин · ✅ Complete

---

### 💡 Практические примеры

9. **[09-DataFlows.md](09-DataFlows.md)** - 4 полных потока данных (end-to-end)
   - Сценарий 1: User добавляет 100MB файл
   - Сценарий 2: User получает файл из сети
   - Сценарий 3: User выполняет семантический поиск
   - Сценарий 4: User выполняет логический запрос
   - ⏱️ Read time: 45 мин

---

### 📊 Производительность и метрики

10. **[10-Performance.md](10-Performance.md)** - Производительность системы
    - Таблица операций (P50/P99/P999 latency)
    - Пропускная способность
    - Где находятся bottleneck'и
    - Memory consumption (для 1TB данных)
    - Real-world timing examples
    - Советы по оптимизации
    - ⏱️ Read time: 30 мин

---

### ⚠️ Обработка ошибок и восстановление

11. **[[11-ErrorHandling]]** - Обработка ошибок и recovery
    - Категории сбоев, retry с backoff, circuit breaker, по доменам
    - ⏱️ 35 мин · ✅ Complete

12. **[[12-MasterArchitecture]]** - Мастер-архитектура (полный DDD-анализ)
    - Стратегическая карта контекстов, все 5 доменов, потоки данных, инварианты
    - Перевод IPFRS_ARCHITECTURE_MASTER.md (Opus 4.8) · ⏱️ 60 мин · ✅ Complete

13. **[[13-DeepArchitecture]]** - Глубокая архитектура системы (полная)
    - 6 слоёв, 5 контекстов, рантайм, storage/сеть/семантика/логика deep dives, память, ошибки
    - Перевод IPFRS_DEEP_ARCHITECTURE.md · ⏱️ 90 мин · ✅ Complete

---

## Таблица зависимостей между статьями

```
README (точка входа)
  ├─→ 01-Overview (что это?)
  ├─→ 02-Architecture Stack (как устроено?)
  │   └─→ 03-Bounded Contexts (5 доменов)
  │       ├─→ 04-Storage Domain
  │       ├─→ 05-Network Domain
  │       ├─→ 06-Semantic Domain
  │       ├─→ 07-Logic Domain
  │       └─→ 08-Transport Domain
  │
  ├─→ 09-Data Flows (как работает end-to-end?)
  ├─→ 10-Performance (как быстро?)
  ├─→ 11-Error Handling (что если что-то сломается?)
  ├─→ 12-MasterArchitecture (полный DDD-анализ, синтез)
  └─→ 13-DeepArchitecture (полная глубокая архитектура)
```

---

## Как выбрать, что читать?

### 👨‍💼 Я архитектор (2-3 часа)
```
Путь: README → 01-Overview → 02-ArchitectureStack → 03-BoundedContexts → 10-Performance

Сфокусируйтесь на:
- Общей архитектуре
- Взаимодействии доменов
- Performance trade-offs
```

### 👨‍💻 Я инженер (3+ часа)
```
Путь 1 (новичок):
  README → 01-Overview → [интересующий домен] → 09-DataFlows → 10-Performance

Путь 2 (опытный):
  [интересующий домен] → 09-DataFlows → 10-Performance → Real code
```

### 👨‍🔧 Я DevOps (1.5 часа)
```
Путь: README → 02-ArchitectureStack → 10-Performance → 11-ErrorHandling

Сфокусируйтесь на:
- Слоях архитектуры
- Memory consumption
- Latency profiles
- Error recovery
```

### 👨‍🎓 Я контрибьютор (5+ часов)
```
Полный путь:
  README → ВСЕ статьи от 01 до 11 → Real code

Затем:
  Углубитесь в crates/ исходный код
  Начните делать PR'ы!
```

---

## Быстрая справка (crib sheet)

### Основные концепции
- **CID**: Криптографический идентификатор (hash) контента
- **Block**: Неизменяемая единица, идентифицируемая CID
- **Peer**: Удалённый узел в сети
- **DHT**: Распределённая таблица хеширования (для поиска peer'ов)
- **HNSW**: Индекс для быстрого k-NN поиска (~1ms для 100k vectors)
- **Bitswap**: Протокол обмена блоками между peer'ами

### Инварианты (НИКОГДА не нарушаются)
```
hash(data) == cid         (Storage)
PeerId = hash(pub_key)    (Network)
0 ≤ similarity ≤ 1        (Semantic)
Rules are consistent      (Logic)
FIFO per-peer             (Transport)
```

### Быстрые числа
```
Block GET (cache):  30µs
Block PUT:          50µs
HNSW search:        1-10ms
DHT lookup:         150-300ms
Full fetch:         200-1000ms
Memory (1TB):       ~4.5 GB
```

---

## Интеграция с исходным кодом

Каждая статья wiki ссылается на соответствующие крейты:

```
01-Overview                 → /Vendor/ipfrs/
02-Architecture Stack       → All crates in /Vendor/ipfrs/crates/
03-Bounded Contexts         → Each domain has its crate
04-StorageDomain            → ipfrs-storage/
05-NetworkDomain            → ipfrs-network/
06-SemanticDomain           → ipfrs-semantic/
07-LogicDomain              → ipfrs-tensorlogic/
08-TransportDomain          → ipfrs-transport/
09-DataFlows                → Application logic
10-Performance              → All crates (benchmarks)
11-ErrorHandling            → Error handling in all crates
```

---

## Как использовать эту Wiki

### 📖 Режим 1: Sequential Reading
Начните с README, читайте по порядку (01 → 02 → 03 → ...).

### 🔍 Режим 2: Topic Jumping
Используйте таблицу выше, прыгайте на интересующий вас топик.

### 💾 Режим 3: Reference
Ищите что-то конкретное (например, "HNSW" или "DHT") с Ctrl+F.

### 🔗 Режим 4: Exploration
Следуйте ссылкам между статьями, исследуйте граф знаний.

---

## Обновления и вклады

Эта wiki может быть дополнена:
- Большей информацией о каждом домене (04-08)
- Полной страницей об обработке ошибок (11)
- Примерами кода из исходника
- Диаграммами и визуализациями
- Вопросами/ответами и FAQ

---

## О стиле этой Wiki

Вдохновлено подходом Андрея Карпати к ведению записей:

> "Идея вести подробные заметки о своём исследовании, чтобы создать второй мозг."

Здесь вы найдёте:
- ✓ Главные идеи (не все детали)
- ✓ Много диаграмм и примеров
- ✓ Связи между концепциями
- ✓ Практические инсайты
- ✓ Честный анализ trade-off'ов

---

**Общая статистика Wiki**:
- Статей: 13 (✅ все complete) + 4 meta-файла (README, INDEX, WIKI_SCHEMA, log)
- Строк: 7,400+
- Примеров кода: 100+ (с `file:line` привязкой к источнику)
- Диаграмм: 50+
- Время чтения: 3-5 часов (полностью)
- Паттерн: Karpathy LLM Wiki (3 слоя: источники → wiki → схема)

---

### 👉 Начните отсюда:

1. Если первый раз: **[README.md](README.md)**
2. Если хотите общее понимание: **[01-Overview.md](01-Overview.md)**
3. Если интересует архитектура: **[02-Architecture Stack.md](02-ArchitectureStack.md)**
4. Если хотите примеры: **[09-Data Flows.md](09-DataFlows.md)**

---

**Последнее обновление**: 2026-06-18  
**Версия**: 0.2.0 "Network Release"  
**Статус**: ✅ Active Development

Добро пожаловать в IPFRS Wiki! 🚀
