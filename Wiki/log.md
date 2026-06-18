---
title: log
type: navigation
summary: Append-only хроника эволюции Wiki (формат Карпати — grep-friendly)
tags: [meta, log, llm-wiki]
updated: 2026-06-18
---

# log — Хроника эволюции Wiki

> Append-only. Новые записи — **сверху**. Формат: `## [YYYY-MM-DD] operation | title`.
> `operation ∈ {ingest, query, lint, refactor, sync}`. См. правила в [[WIKI_SCHEMA]].
> Grep-примеры: `grep '\] ingest' log.md`, `grep '\] sync' log.md`.

---

## [2026-06-18] ingest | Перевод IPFRS_ARCHITECTURE_MASTER.md → Wiki
- Создан [[12-MasterArchitecture]] — русский перевод мастер-документа (Opus 4.8, 872 строки)
- 14 частей: стратегическая карта, Shared Kernel, 5 доменов, 2 потока данных, инварианты, компромиссы
- Добавлен YAML frontmatter, Obsidian wikilinks на все связанные статьи
- Обновлён [[INDEX]]: 12 статей, 5,500+ строк

## [2026-06-18] lint | Полная сверка Wiki против реального кода
- Проверено 43 утверждения по 5 доменам (5 параллельных агентов против `Vendor/ipfrs/crates/**`)
- ✅ 41 VERIFIED (структуры, поля, line-номера совпали — Storage 10/10, Semantic 10/10, Logic 11/11)
- 🔧 Реальная ошибка: [[08-TransportDomain]] retry backoff был `2^min(retry,6) cap 32×` → код `2^min(retry,10)` cap 1024× (`want_list.rs:437–459`). Исправлено в Wiki + ARCHITECTURE_DDD_DEEP.md
- ⚠️ Ложное срабатывание агента: [[05-NetworkDomain]] репутация — агент проверил `peer_reputation.rs`, но композитный EWMA живёт в `reputation.rs:140+` (поля transfer_success_rate/latency_score/... реально есть). Идеализированная иллюстрация в Wiki заменена на 3 реальных скорера
- Вывод: документация на ~95% точна к коду; единственная фактическая ошибка устранена

## [2026-06-18] refactor | Применён паттерн Карпати «LLM Wiki»
- Создан [[WIKI_SCHEMA]] — слой-схема (правила поддержки, трёхслойная архитектура, workflows ingest/query/lint)
- Создан этот [[log]] — хроника эволюции
- Добавлен YAML frontmatter во все страницы (type, summary, tags, source, related, read_time)
- Добавлены Obsidian-wikilinks `[[ ]]` для backlinks и графа
- Источник паттерна: Karpathy LLM Wiki gist

## [2026-06-18] sync | Синхронизация Semantic/Logic/Transport с кодом
- [[06-SemanticDomain]]: VectorIndex из `semantic/hnsw.rs`, auto-tuning параметры, 6 domain services
- [[07-LogicDomain]]: IR Term/Predicate/Rule/KnowledgeBase из `ir.rs:13–277`, InferenceEngine
- [[08-TransportDomain]]: Session + watch-каналы, Priority enum, WantList BinaryHeap
- источник: Vendor/ipfrs/ARCHITECTURE_DDD_DEEP.md (анализ Opus 4.8)

## [2026-06-18] sync | Синхронизация Storage/Network с кодом
- [[04-StorageDomain]]: BlockStore trait (все методы), 5 адаптеров, Block из `core/block.rs:57–120`
- [[05-NetworkDomain]]: PeerInfo/PeerRecord из `peer.rs:22–76`, PeerStore, reputation u8 [0,100]
- источник: Vendor/ipfrs/ARCHITECTURE_DDD_DEEP.md

## [2026-06-18] refactor | Выравнивание диаграмм в Obsidian
- Ручное выравнивание ASCII box-drawing: [[02-ArchitectureStack]], [[04-StorageDomain]], [[08-TransportDomain]], [[09-DataFlows]]
- Без изменений содержания, только визуальное выравнивание

## [2026-06-18] ingest | 6 глубоких статей по доменам
- Созданы: [[04-StorageDomain]], [[05-NetworkDomain]], [[06-SemanticDomain]], [[07-LogicDomain]], [[08-TransportDomain]], [[11-ErrorHandling]]
- извлечено из IPFRS_ARCHITECTURE_MASTER.md
- ~2,181 строк, 100+ примеров кода

## [2026-06-18] ingest | Базовая Wiki + мастер-документ
- Созданы первые 7 страниц: [[README]], [[INDEX]], [[01-Overview]], [[02-ArchitectureStack]], [[03-BoundedContexts]], [[09-DataFlows]], [[10-Performance]]
- Стиль «второго мозга» Карпати, навигация по ролям
- источник: глубокая индексация cool-japan/Vendor/ipfrs (12 крейтов)
- параллельно: IPFRS_ARCHITECTURE_MASTER.md (Opus 4.8, code-grounded DDD)
